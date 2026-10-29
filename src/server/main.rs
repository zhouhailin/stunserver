//! STUN Server
//!
//! High-performance STUN server implementation with async I/O.
//! Supports YAML configuration files, console and file logging,
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
    /// Path to YAML configuration file.
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Listen address and port (overrides config file).
    #[arg(short, long)]
    listen: Option<SocketAddr>,

    /// Suppress startup log output.
    #[arg(short, long)]
    quiet: bool,

    /// Enable debug logging (sets log level to debug and prints network messages).
    #[arg(long)]
    debug: bool,
}

fn main() {
    let args = Args::parse();

    // Load configuration: CLI args override file values, file overrides defaults.
    let mut cfg = if let Some(ref config_path) = args.config {
        match config::load_config(config_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
    } else {
        config::default_config()
    };

    // CLI overrides.
    if let Some(listen) = args.listen {
        cfg.server.listen = listen;
    }

    // Debug flag overrides log level.
    if args.debug {
        cfg.logging.level = config::LogLevel::Debug;
    }

    // Validate final configuration.
    if let Err(e) = config::validate_config(&cfg) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }

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

    log::info!("Starting STUN server on {}", cfg.server.listen);

    // Create server state.
    let state = Arc::new(server::ServerState::new());

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
