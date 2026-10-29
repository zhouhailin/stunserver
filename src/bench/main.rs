//! STUN Benchmark Tool
//!
//! Performance testing tool for STUN servers. Sends requests at a
//! configurable QPS rate and reports latency percentiles and success rates.

use clap::Parser;
use std::process;
use std::time::Duration;
use stunbench::{
    address::{AddressFamily, NetworkAddress},
    bench::{self, BenchConfig},
    metadata::BuildMetadata,
};

/// Command-line arguments for stunbench.
#[derive(Parser, Debug)]
#[command(
    name = "stunbench",
    version,
    about = "STUN server benchmark tool",
    before_help = BuildMetadata::capture().format_version_line()
)]
struct Args {
    /// Server address (IP or domain, with optional port). Defaults to port 3478.
    /// Examples: 192.168.1.1, 192.168.1.1:3478, example.com, example.com:3478
    server: String,

    /// Target queries per second.
    #[arg(short, long)]
    qps: u32,

    /// Test duration in seconds.
    #[arg(short, long, default_value = "60")]
    duration: u64,

    /// Total number of requests (alternative to duration).
    #[arg(short, long, conflicts_with = "duration")]
    requests: Option<u64>,

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

    // Validate arguments.
    if args.qps == 0 {
        eprintln!("Error: QPS must be greater than 0");
        process::exit(1);
    }

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

    // Build benchmark configuration.
    let config = if let Some(requests) = args.requests {
        println!(
            "Starting benchmark: {} QPS for {} requests",
            args.qps, requests
        );
        BenchConfig {
            server_addrs: resolved_addrs.clone(),
            target_qps: args.qps,
            duration: None,
            request_count: Some(requests),
        }
    } else {
        println!(
            "Starting benchmark: {} QPS for {} seconds",
            args.qps, args.duration
        );
        BenchConfig {
            server_addrs: resolved_addrs.clone(),
            target_qps: args.qps,
            duration: Some(Duration::from_secs(args.duration)),
            request_count: None,
        }
    };

    println!("Target: {}", network_addr);
    if resolved_addrs.len() > 1 {
        println!(
            "Resolved to {} addresses, using {} (with fallback)",
            resolved_addrs.len(),
            primary_addr
        );
    } else {
        println!("Using address {}", primary_addr);
    }
    println!();

    // Build and run the Tokio runtime.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Error: Failed to create runtime: {}", e);
            process::exit(2);
        }
    };

    let result = runtime.block_on(bench::run_benchmark(&config));

    match result {
        Ok(stats) => {
            bench::print_results(&stats);
            process::exit(0);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(3);
        }
    }
}
