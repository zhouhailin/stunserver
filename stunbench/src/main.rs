//! STUN Benchmark Tool
//!
//! Performance testing tool for STUN servers. Sends requests at a
//! configurable QPS rate and reports latency percentiles and success rates.

use chrono::{Local, Utc};
use clap::Parser;
use std::net::SocketAddr;
use std::process;
use std::time::Duration;
use stunbench::bench::{self, BenchConfig};

/// Command-line arguments for stunbench.
#[derive(Parser, Debug)]
#[command(name = "stunbench", version, about = "STUN server benchmark tool")]
struct Args {
    /// Server address and port (e.g. 192.168.1.1:3478).
    server: SocketAddr,

    /// Target queries per second.
    #[arg(short, long)]
    qps: u32,

    /// Test duration in seconds.
    #[arg(short, long, default_value = "60")]
    duration: u64,

    /// Total number of requests (alternative to duration).
    #[arg(short, long, conflicts_with = "duration")]
    requests: Option<u64>,
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

    println!("STUN Benchmark v{}", version);
    println!("Commit: {}", commit_id);
    println!("Commit Time: {} (UTC)", commit_time);
    println!("Build Time: {} (UTC)", build_time);
    println!("Startup Time: {} (UTC)", startup_time_utc);
    println!("Startup Time: {} ({})", startup_time_local, timezone);
}

fn main() {
    let args = Args::parse();

    // Validate arguments.
    if args.qps == 0 {
        eprintln!("Error: QPS must be greater than 0");
        process::exit(1);
    }

    // Display build info.
    print_build_info();

    // Initialize console-only logging.
    env_logger::init();

    // Build benchmark configuration.
    let config = if let Some(requests) = args.requests {
        println!(
            "Starting benchmark: {} QPS for {} requests",
            args.qps, requests
        );
        BenchConfig {
            server_addr: args.server,
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
            server_addr: args.server,
            target_qps: args.qps,
            duration: Some(Duration::from_secs(args.duration)),
            request_count: None,
        }
    };

    println!("Target: {}", args.server);
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
