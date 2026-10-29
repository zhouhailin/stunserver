//! STUN Client
//!
//! Command-line STUN client for testing server functionality.
//! Sends a STUN Binding Request and displays the mapped address.

use chrono::{Local, Utc};
use clap::Parser;
use std::net::SocketAddr;
use std::process;
use std::time::Duration;
use stunclient::client;

/// Command-line arguments for stunclient.
#[derive(Parser, Debug)]
#[command(name = "stunclient", version, about = "STUN client for testing")]
struct Args {
    /// Server address and port (e.g. 192.168.1.1:3478).
    server: SocketAddr,

    /// Request timeout in seconds.
    #[arg(short, long, default_value = "5")]
    timeout: u64,
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

    println!("STUN Client v{}", version);
    println!("Commit: {}", commit_id);
    println!("Commit Time: {} (UTC)", commit_time);
    println!("Build Time: {} (UTC)", build_time);
    println!("Startup Time: {} (UTC)", startup_time_utc);
    println!("Startup Time: {} ({})", startup_time_local, timezone);
}

fn main() {
    let args = Args::parse();

    // Display build info.
    print_build_info();

    // Initialize console-only logging.
    env_logger::init();

    println!("Sending binding request to {}...", args.server);

    // Build and run the Tokio runtime.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Error: Failed to create runtime: {}", e);
            process::exit(2);
        }
    };

    let timeout = Duration::from_secs(args.timeout);

    let result = runtime.block_on(client::send_binding_request(args.server, timeout));

    match result {
        Ok(response) => {
            println!(
                "Response received in {:.1}ms",
                response.latency.as_secs_f64() * 1000.0
            );
            println!(
                "Mapped Address: {}:{}",
                response.mapped_address.ip(),
                response.mapped_address.port()
            );
            process::exit(0);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            // Map error to exit code.
            match e {
                client::ClientError::Timeout(_) => process::exit(3),
                client::ClientError::Connection(_) => process::exit(2),
                client::ClientError::Protocol(_) => process::exit(4),
                client::ClientError::InvalidAddress(_) => process::exit(1),
            }
        }
    }
}
