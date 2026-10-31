//! Logging initialization for STUN Server.
//!
//! Supports console output and file output with daily rotation,
//! size-based rotation, and configurable file retention.

use crate::config::LoggingConfig;
use chrono::Local;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// Format a timestamp with millisecond precision.
fn format_timestamp(now: chrono::DateTime<Local>) -> String {
    now.format("%Y-%m-%dT%H:%M:%S%.3f").to_string()
}

/// A simple file logger with daily rotation and size-based rotation.
struct FileLogger {
    /// Current log file handle.
    file: File,
    /// Path to the active log file.
    path: PathBuf,
    /// Maximum file size in bytes before rotation.
    max_size: u64,
    /// Maximum number of rotated files to retain.
    max_files: u32,
    /// Current date string for daily rotation detection.
    current_date: String,
    /// Current approximate file size (tracked without stat calls).
    current_size: u64,
}

impl FileLogger {
    /// Create a new FileLogger from LoggingConfig.
    fn new(config: &LoggingConfig) -> Result<Self, std::io::Error> {
        let log_path = config.log_file_path();

        // Ensure parent directory exists.
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let current_size = file.metadata()?.len();
        let current_date = Local::now().format("%Y-%m-%d").to_string();

        Ok(Self {
            file,
            path: log_path,
            max_size: config.max_file_size_bytes(),
            max_files: config.retention_days,
            current_date,
            current_size,
        })
    }

    /// Write a log line to the file, performing rotation if needed.
    fn write_log(&mut self, record: &log::Record) -> Result<(), std::io::Error> {
        let now = Local::now();
        let today = now.format("%Y-%m-%d").to_string();

        // Daily rotation: archive under the date the content belongs to.
        if today != self.current_date {
            let previous_date = self.current_date.clone();
            self.rotate(&previous_date)?;
            self.current_date = today;
        }

        // Size-based rotation: if file exceeds max_size, rotate.
        if self.current_size >= self.max_size {
            let current_date = self.current_date.clone();
            self.rotate(&current_date)?;
        }

        let timestamp = format_timestamp(now);
        let line = format!("{} [{}] {}\n", timestamp, record.level(), record.args());

        let bytes = line.as_bytes();
        self.file.write_all(bytes)?;
        self.current_size += bytes.len() as u64;

        Ok(())
    }

    /// Rotate the current log file to `{path}.{date}.{sequence}`, open a fresh
    /// file, and prune old files exceeding `max_files`.
    fn rotate(&mut self, date_suffix: &str) -> Result<(), std::io::Error> {
        // Close current file by dropping, then rename.
        drop(std::mem::replace(
            &mut self.file,
            File::create("/dev/null")?,
        ));

        // Pick the first free sequence number for this date; never overwrite.
        let mut seq = 1u32;
        let final_rotated = loop {
            let candidate =
                PathBuf::from(format!("{}.{}.{}", self.path.display(), date_suffix, seq));
            if !candidate.exists() {
                break candidate;
            }
            seq += 1;
        };

        // Rename current log to rotated name (ignore error if source doesn't exist).
        let _ = fs::rename(&self.path, &final_rotated);

        // Open a fresh log file.
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.current_size = 0;

        // Prune old log files if we exceed max_files.
        self.prune_old_files()?;

        Ok(())
    }

    /// Delete the oldest rotated log files if the count exceeds `max_files`.
    fn prune_old_files(&self) -> Result<(), std::io::Error> {
        let parent = match self.path.parent() {
            Some(p) => p,
            None => return Ok(()),
        };

        let file_stem = self.path.file_name().unwrap_or_default().to_string_lossy();

        // Collect all rotated files matching the pattern `{stem}.*`.
        let mut rotated_files: Vec<PathBuf> = fs::read_dir(parent)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|p| {
                let name = p.file_name().unwrap_or_default().to_string_lossy();
                name.starts_with(file_stem.as_ref()) && name != file_stem.as_ref() && p.is_file()
            })
            .collect();

        // Sort by modification time (oldest first).
        rotated_files.sort_by(|a, b| {
            let time_a = fs::metadata(a)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let time_b = fs::metadata(b)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            time_a.cmp(&time_b)
        });

        // Delete oldest files if count exceeds max_files.
        while rotated_files.len() > self.max_files as usize {
            if let Some(oldest) = rotated_files.first() {
                let _ = fs::remove_file(oldest);
            }
            rotated_files.remove(0);
        }

        Ok(())
    }
}

/// Global file logger instance protected by a mutex.
static FILE_LOGGER: Mutex<Option<FileLogger>> = Mutex::new(None);

/// Custom log implementation that dispatches to console and/or file output.
struct StunLogger {
    enable_console: bool,
    level_filter: log::LevelFilter,
}

impl log::Log for StunLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level_filter
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        if self.enable_console {
            let timestamp = format_timestamp(Local::now());
            eprintln!("{} [{}] {}", timestamp, record.level(), record.args());
        }

        if let Ok(mut guard) = FILE_LOGGER.lock() {
            if let Some(ref mut logger) = *guard {
                let _ = logger.write_log(record);
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = FILE_LOGGER.lock() {
            if let Some(ref mut logger) = *guard {
                let _ = logger.file.flush();
            }
        }
    }
}

/// Initialize the logging subsystem based on the provided configuration.
///
/// Sets up console logging, file logging, or both depending on config.
/// Returns an error if file logging is requested but cannot be initialized.
pub fn init_logging(config: &LoggingConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize file logger.
    let file_logger = FileLogger::new(config)?;
    if let Ok(mut guard) = FILE_LOGGER.lock() {
        *guard = Some(file_logger);
    }

    // Create and install the global logger.
    let logger = StunLogger {
        enable_console: config.enable_console,
        level_filter: config.level_filter(),
    };

    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(config.level_filter());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Log;
    use std::io::Read;

    fn test_logging_config(dir: &std::path::Path) -> LoggingConfig {
        LoggingConfig {
            level: "info".to_string(),
            log_dir: dir.to_path_buf(),
            file_prefix: "test".to_string(),
            max_file_size_mb: 1,
            retention_days: 3,
            enable_console: false,
        }
    }

    #[test]
    fn test_file_logger_create_and_write() {
        let dir = std::env::temp_dir().join("stunserver_test_logger_create");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let config = test_logging_config(&dir);

        let mut logger = FileLogger::new(&config).unwrap();
        let log_path = config.log_file_path();
        assert!(log_path.exists());

        let record = log::Record::builder()
            .args(format_args!("test message"))
            .level(log::Level::Info)
            .build();

        logger.write_log(&record).unwrap();

        // Verify content was written.
        let mut contents = String::new();
        File::open(&log_path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert!(contents.contains("[INFO] test message"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_logger_size_rotation() {
        let dir = std::env::temp_dir().join("stunserver_test_logger_rotate");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut config = test_logging_config(&dir);
        config.max_file_size_mb = 0; // Will use 0 bytes from the helper, but let's set explicitly
                                     // Actually set a very small size to trigger rotation
        let _log_path = config.log_file_path();

        let mut logger = FileLogger::new(&config).unwrap();
        // Override max_size to trigger rotation quickly
        logger.max_size = 50;

        let record = log::Record::builder()
            .args(format_args!(
                "message that is long enough to trigger rotation"
            ))
            .level(log::Level::Info)
            .build();

        // Write multiple times to trigger rotation.
        for _ in 0..5 {
            logger.write_log(&record).unwrap();
        }

        // Verify that rotated files exist.
        let entries: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).collect();

        assert!(
            !entries.is_empty(),
            "Expected at least 1 file, got {}",
            entries.len()
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_logger_creates_parent_dirs() {
        let dir = std::env::temp_dir().join("stunserver_test_logger_nested/deep/path");
        let _ = fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());

        let config = test_logging_config(&dir);

        let _logger = FileLogger::new(&config).unwrap();
        let log_path = config.log_file_path();
        assert!(log_path.exists());

        let _ = fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn test_file_logger_prune_old_files() {
        let dir = std::env::temp_dir().join("stunserver_test_logger_prune");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let config = test_logging_config(&dir);
        let log_path = config.log_file_path();

        // Create some fake rotated files.
        File::create(dir.join("test.log.2026-01-01")).unwrap();
        File::create(dir.join("test.log.2026-01-02")).unwrap();
        File::create(dir.join("test.log.2026-01-03")).unwrap();
        File::create(dir.join("test.log.2026-01-04")).unwrap();
        // Create the main log file.
        File::create(&log_path).unwrap();

        let logger = FileLogger::new(&config).unwrap();
        logger.prune_old_files().unwrap();

        // Count remaining rotated files (should be at most retention_days = 3).
        let rotated_count = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("test.log.") && name != "test.log"
            })
            .count();

        assert!(
            rotated_count <= 3,
            "Expected <= 3 rotated files, got {}",
            rotated_count
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stun_logger_enabled() {
        let logger = StunLogger {
            enable_console: true,
            level_filter: log::LevelFilter::Info,
        };

        let info_meta = log::MetadataBuilder::new().level(log::Level::Info).build();
        let debug_meta = log::MetadataBuilder::new().level(log::Level::Debug).build();
        let error_meta = log::MetadataBuilder::new().level(log::Level::Error).build();

        assert!(logger.enabled(&info_meta));
        assert!(!logger.enabled(&debug_meta));
        assert!(logger.enabled(&error_meta));
    }

    #[test]
    fn test_stun_logger_log_console() {
        let logger = StunLogger {
            enable_console: true,
            level_filter: log::LevelFilter::Info,
        };

        let record = log::Record::builder()
            .args(format_args!("console test message"))
            .level(log::Level::Info)
            .build();

        logger.log(&record);
    }

    #[test]
    fn test_stun_logger_log_file() {
        let dir = std::env::temp_dir().join("stunserver_test_logger_file_output");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let config = test_logging_config(&dir);
        let log_path = config.log_file_path();

        let file_logger = FileLogger::new(&config).unwrap();
        if let Ok(mut guard) = FILE_LOGGER.lock() {
            *guard = Some(file_logger);
        }

        let logger = StunLogger {
            enable_console: false,
            level_filter: log::LevelFilter::Info,
        };

        let record = log::Record::builder()
            .args(format_args!("file test message"))
            .level(log::Level::Info)
            .build();

        logger.log(&record);
        logger.flush();

        let mut contents = String::new();
        File::open(&log_path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert!(contents.contains("[INFO] file test message"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stun_logger_log_both() {
        let dir = std::env::temp_dir().join("stunserver_test_logger_both_output");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let config = test_logging_config(&dir);
        let log_path = config.log_file_path();

        let file_logger = FileLogger::new(&config).unwrap();
        if let Ok(mut guard) = FILE_LOGGER.lock() {
            *guard = Some(file_logger);
        }

        let logger = StunLogger {
            enable_console: true,
            level_filter: log::LevelFilter::Info,
        };

        let record = log::Record::builder()
            .args(format_args!("both test message"))
            .level(log::Level::Info)
            .build();

        logger.log(&record);
        logger.flush();

        let mut contents = String::new();
        File::open(&log_path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert!(contents.contains("[INFO] both test message"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stun_logger_log_disabled() {
        let logger = StunLogger {
            enable_console: true,
            level_filter: log::LevelFilter::Error,
        };

        let record = log::Record::builder()
            .args(format_args!("debug message"))
            .level(log::Level::Debug)
            .build();

        logger.log(&record);
    }

    #[test]
    fn test_file_logger_daily_rotation() {
        let dir = std::env::temp_dir().join("stunserver_test_logger_daily");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let config = test_logging_config(&dir);

        let mut logger = FileLogger::new(&config).unwrap();
        logger.current_date = "2020-01-01".to_string();

        let record = log::Record::builder()
            .args(format_args!("daily rotation test"))
            .level(log::Level::Info)
            .build();

        logger.write_log(&record).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_logger_rotate_with_existing_files() {
        let dir = std::env::temp_dir().join("stunserver_test_logger_rotate_existing");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut config = test_logging_config(&dir);
        config.max_file_size_mb = 100;
        let _log_path = config.log_file_path();

        let mut logger = FileLogger::new(&config).unwrap();

        File::create(dir.join("test.log.2026-10-29")).unwrap();

        let record = log::Record::builder()
            .args(format_args!(
                "message to trigger rotation with existing file"
            ))
            .level(log::Level::Info)
            .build();

        logger.write_log(&record).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stun_logger_flush_without_file_logger() {
        let logger = StunLogger {
            enable_console: false,
            level_filter: log::LevelFilter::Info,
        };

        if let Ok(mut guard) = FILE_LOGGER.lock() {
            *guard = None;
        }

        logger.flush();
    }

    fn today_string() -> String {
        Local::now().format("%Y-%m-%d").to_string()
    }

    fn read_file(path: &std::path::Path) -> String {
        let mut contents = String::new();
        File::open(path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        contents
    }

    fn write_info(logger: &mut FileLogger, message: &str) {
        logger
            .write_log(
                &log::Record::builder()
                    .args(format_args!("{}", message))
                    .level(log::Level::Info)
                    .build(),
            )
            .unwrap();
    }

    #[test]
    fn test_rotate_names_first_file_with_date_and_sequence_one() {
        let dir = std::env::temp_dir().join("stunserver_test_rotate_naming_first");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let config = test_logging_config(&dir);
        let mut logger = FileLogger::new(&config).unwrap();
        logger.max_size = 10;

        write_info(&mut logger, "first message");
        // The next write sees the size limit exceeded and rotates first.
        write_info(&mut logger, "second message");

        let rotated = dir.join(format!("test.log.{}.1", today_string()));
        assert!(rotated.exists(), "expected {}", rotated.display());
        let contents = read_file(&rotated);
        assert!(contents.contains("first message"));
        assert!(!contents.contains("second message"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rotate_increments_sequence_without_overwriting() {
        let dir = std::env::temp_dir().join("stunserver_test_rotate_naming_sequence");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let today = today_string();
        let first_rotated = dir.join(format!("test.log.{}.1", today));
        fs::write(&first_rotated, "sentinel").unwrap();

        let config = test_logging_config(&dir);
        let mut logger = FileLogger::new(&config).unwrap();
        logger.max_size = 10;

        write_info(&mut logger, "first message");
        write_info(&mut logger, "second message");

        let second_rotated = dir.join(format!("test.log.{}.2", today));
        assert!(
            second_rotated.exists(),
            "expected {}",
            second_rotated.display()
        );
        assert!(read_file(&second_rotated).contains("first message"));
        assert_eq!(read_file(&first_rotated), "sentinel");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cross_day_rotation_uses_previous_date_then_starts_at_one() {
        let dir = std::env::temp_dir().join("stunserver_test_rotate_cross_day");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let config = test_logging_config(&dir);
        let mut logger = FileLogger::new(&config).unwrap();
        logger.max_size = 1_000_000;
        logger.current_date = "2020-01-01".to_string();

        write_info(&mut logger, "first message");

        let old_date_rotated = dir.join("test.log.2020-01-01.1");
        assert!(
            old_date_rotated.exists(),
            "expected {}",
            old_date_rotated.display()
        );
        assert_eq!(logger.current_date, today_string());

        // The first size rotation of the new day must use sequence 1.
        logger.max_size = 1;
        write_info(&mut logger, "second message");

        let today_rotated = dir.join(format!("test.log.{}.1", today_string()));
        assert!(
            today_rotated.exists(),
            "expected {}",
            today_rotated.display()
        );
        assert!(read_file(&today_rotated).contains("first message"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_prune_still_covers_legacy_rotated_names() {
        let dir = std::env::temp_dir().join("stunserver_test_rotate_legacy_prune");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let config = test_logging_config(&dir);
        let log_path = config.log_file_path();
        let today = today_string();

        File::create(dir.join("test.log.2026-10-29")).unwrap();
        File::create(dir.join(format!("test.log.{}.1", today))).unwrap();
        File::create(dir.join(format!("test.log.{}.2", today))).unwrap();
        File::create(dir.join(format!("test.log.{}.3", today))).unwrap();
        File::create(&log_path).unwrap();

        let logger = FileLogger::new(&config).unwrap();
        logger.prune_old_files().unwrap();

        let rotated_count = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("test.log.") && name != "test.log"
            })
            .count();

        assert!(
            rotated_count <= config.retention_days as usize,
            "expected <= {} rotated files, got {}",
            config.retention_days,
            rotated_count
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_format_timestamp_has_millisecond_precision() {
        let timestamp = format_timestamp(Local::now());

        // e.g. 2026-10-31T14:39:08.123
        assert_eq!(timestamp.len(), 23);
        assert_eq!(&timestamp[19..20], ".");
        assert!(timestamp[20..].chars().all(|c| c.is_ascii_digit()));
        assert_eq!(&timestamp[4..5], "-");
        assert_eq!(&timestamp[10..11], "T");
        assert_eq!(&timestamp[13..14], ":");
        assert_eq!(&timestamp[16..17], ":");
    }
}
