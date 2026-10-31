//! STUN Server
//!
//! High-performance STUN server implementation with async I/O.
//! Supports TOML configuration files, console and file logging,
//! and graceful shutdown via SIGINT.

use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use stunserver::{config, logging, metadata::BuildMetadata, server};

/// Command-line arguments for stunserver.
#[derive(Parser, Debug)]
#[command(
    name = "stunserver",
    version,
    about = "High-performance STUN server",
    before_help = BuildMetadata::capture().format_version_line()
)]
struct Args {
    /// Path to TOML configuration file (default: config/stunserver.conf).
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Listen address and port (overrides config file, e.g. "0.0.0.0:3478").
    #[arg(short, long)]
    listen: Option<SocketAddr>,

    /// Suppress startup log output.
    #[arg(short, long)]
    quiet: bool,

    /// Enable debug logging (prints parsed message summaries).
    #[arg(long)]
    debug: bool,

    /// Enable trace logging (prints raw packet bytes; implies debug).
    #[arg(long)]
    trace: bool,
}

fn main() {
    let args = Args::parse();

    // Load configuration: CLI args override file values, file overrides defaults.
    // Without --config, config/stunserver.conf is loaded when it exists.
    let mut cfg = match config::load_config_or_default(
        args.config.as_deref(),
        std::path::Path::new(config::DEFAULT_CONFIG_PATH),
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    // CLI overrides.
    if let Some(listen) = args.listen {
        cfg.server.listen_address = listen.ip().to_string();
        cfg.server.port = listen.port();
    }

    // CLI level flags override the configured log level.
    cfg.logging.level = resolve_log_level(&args, &cfg.logging.level);

    // Validate final configuration.
    if let Err(e) = config::validate_config(&cfg) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }

    // Resolve authentication (derives keys once; never logs secrets).
    let auth = match stunserver::auth::AuthService::from_config(&cfg.auth) {
        Ok(auth) => auth,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    // Display build info.
    if !args.quiet {
        let metadata = BuildMetadata::capture();
        metadata.log_startup();
    }

    // Initialize logging.
    if let Err(e) = logging::init_logging(&cfg.logging) {
        eprintln!("Error: Failed to initialize logging: {}", e);
        process::exit(1);
    }

    let listen_addr = match cfg.server.socket_addr() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("Error: Invalid listen address: {}", e);
            process::exit(1);
        }
    };

    log::info!("Starting STUN server on {}", listen_addr);
    log::info!("Authentication mode: {:?}", auth.mode());

    // Create server state.
    let state = Arc::new(server::ServerState::with_auth(auth));

    // Build and run the Tokio runtime.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            log::error!("Failed to create Tokio runtime: {}", e);
            process::exit(3);
        }
    };

    let result = runtime.block_on(server::run_server(&cfg.server, state));

    match result {
        Ok(()) => {
            log::info!("Server stopped gracefully");
            process::exit(130);
        }
        Err(e) => {
            log::error!("Server error: {}", e);
            // Distinguish network binding errors from other runtime errors.
            let msg = e.to_string();
            if msg.contains("Address already in use")
                || msg.contains("permission denied")
                || msg.contains("Cannot assign requested address")
            {
                process::exit(2);
            }
            process::exit(3);
        }
    }
}

/// Resolve the effective log level: CLI flags override the configured level.
fn resolve_log_level(args: &Args, config_level: &str) -> String {
    if args.trace {
        "trace".to_string()
    } else if args.debug {
        "debug".to_string()
    } else {
        config_level.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Args {
        Args::try_parse_from(args).expect("args should parse")
    }

    #[test]
    fn test_level_override_debug_only() {
        let args = parse(&["stunserver", "--debug"]);
        assert_eq!(resolve_log_level(&args, "info"), "debug");
    }

    #[test]
    fn test_level_override_trace_only() {
        let args = parse(&["stunserver", "--trace"]);
        assert_eq!(resolve_log_level(&args, "info"), "trace");
    }

    #[test]
    fn test_level_override_trace_wins_over_debug() {
        let args = parse(&["stunserver", "--trace", "--debug"]);
        assert_eq!(resolve_log_level(&args, "warn"), "trace");
    }

    #[test]
    fn test_level_override_keeps_config_without_flags() {
        let args = parse(&["stunserver"]);
        assert_eq!(resolve_log_level(&args, "warn"), "warn");
    }

    #[test]
    fn test_help_documents_trace_and_debug() {
        let help = Args::command().render_help().to_string();
        assert!(help.contains("--debug"));
        assert!(help.contains("--trace"));
        assert!(help.contains("prints raw packet bytes"));
    }
}
