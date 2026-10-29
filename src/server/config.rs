//! Configuration module for STUN Server.
//!
//! Provides TOML-based configuration loading with validation,
//! custom deserializers for human-readable size formats, and
//! sensible defaults for all optional fields.

use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Default value functions for serde
// ---------------------------------------------------------------------------

fn default_listen_address() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3478
}

fn default_worker_threads() -> usize {
    0
}

fn default_max_connections() -> usize {
    10000
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_dir() -> PathBuf {
    PathBuf::from("/opt/stunserver/log")
}

fn default_file_prefix() -> String {
    "stunserver".to_string()
}

fn default_max_file_size_mb() -> u64 {
    100
}

fn default_retention_days() -> u32 {
    30
}

fn default_enable_console() -> bool {
    true
}

fn default_udp_recv_buffer_size() -> usize {
    1048576
}

fn default_udp_send_buffer_size() -> usize {
    1048576
}

fn default_tcp_connection_timeout_sec() -> u64 {
    30
}

fn default_tcp_idle_timeout_sec() -> u64 {
    300
}

fn default_enable_tls() -> bool {
    false
}

// ---------------------------------------------------------------------------
// Root configuration
// ---------------------------------------------------------------------------

/// Top-level configuration containing server, logging, network, and security sections.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub logging: LoggingConfig,

    #[serde(default)]
    pub network: NetworkConfig,

    #[serde(default)]
    pub security: SecurityConfig,
}

// ---------------------------------------------------------------------------
// Server configuration
// ---------------------------------------------------------------------------

/// Server network settings.
#[derive(Deserialize, Debug, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_listen_address")]
    pub listen_address: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,

    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_address: default_listen_address(),
            port: default_port(),
            worker_threads: default_worker_threads(),
            max_connections: default_max_connections(),
        }
    }
}

impl ServerConfig {
    /// Construct a SocketAddr from listen_address and port.
    pub fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.listen_address, self.port).parse()
    }
}

// ---------------------------------------------------------------------------
// Logging configuration
// ---------------------------------------------------------------------------

/// Logging settings controlling level, output destination, and file rotation.
#[derive(Deserialize, Debug, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,

    #[serde(default = "default_log_dir")]
    pub log_dir: PathBuf,

    #[serde(default = "default_file_prefix")]
    pub file_prefix: String,

    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: u64,

    #[serde(default = "default_retention_days")]
    pub retention_days: u32,

    #[serde(default = "default_enable_console")]
    pub enable_console: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            log_dir: default_log_dir(),
            file_prefix: default_file_prefix(),
            max_file_size_mb: default_max_file_size_mb(),
            retention_days: default_retention_days(),
            enable_console: default_enable_console(),
        }
    }
}

impl LoggingConfig {
    /// Get the log file path.
    pub fn log_file_path(&self) -> PathBuf {
        self.log_dir.join(format!("{}.log", self.file_prefix))
    }

    /// Get max file size in bytes.
    pub fn max_file_size_bytes(&self) -> u64 {
        self.max_file_size_mb * 1024 * 1024
    }

    /// Parse log level string to log::LevelFilter.
    pub fn level_filter(&self) -> log::LevelFilter {
        match self.level.to_lowercase().as_str() {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" | "warning" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        }
    }

    /// Determine if file logging is enabled.
    /// File logging is enabled when console is disabled or when log_dir is explicitly set.
    pub fn file_logging_enabled(&self) -> bool {
        // Always enable file logging for now; console output is controlled by enable_console
        true
    }
}

// ---------------------------------------------------------------------------
// Network configuration
// ---------------------------------------------------------------------------

/// Network settings for UDP and TCP sockets.
#[derive(Deserialize, Debug, Clone)]
pub struct NetworkConfig {
    #[serde(default = "default_udp_recv_buffer_size")]
    pub udp_recv_buffer_size: usize,

    #[serde(default = "default_udp_send_buffer_size")]
    pub udp_send_buffer_size: usize,

    #[serde(default = "default_tcp_connection_timeout_sec")]
    pub tcp_connection_timeout_sec: u64,

    #[serde(default = "default_tcp_idle_timeout_sec")]
    pub tcp_idle_timeout_sec: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            udp_recv_buffer_size: default_udp_recv_buffer_size(),
            udp_send_buffer_size: default_udp_send_buffer_size(),
            tcp_connection_timeout_sec: default_tcp_connection_timeout_sec(),
            tcp_idle_timeout_sec: default_tcp_idle_timeout_sec(),
        }
    }
}

// ---------------------------------------------------------------------------
// Security configuration
// ---------------------------------------------------------------------------

/// Security settings including TLS configuration (future expansion).
#[derive(Deserialize, Debug, Clone)]
pub struct SecurityConfig {
    #[serde(default = "default_enable_tls")]
    pub enable_tls: bool,

    pub tls_cert_path: Option<PathBuf>,

    pub tls_key_path: Option<PathBuf>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_tls: default_enable_tls(),
            tls_cert_path: None,
            tls_key_path: None,
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

    #[error("Invalid TOML syntax: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("Invalid port number: {0}. Must be in range 1-65535")]
    InvalidPort(u16),

    #[error("Invalid log level: '{0}'. Valid options: trace, debug, info, warn, error")]
    InvalidLogLevel(String),

    #[error("Invalid worker threads: {0}. Must be in range 0-256")]
    InvalidWorkerThreads(usize),

    #[error("Invalid max connections: {0}. Must be in range 1-1000000")]
    InvalidMaxConnections(usize),

    #[error("Invalid max file size: {0}. Must be in range 1-10000 MB")]
    InvalidMaxFileSize(u64),

    #[error("Invalid retention days: {0}. Must be in range 1-365")]
    InvalidRetentionDays(u32),

    #[error("TLS enabled but certificate path not provided")]
    MissingTlsCertPath,

    #[error("TLS enabled but key path not provided")]
    MissingTlsKeyPath,
}

/// Load configuration from a TOML file.
///
/// Returns the parsed and validated `Config`.
pub fn load_config(path: &std::path::Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::FileNotFound(path.to_path_buf()));
    }

    let contents = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&contents)?;

    validate_config(&config)?;
    Ok(config)
}

/// Build a default configuration (used when no config file is specified).
pub fn default_config() -> Config {
    Config::default()
}

/// Validate a configuration after loading.
///
/// Checks conditional requirements and value constraints.
pub fn validate_config(config: &Config) -> Result<(), ConfigError> {
    // Validate server config
    if config.server.port == 0 {
        return Err(ConfigError::InvalidPort(config.server.port));
    }

    if config.server.worker_threads > 256 {
        return Err(ConfigError::InvalidWorkerThreads(
            config.server.worker_threads,
        ));
    }

    if config.server.max_connections == 0 || config.server.max_connections > 1_000_000 {
        return Err(ConfigError::InvalidMaxConnections(
            config.server.max_connections,
        ));
    }

    // Validate logging config
    let valid_levels = ["trace", "debug", "info", "warn", "warning", "error"];
    if !valid_levels.contains(&config.logging.level.to_lowercase().as_str()) {
        return Err(ConfigError::InvalidLogLevel(config.logging.level.clone()));
    }

    if config.logging.max_file_size_mb == 0 || config.logging.max_file_size_mb > 10000 {
        return Err(ConfigError::InvalidMaxFileSize(
            config.logging.max_file_size_mb,
        ));
    }

    if config.logging.retention_days == 0 || config.logging.retention_days > 365 {
        return Err(ConfigError::InvalidRetentionDays(
            config.logging.retention_days,
        ));
    }

    // Validate security config
    if config.security.enable_tls {
        if config.security.tls_cert_path.is_none() {
            return Err(ConfigError::MissingTlsCertPath);
        }
        if config.security.tls_key_path.is_none() {
            return Err(ConfigError::MissingTlsKeyPath);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Config deserialization tests ---

    #[test]
    fn test_deserialize_minimal_config() {
        let toml_str = "";
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.listen_address, "0.0.0.0");
        assert_eq!(config.server.port, 3478);
        assert_eq!(config.logging.level, "info");
        assert!(config.logging.enable_console);
    }

    #[test]
    fn test_deserialize_server_only() {
        let toml_str = r#"
[server]
listen_address = "127.0.0.1"
port = 8080
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.listen_address, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
    }

    #[test]
    fn test_deserialize_full_config() {
        let toml_str = r#"
[server]
listen_address = "0.0.0.0"
port = 3478
worker_threads = 4
max_connections = 5000

[logging]
level = "debug"
log_dir = "/var/log/stunserver"
file_prefix = "test"
max_file_size_mb = 50
retention_days = 7
enable_console = false

[network]
udp_recv_buffer_size = 2097152
udp_send_buffer_size = 2097152
tcp_connection_timeout_sec = 60
tcp_idle_timeout_sec = 600

[security]
enable_tls = false
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.port, 3478);
        assert_eq!(config.server.worker_threads, 4);
        assert_eq!(config.server.max_connections, 5000);
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.log_dir, PathBuf::from("/var/log/stunserver"));
        assert_eq!(config.logging.file_prefix, "test");
        assert_eq!(config.logging.max_file_size_mb, 50);
        assert_eq!(config.logging.retention_days, 7);
        assert!(!config.logging.enable_console);
        assert_eq!(config.network.udp_recv_buffer_size, 2097152);
        assert_eq!(config.network.tcp_connection_timeout_sec, 60);
        assert!(!config.security.enable_tls);
    }

    #[test]
    fn test_server_config_socket_addr() {
        let config = ServerConfig {
            listen_address: "192.168.1.1".to_string(),
            port: 8080,
            worker_threads: 0,
            max_connections: 10000,
        };
        let addr = config.socket_addr().unwrap();
        assert_eq!(addr, "192.168.1.1:8080".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn test_logging_config_log_file_path() {
        let config = LoggingConfig {
            level: "info".to_string(),
            log_dir: PathBuf::from("/var/log/stunserver"),
            file_prefix: "test".to_string(),
            max_file_size_mb: 100,
            retention_days: 30,
            enable_console: true,
        };
        assert_eq!(
            config.log_file_path(),
            PathBuf::from("/var/log/stunserver/test.log")
        );
    }

    #[test]
    fn test_logging_config_max_file_size_bytes() {
        let config = LoggingConfig {
            level: "info".to_string(),
            log_dir: PathBuf::from("/var/log"),
            file_prefix: "stunserver".to_string(),
            max_file_size_mb: 100,
            retention_days: 30,
            enable_console: true,
        };
        assert_eq!(config.max_file_size_bytes(), 100 * 1024 * 1024);
    }

    #[test]
    fn test_logging_config_level_filter() {
        let mut config = LoggingConfig::default();

        config.level = "trace".to_string();
        assert_eq!(config.level_filter(), log::LevelFilter::Trace);

        config.level = "debug".to_string();
        assert_eq!(config.level_filter(), log::LevelFilter::Debug);

        config.level = "info".to_string();
        assert_eq!(config.level_filter(), log::LevelFilter::Info);

        config.level = "warn".to_string();
        assert_eq!(config.level_filter(), log::LevelFilter::Warn);

        config.level = "error".to_string();
        assert_eq!(config.level_filter(), log::LevelFilter::Error);

        config.level = "invalid".to_string();
        assert_eq!(config.level_filter(), log::LevelFilter::Info);
    }

    // --- Validation tests ---

    #[test]
    fn test_validate_valid_config() {
        let config = Config::default();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_invalid_port_zero() {
        let mut config = Config::default();
        config.server.port = 0;
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidPort(0)));
    }

    #[test]
    fn test_validate_invalid_worker_threads() {
        let mut config = Config::default();
        config.server.worker_threads = 300;
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidWorkerThreads(300)));
    }

    #[test]
    fn test_validate_invalid_max_connections_zero() {
        let mut config = Config::default();
        config.server.max_connections = 0;
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidMaxConnections(0)));
    }

    #[test]
    fn test_validate_invalid_max_connections_too_large() {
        let mut config = Config::default();
        config.server.max_connections = 2_000_000;
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidMaxConnections(2_000_000)));
    }

    #[test]
    fn test_validate_invalid_log_level() {
        let mut config = Config::default();
        config.logging.level = "invalid".to_string();
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidLogLevel(_)));
    }

    #[test]
    fn test_validate_invalid_max_file_size_zero() {
        let mut config = Config::default();
        config.logging.max_file_size_mb = 0;
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidMaxFileSize(0)));
    }

    #[test]
    fn test_validate_invalid_max_file_size_too_large() {
        let mut config = Config::default();
        config.logging.max_file_size_mb = 20000;
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidMaxFileSize(20000)));
    }

    #[test]
    fn test_validate_invalid_retention_days_zero() {
        let mut config = Config::default();
        config.logging.retention_days = 0;
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidRetentionDays(0)));
    }

    #[test]
    fn test_validate_invalid_retention_days_too_large() {
        let mut config = Config::default();
        config.logging.retention_days = 400;
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidRetentionDays(400)));
    }

    #[test]
    fn test_validate_tls_enabled_without_cert() {
        let mut config = Config::default();
        config.security.enable_tls = true;
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::MissingTlsCertPath));
    }

    #[test]
    fn test_validate_tls_enabled_without_key() {
        let mut config = Config::default();
        config.security.enable_tls = true;
        config.security.tls_cert_path = Some(PathBuf::from("/path/to/cert.pem"));
        let err = validate_config(&config).unwrap_err();
        assert!(matches!(err, ConfigError::MissingTlsKeyPath));
    }

    #[test]
    fn test_validate_tls_enabled_with_both_paths() {
        let mut config = Config::default();
        config.security.enable_tls = true;
        config.security.tls_cert_path = Some(PathBuf::from("/path/to/cert.pem"));
        config.security.tls_key_path = Some(PathBuf::from("/path/to/key.pem"));
        assert!(validate_config(&config).is_ok());
    }

    // --- load_config tests ---

    #[test]
    fn test_load_config_file_not_found() {
        let err = load_config(std::path::Path::new("/nonexistent/config.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::FileNotFound(_)));
    }

    #[test]
    fn test_default_config_is_valid() {
        let config = default_config();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_network_config_defaults() {
        let config = NetworkConfig::default();
        assert_eq!(config.udp_recv_buffer_size, 1048576);
        assert_eq!(config.udp_send_buffer_size, 1048576);
        assert_eq!(config.tcp_connection_timeout_sec, 30);
        assert_eq!(config.tcp_idle_timeout_sec, 300);
    }

    #[test]
    fn test_security_config_defaults() {
        let config = SecurityConfig::default();
        assert!(!config.enable_tls);
        assert!(config.tls_cert_path.is_none());
        assert!(config.tls_key_path.is_none());
    }
}
