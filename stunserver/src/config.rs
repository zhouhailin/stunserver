//! Configuration module for STUN Server.
//!
//! Provides YAML-based configuration loading with validation,
//! custom deserializers for human-readable size formats, and
//! sensible defaults for all optional fields.

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Default value functions for serde
// ---------------------------------------------------------------------------

/// Default listen address: all interfaces on STUN port 3478.
fn default_listen() -> SocketAddr {
    "0.0.0.0:3478"
        .parse()
        .expect("valid default listen address")
}

/// Default log level: info.
fn default_log_level() -> LogLevel {
    LogLevel::Info
}

/// Default log output: both console and file.
fn default_log_output() -> LogOutput {
    LogOutput::Both
}

// ---------------------------------------------------------------------------
// Root configuration
// ---------------------------------------------------------------------------

/// Top-level configuration containing server and logging sections.
#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    /// Server network and operational settings.
    #[serde(default)]
    pub server: ServerConfig,

    /// Logging configuration.
    #[serde(default)]
    pub logging: LoggingConfig,
}

// ---------------------------------------------------------------------------
// Server configuration
// ---------------------------------------------------------------------------

/// Server network settings.
#[derive(Deserialize, Debug, Clone)]
pub struct ServerConfig {
    /// Listen address and port (e.g. "0.0.0.0:3478").
    #[serde(
        default = "default_listen",
        deserialize_with = "deserialize_socket_addr"
    )]
    pub listen: SocketAddr,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: default_listen(),
        }
    }
}

/// Custom deserializer for SocketAddr from a YAML string.
fn deserialize_socket_addr<'de, D>(deserializer: D) -> Result<SocketAddr, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<SocketAddr>().map_err(de::Error::custom)
}

// ---------------------------------------------------------------------------
// Logging configuration
// ---------------------------------------------------------------------------

/// Logging settings controlling level, output destination, and file rotation.
#[derive(Deserialize, Debug, Clone)]
pub struct LoggingConfig {
    /// Log level filter.
    #[serde(default = "default_log_level")]
    pub level: LogLevel,

    /// Where log output is sent.
    #[serde(default = "default_log_output")]
    pub output: LogOutput,

    /// File logging configuration (required when output is "file" or "both").
    pub file: Option<FileLogConfig>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            output: default_log_output(),
            file: Some(FileLogConfig::default()),
        }
    }
}

// ---------------------------------------------------------------------------
// File logging configuration
// ---------------------------------------------------------------------------

/// File-based logging configuration with rotation support.
#[derive(Deserialize, Debug, Clone)]
pub struct FileLogConfig {
    /// Log file path (absolute or relative).
    pub path: PathBuf,

    /// Maximum file size before rotation (e.g. "100MB").
    pub max_size: FileSize,

    /// Rotation strategy (currently only "daily" is supported).
    pub rotation: LogRotation,

    /// Maximum number of rotated log files to retain.
    pub max_files: u32,
}

impl Default for FileLogConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("/var/log/stunserver/stunserver.log"),
            max_size: FileSize(100 * 1024 * 1024), // 100 MB
            rotation: LogRotation::Daily,
            max_files: 7,
        }
    }
}

// ---------------------------------------------------------------------------
// Enum types
// ---------------------------------------------------------------------------

/// Log level filter.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    /// Convert to the `log` crate's LevelFilter.
    pub fn to_level_filter(self) -> log::LevelFilter {
        match self {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Error => write!(f, "error"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Debug => write!(f, "debug"),
        }
    }
}

/// Log output destination.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogOutput {
    Console,
    File,
    Both,
}

/// Log rotation strategy.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogRotation {
    Daily,
}

// ---------------------------------------------------------------------------
// FileSize custom type
// ---------------------------------------------------------------------------

/// A file size in bytes, deserialized from a human-readable string like "100MB".
///
/// Supported units: B, KB, MB, GB (powers of 1024).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileSize(pub u64);

impl FileSize {
    /// Parse a human-readable size string into bytes.
    pub fn parse(s: &str) -> Result<u64, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("empty size string".to_string());
        }

        // Find where the numeric part ends and the unit begins.
        let unit_start = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());

        let (num_part, unit_part) = s.split_at(unit_start);
        let num_part = num_part.trim();
        let unit_part = unit_part.trim().to_uppercase();

        if num_part.is_empty() {
            return Err(format!("missing numeric value in size: '{}'", s));
        }

        let value: u64 = num_part
            .parse()
            .map_err(|e| format!("invalid number in size '{}': {}", s, e))?;

        let multiplier: u64 = match unit_part.as_str() {
            "B" => 1,
            "KB" => 1024,
            "MB" => 1024 * 1024,
            "GB" => 1024 * 1024 * 1024,
            "" => 1, // bare number treated as bytes
            other => {
                return Err(format!(
                    "unknown size unit '{}' in '{}'. Valid units: B, KB, MB, GB",
                    other, s
                ))
            }
        };

        if value == 0 {
            return Err(format!("size must be > 0, got '{}'", s));
        }

        Ok(value * multiplier)
    }
}

impl<'de> Deserialize<'de> for FileSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FileSizeVisitor;

        impl<'de> Visitor<'de> for FileSizeVisitor {
            type Value = FileSize;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a file size string like \"100MB\" or a number of bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<FileSize, E>
            where
                E: de::Error,
            {
                FileSize::parse(value)
                    .map(FileSize)
                    .map_err(de::Error::custom)
            }

            fn visit_u64<E>(self, value: u64) -> Result<FileSize, E>
            where
                E: de::Error,
            {
                if value == 0 {
                    Err(de::Error::custom("size must be > 0"))
                } else {
                    Ok(FileSize(value))
                }
            }
        }

        deserializer.deserialize_any(FileSizeVisitor)
    }
}

impl fmt::Display for FileSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.0;
        if bytes >= 1024 * 1024 * 1024 && bytes.is_multiple_of(1024 * 1024 * 1024) {
            write!(f, "{}GB", bytes / (1024 * 1024 * 1024))
        } else if bytes >= 1024 * 1024 && bytes.is_multiple_of(1024 * 1024) {
            write!(f, "{}MB", bytes / (1024 * 1024))
        } else if bytes >= 1024 && bytes.is_multiple_of(1024) {
            write!(f, "{}KB", bytes / 1024)
        } else {
            write!(f, "{}B", bytes)
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration loading and validation
// ---------------------------------------------------------------------------

/// Errors that can occur during configuration loading or validation.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Failed to read configuration file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Invalid YAML syntax: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("Invalid logging level: '{0}'. Valid options: error, warn, info, debug")]
    InvalidLogLevel(String),

    #[error("Invalid log output: '{0}'. Valid options: console, file, both")]
    InvalidLogOutput(String),

    #[error("File logging configuration required when output is 'file' or 'both'")]
    MissingFileConfig,

    #[error("Invalid file size: {0}")]
    InvalidFileSize(String),

    #[error("max_files must be > 0")]
    InvalidMaxFiles,
}

/// Load configuration from a YAML file.
///
/// Returns the parsed and validated `Config`.
pub fn load_config(path: &std::path::Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::FileNotFound(path.to_path_buf()));
    }

    let contents = std::fs::read_to_string(path)?;
    let config: Config = serde_yaml::from_str(&contents)?;

    validate_config(&config)?;
    Ok(config)
}

/// Build a default configuration (used when no config file is specified).
pub fn default_config() -> Config {
    Config {
        server: ServerConfig::default(),
        logging: LoggingConfig::default(),
    }
}

/// Validate a configuration after loading.
///
/// Checks conditional requirements and value constraints.
pub fn validate_config(config: &Config) -> Result<(), ConfigError> {
    // If output requires file logging, the file section must be present.
    match config.logging.output {
        LogOutput::File | LogOutput::Both => {
            if config.logging.file.is_none() {
                return Err(ConfigError::MissingFileConfig);
            }
        }
        LogOutput::Console => {}
    }

    // Validate file config constraints when present.
    if let Some(ref file_cfg) = config.logging.file {
        if file_cfg.max_size.0 == 0 {
            return Err(ConfigError::InvalidFileSize(
                "max_size must be > 0".to_string(),
            ));
        }
        if file_cfg.max_files == 0 {
            return Err(ConfigError::InvalidMaxFiles);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- FileSize parsing tests ---

    #[test]
    fn test_file_size_parse_bytes() {
        assert_eq!(FileSize::parse("100B").unwrap(), 100);
    }

    #[test]
    fn test_file_size_parse_kb() {
        assert_eq!(FileSize::parse("500KB").unwrap(), 500 * 1024);
    }

    #[test]
    fn test_file_size_parse_mb() {
        assert_eq!(FileSize::parse("100MB").unwrap(), 100 * 1024 * 1024);
    }

    #[test]
    fn test_file_size_parse_gb() {
        assert_eq!(FileSize::parse("1GB").unwrap(), 1024 * 1024 * 1024);
    }

    #[test]
    fn test_file_size_parse_case_insensitive() {
        assert_eq!(FileSize::parse("100mb").unwrap(), 100 * 1024 * 1024);
        assert_eq!(FileSize::parse("1gb").unwrap(), 1024 * 1024 * 1024);
    }

    #[test]
    fn test_file_size_parse_zero_rejected() {
        assert!(FileSize::parse("0MB").is_err());
    }

    #[test]
    fn test_file_size_parse_empty_rejected() {
        assert!(FileSize::parse("").is_err());
    }

    #[test]
    fn test_file_size_parse_invalid_unit() {
        assert!(FileSize::parse("100TB").is_err());
    }

    #[test]
    fn test_file_size_display() {
        assert_eq!(format!("{}", FileSize(100)), "100B");
        assert_eq!(format!("{}", FileSize(1024)), "1KB");
        assert_eq!(format!("{}", FileSize(100 * 1024 * 1024)), "100MB");
        assert_eq!(format!("{}", FileSize(2 * 1024 * 1024 * 1024)), "2GB");
    }

    // --- Config deserialization tests ---

    #[test]
    fn test_deserialize_minimal_config() {
        let yaml = "";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config.server.listen,
            "0.0.0.0:3478".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(config.logging.level, LogLevel::Info);
        assert_eq!(config.logging.output, LogOutput::Both);
    }

    #[test]
    fn test_deserialize_server_only() {
        let yaml = r#"
server:
  listen: "127.0.0.1:8080"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config.server.listen,
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn test_deserialize_full_config() {
        let yaml = r#"
server:
  listen: "0.0.0.0:3478"
logging:
  level: debug
  output: file
  file:
    path: "/var/log/stunserver/test.log"
    max_size: "50MB"
    rotation: daily
    max_files: 3
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.logging.level, LogLevel::Debug);
        assert_eq!(config.logging.output, LogOutput::File);
        let file_cfg = config.logging.file.unwrap();
        assert_eq!(file_cfg.path, PathBuf::from("/var/log/stunserver/test.log"));
        assert_eq!(file_cfg.max_size.0, 50 * 1024 * 1024);
        assert_eq!(file_cfg.rotation, LogRotation::Daily);
        assert_eq!(file_cfg.max_files, 3);
    }

    #[test]
    fn test_deserialize_console_only() {
        let yaml = r#"
logging:
  level: debug
  output: console
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.logging.output, LogOutput::Console);
        assert!(config.logging.file.is_none());
    }

    // --- Validation tests ---

    #[test]
    fn test_validate_missing_file_config_for_file_output() {
        let config = Config {
            server: ServerConfig::default(),
            logging: LoggingConfig {
                level: LogLevel::Info,
                output: LogOutput::File,
                file: None,
            },
        };
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::MissingFileConfig));
    }

    #[test]
    fn test_validate_missing_file_config_for_both_output() {
        let config = Config {
            server: ServerConfig::default(),
            logging: LoggingConfig {
                level: LogLevel::Info,
                output: LogOutput::Both,
                file: None,
            },
        };
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::MissingFileConfig));
    }

    #[test]
    fn test_validate_console_without_file_config_ok() {
        let config = Config {
            server: ServerConfig::default(),
            logging: LoggingConfig {
                level: LogLevel::Info,
                output: LogOutput::Console,
                file: None,
            },
        };
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_zero_max_files() {
        let config = Config {
            server: ServerConfig::default(),
            logging: LoggingConfig {
                level: LogLevel::Info,
                output: LogOutput::File,
                file: Some(FileLogConfig {
                    path: PathBuf::from("/tmp/test.log"),
                    max_size: FileSize(1024),
                    rotation: LogRotation::Daily,
                    max_files: 0,
                }),
            },
        };
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidMaxFiles));
    }

    #[test]
    fn test_validate_valid_config() {
        let config = Config {
            server: ServerConfig::default(),
            logging: LoggingConfig {
                level: LogLevel::Info,
                output: LogOutput::File,
                file: Some(FileLogConfig {
                    path: PathBuf::from("/tmp/test.log"),
                    max_size: FileSize(100 * 1024 * 1024),
                    rotation: LogRotation::Daily,
                    max_files: 7,
                }),
            },
        };
        assert!(validate_config(&config).is_ok());
    }

    // --- load_config tests ---

    #[test]
    fn test_load_config_file_not_found() {
        let err = load_config(std::path::Path::new("/nonexistent/config.yaml")).unwrap_err();
        assert!(matches!(err, ConfigError::FileNotFound(_)));
    }

    #[test]
    fn test_default_config_is_valid() {
        let config = default_config();
        assert!(validate_config(&config).is_ok());
    }

    // --- LogLevel tests ---

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Error.to_string(), "error");
        assert_eq!(LogLevel::Warn.to_string(), "warn");
        assert_eq!(LogLevel::Info.to_string(), "info");
        assert_eq!(LogLevel::Debug.to_string(), "debug");
    }

    #[test]
    fn test_log_level_to_level_filter() {
        assert_eq!(LogLevel::Error.to_level_filter(), log::LevelFilter::Error);
        assert_eq!(LogLevel::Warn.to_level_filter(), log::LevelFilter::Warn);
        assert_eq!(LogLevel::Info.to_level_filter(), log::LevelFilter::Info);
        assert_eq!(LogLevel::Debug.to_level_filter(), log::LevelFilter::Debug);
    }
}
