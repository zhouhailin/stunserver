//! STUN Server
//!
//! High-performance STUN server implementation with async I/O.
//! Supports YAML configuration files, console and file logging,
//! and graceful shutdown via SIGINT.

use chrono::{Local, Utc};
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use stunserver::{config, logging, server};

/// Command-line arguments for stunserver.
#[derive(Parser, Debug)]
#[command(name = "stunserver", version, about = "High-performance STUN server")]
struct Args {
    /// Path to YAML configuration file.
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Listen address and port (overrides config file).
    #[arg(short, long)]
    listen: Option<SocketAddr>,
}

/// Display build metadata at startup.
fn print_build_info() {
    let version = env!("APP_VERSION");
    let commit_id = env!("GIT_COMMIT_ID");
    let commit_time = env!("GIT_COMMIT_TIME");
    let build_time = env!("BUILD_TIME");
    let startup_time_utc = Utc::now().to_rfc3339();
    let startup_time_local = Local::now().to_rfc3339();
    let timezone = Local::now().offset().to_string();

    println!("STUN Server v{}", version);
    println!("Commit: {}", commit_id);
    println!("Commit Time: {} (UTC)", commit_time);
    println!("Build Time: {} (UTC)", build_time);
    println!("Startup Time: {} (UTC)", startup_time_utc);
    println!("Startup Time: {} ({})", startup_time_local, timezone);
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

    // Validate final configuration.
    if let Err(e) = config::validate_config(&cfg) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }

    // Display build info.
    print_build_info();

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
