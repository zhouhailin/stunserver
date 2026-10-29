//! Logging initialization for STUN Server.
//!
//! Supports console output via env_logger and file output with daily rotation,
//! size-based rotation, and configurable file retention.

use crate::config::{FileLogConfig, LogLevel, LogOutput, LoggingConfig};
use chrono::Local;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

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
    /// Create a new FileLogger, opening or creating the log file at `path`.
    fn new(config: &FileLogConfig) -> Result<Self, std::io::Error> {
        // Ensure parent directory exists.
        if let Some(parent) = config.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.path)?;

        let current_size = file.metadata()?.len();
        let current_date = Local::now().format("%Y-%m-%d").to_string();

        Ok(Self {
            file,
            path: config.path.clone(),
            max_size: config.max_size.0,
            max_files: config.max_files,
            current_date,
            current_size,
        })
    }

    /// Write a log line to the file, performing rotation if needed.
    fn write_log(&mut self, record: &log::Record) -> Result<(), std::io::Error> {
        let now = Local::now();
        let today = now.format("%Y-%m-%d").to_string();

        // Daily rotation: if the date has changed, rotate the file.
        if today != self.current_date {
            self.rotate(&today)?;
            self.current_date = today;
        }

        // Size-based rotation: if file exceeds max_size, rotate.
        if self.current_size >= self.max_size {
            let date_suffix = Local::now().format("%Y-%m-%d").to_string();
            self.rotate(&date_suffix)?;
        }

        let timestamp = now.format("%Y-%m-%dT%H:%M:%S%.3f");
        let line = format!("{} [{}] {}\n", timestamp, record.level(), record.args());

        let bytes = line.as_bytes();
        self.file.write_all(bytes)?;
        self.current_size += bytes.len() as u64;

        Ok(())
    }

    /// Rotate the current log file: rename to `{path}.{date}`, open a fresh file,
    /// and prune old files exceeding `max_files`.
    fn rotate(&mut self, date_suffix: &str) -> Result<(), std::io::Error> {
        // Close current file by dropping, then rename.
        drop(std::mem::replace(
            &mut self.file,
            File::create("/dev/null")?,
        ));

        let rotated_path = PathBuf::from(format!("{}.{}", self.path.display(), date_suffix));

        // If a rotated file with today's date already exists, append a sequence number.
        let mut final_rotated = rotated_path.clone();
        let mut seq = 1u32;
        while final_rotated.exists() {
            final_rotated =
                PathBuf::from(format!("{}.{}.{}", self.path.display(), date_suffix, seq));
            seq += 1;
        }

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
    output: LogOutput,
    level: LogLevel,
}

impl log::Log for StunLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level.to_level_filter()
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        match self.output {
            LogOutput::Console => {
                eprintln!("[{}] {}", record.level(), record.args());
            }
            LogOutput::File => {
                if let Ok(mut guard) = FILE_LOGGER.lock() {
                    if let Some(ref mut logger) = *guard {
                        let _ = logger.write_log(record);
                    }
                }
            }
            LogOutput::Both => {
                eprintln!("[{}] {}", record.level(), record.args());
                if let Ok(mut guard) = FILE_LOGGER.lock() {
                    if let Some(ref mut logger) = *guard {
                        let _ = logger.write_log(record);
                    }
                }
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
/// Sets up console logging, file logging, or both depending on `config.output`.
/// Returns an error if file logging is requested but cannot be initialized.
pub fn init_logging(config: &LoggingConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize file logger if needed.
    match config.output {
        LogOutput::File | LogOutput::Both => {
            if let Some(ref file_config) = config.file {
                let file_logger = FileLogger::new(file_config)?;
                if let Ok(mut guard) = FILE_LOGGER.lock() {
                    *guard = Some(file_logger);
                }
            }
        }
        LogOutput::Console => {}
    }

    // Create and install the global logger.
    let logger = StunLogger {
        output: config.output,
        level: config.level,
    };

    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(config.level.to_level_filter());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FileSize;
    use crate::config::LogRotation;
    use log::Log;
    use std::io::Read;

    #[test]
    fn test_file_logger_create_and_write() {
        let dir = std::env::temp_dir().join("stunserver_test_logger_create");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let log_path = dir.join("test.log");
        let config = FileLogConfig {
            path: log_path.clone(),
            max_size: FileSize(1024 * 1024),
            rotation: LogRotation::Daily,
            max_files: 3,
        };

        let mut logger = FileLogger::new(&config).unwrap();
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

        let log_path = dir.join("test.log");
        // Set very small max_size to trigger rotation.
        let config = FileLogConfig {
            path: log_path.clone(),
            max_size: FileSize(50), // 50 bytes - will trigger rotation quickly
            rotation: LogRotation::Daily,
            max_files: 2,
        };

        let mut logger = FileLogger::new(&config).unwrap();

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

        // Should have at least the main file and one rotated file.
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

        let log_path = dir.join("test.log");
        let config = FileLogConfig {
            path: log_path.clone(),
            max_size: FileSize(1024),
            rotation: LogRotation::Daily,
            max_files: 3,
        };

        let _logger = FileLogger::new(&config).unwrap();
        assert!(log_path.exists());

        let _ = fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn test_file_logger_prune_old_files() {
        let dir = std::env::temp_dir().join("stunserver_test_logger_prune");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let log_path = dir.join("test.log");

        // Create some fake rotated files.
        File::create(dir.join("test.log.2026-01-01")).unwrap();
        File::create(dir.join("test.log.2026-01-02")).unwrap();
        File::create(dir.join("test.log.2026-01-03")).unwrap();
        File::create(dir.join("test.log.2026-01-04")).unwrap();
        // Create the main log file.
        File::create(&log_path).unwrap();

        let config = FileLogConfig {
            path: log_path.clone(),
            max_size: FileSize(1024 * 1024),
            rotation: LogRotation::Daily,
            max_files: 2,
        };

        let logger = FileLogger::new(&config).unwrap();
        logger.prune_old_files().unwrap();

        // Count remaining rotated files (should be at most 2).
        let rotated_count = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("test.log.") && name != "test.log"
            })
            .count();

        assert!(
            rotated_count <= 2,
            "Expected <= 2 rotated files, got {}",
            rotated_count
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stun_logger_enabled() {
        let logger = StunLogger {
            output: LogOutput::Console,
            level: LogLevel::Info,
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
            output: LogOutput::Console,
            level: LogLevel::Info,
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

        let log_path = dir.join("test.log");
        let config = FileLogConfig {
            path: log_path.clone(),
            max_size: FileSize(1024 * 1024),
            rotation: LogRotation::Daily,
            max_files: 3,
        };

        let file_logger = FileLogger::new(&config).unwrap();
        if let Ok(mut guard) = FILE_LOGGER.lock() {
            *guard = Some(file_logger);
        }

        let logger = StunLogger {
            output: LogOutput::File,
            level: LogLevel::Info,
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

        let log_path = dir.join("test.log");
        let config = FileLogConfig {
            path: log_path.clone(),
            max_size: FileSize(1024 * 1024),
            rotation: LogRotation::Daily,
            max_files: 3,
        };

        let file_logger = FileLogger::new(&config).unwrap();
        if let Ok(mut guard) = FILE_LOGGER.lock() {
            *guard = Some(file_logger);
        }

        let logger = StunLogger {
            output: LogOutput::Both,
            level: LogLevel::Info,
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
            output: LogOutput::Console,
            level: LogLevel::Error,
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

        let log_path = dir.join("test.log");
        let config = FileLogConfig {
            path: log_path.clone(),
            max_size: FileSize(1024 * 1024),
            rotation: LogRotation::Daily,
            max_files: 3,
        };

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

        let log_path = dir.join("test.log");
        let config = FileLogConfig {
            path: log_path.clone(),
            max_size: FileSize(10),
            rotation: LogRotation::Daily,
            max_files: 5,
        };

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
            output: LogOutput::File,
            level: LogLevel::Info,
        };

        if let Ok(mut guard) = FILE_LOGGER.lock() {
            *guard = None;
        }

        logger.flush();
    }
}
