//! STUN Client
//!
//! Command-line STUN client for testing server functionality.
//! Sends a STUN Binding Request and displays the mapped address.

use clap::Parser;
use std::process;
use std::time::Duration;
use stunclient::{
    address::{AddressFamily, NetworkAddress},
    client,
    metadata::BuildMetadata,
};

/// Command-line arguments for stunclient.
#[derive(Parser, Debug)]
#[command(
    name = "stunclient",
    version,
    about = "STUN client for testing",
    before_help = BuildMetadata::capture().format_version_line()
)]
struct Args {
    /// Server address (IP or domain, with optional port). Defaults to port 3478.
    /// Examples: 192.168.1.1, 192.168.1.1:3478, example.com, example.com:3478
    #[arg(default_value = "stun.l.google.com:19302")]
    server: String,

    /// Request timeout in seconds.
    #[arg(short, long, default_value = "5")]
    timeout: u64,

    /// Suppress startup log output.
    #[arg(long)]
    quiet: bool,

    /// Enable debug logging (prints network messages).
    #[arg(long)]
    debug: bool,

    /// Use only IPv4 addresses.
    #[arg(short = '4', long = "ipv4-only", conflicts_with = "ipv6_only")]
    ipv4_only: bool,

    /// Use only IPv6 addresses.
    #[arg(short = '6', long = "ipv6-only", conflicts_with = "ipv4_only")]
    ipv6_only: bool,
}

fn main() {
    let args = Args::parse();

    // Display build info.
    if !args.quiet {
        let metadata = BuildMetadata::capture();
        metadata.log_startup();
    }

    // Initialize console-only logging with debug level if requested.
    if args.debug {
        std::env::set_var("RUST_LOG", "debug");
    }
    env_logger::init();

    // Determine address family filter.
    let family = if args.ipv4_only {
        AddressFamily::IPv4Only
    } else if args.ipv6_only {
        AddressFamily::IPv6Only
    } else {
        AddressFamily::Any
    };

    // Parse server address with default port 3478.
    let mut network_addr = match NetworkAddress::parse_with_family(&args.server, 3478, family) {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    // Resolve DNS if needed.
    if let Err(e) = network_addr.resolve() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }

    let resolved_addrs = network_addr.resolved_addrs.clone();
    let primary_addr = network_addr.primary_address().unwrap();

    println!("Sending binding request to {}...", network_addr);
    if resolved_addrs.len() > 1 {
        log::info!(
            "Resolved to {} addresses, using {} (with fallback)",
            resolved_addrs.len(),
            primary_addr
        );
    } else {
        log::info!("Using address {}", primary_addr);
    }

    // Build and run the Tokio runtime.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Error: Failed to create runtime: {}", e);
            process::exit(2);
        }
    };

    let timeout = Duration::from_secs(args.timeout);

    // Try addresses with fallback.
    let result = runtime.block_on(client::send_binding_request_with_fallback(
        &resolved_addrs,
        timeout,
    ));

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
