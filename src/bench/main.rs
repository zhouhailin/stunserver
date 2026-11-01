//! STUN Benchmark Tool
//!
//! Performance testing tool for STUN servers. Sends requests at a
//! configurable QPS rate and reports latency percentiles and success rates.

use chrono::Local;
use clap::{Parser, ValueEnum};
use std::io::Write;
use std::process;
use std::time::Duration;
use stunbench::{
    address::{AddressFamily, NetworkAddress},
    bench::{self, BenchAuth, BenchConfig},
    metadata::BuildMetadata,
};

/// Credential mode requested on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum AuthModeArg {
    Disabled,
    ShortTerm,
    LongTerm,
}

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

    /// Enable debug logging (prints message summaries).
    #[arg(long)]
    debug: bool,

    /// Enable trace logging (prints raw packet bytes; implies debug).
    #[arg(long)]
    trace: bool,

    /// Use only IPv4 addresses.
    #[arg(short = '4', long = "ipv4-only", conflicts_with = "ipv6_only")]
    ipv4_only: bool,

    /// Use only IPv6 addresses.
    #[arg(short = '6', long = "ipv6-only", conflicts_with = "ipv4_only")]
    ipv6_only: bool,

    /// Append a valid terminal FINGERPRINT to every request.
    #[arg(long)]
    fingerprint: bool,

    /// Credential mode: disabled, short-term, or long-term.
    #[arg(long, value_enum, default_value = "disabled")]
    auth_mode: AuthModeArg,

    /// Username for short-term or long-term credentials.
    #[arg(long)]
    username: Option<String>,

    /// Password for short-term or long-term credentials. Visible in process
    /// listings; prefer the STUNBENCH_PASSWORD environment variable.
    #[arg(long)]
    password: Option<String>,

    /// REALM for long-term credentials, overriding the server challenge realm.
    #[arg(long)]
    realm: Option<String>,
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

    // Initialize console-only logging with the requested level.
    if let Some(level) = requested_log_level(&args) {
        std::env::set_var("RUST_LOG", level);
    }
    init_console_logging();

    let auth = match resolve_auth(&args, std::env::var("STUNBENCH_PASSWORD").ok().as_deref()) {
        Ok(auth) => auth,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };
    if let Some(warning) = password_warning(&args) {
        log::warn!("{}", warning);
    }

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
            fingerprint: args.fingerprint,
            auth: auth.clone(),
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
            fingerprint: args.fingerprint,
            auth,
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

/// Build benchmark credentials from CLI flags plus the optional env-var password.
fn resolve_auth(args: &Args, env_password: Option<&str>) -> Result<BenchAuth, String> {
    match args.auth_mode {
        AuthModeArg::Disabled => {
            if args.username.is_some() || args.password.is_some() {
                return Err(
                    "--username/--password require --auth-mode short-term or long-term".to_string(),
                );
            }
            if args.realm.is_some() {
                return Err("--realm requires --auth-mode long-term".to_string());
            }
            Ok(BenchAuth::disabled())
        }
        AuthModeArg::ShortTerm | AuthModeArg::LongTerm => {
            if args.auth_mode != AuthModeArg::LongTerm && args.realm.is_some() {
                return Err("--realm requires --auth-mode long-term".to_string());
            }
            let realm = match args.realm.as_deref() {
                Some("") => return Err("--realm must not be empty".to_string()),
                realm => realm,
            };
            let username = args
                .username
                .as_deref()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    "--username is required when authentication is enabled".to_string()
                })?;
            let password = args
                .password
                .as_deref()
                .or(env_password)
                .filter(|secret| !secret.is_empty())
                .ok_or_else(|| {
                    "--password or STUNBENCH_PASSWORD is required when authentication is enabled"
                        .to_string()
                })?;
            Ok(match args.auth_mode {
                AuthModeArg::LongTerm => match realm {
                    Some(realm) => BenchAuth::long_term_with_realm(username, password, realm),
                    None => BenchAuth::long_term(username, password),
                },
                _ => BenchAuth::short_term(username, password),
            })
        }
    }
}

/// Warn that `--password` leaks through process listings when debug logging is on.
fn password_warning(args: &Args) -> Option<String> {
    if !args.debug || args.password.is_none() {
        return None;
    }
    Some("--password is visible in process listings; prefer STUNBENCH_PASSWORD instead".to_string())
}

/// Initialize console logging with the same format as the server.
fn init_console_logging() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error"))
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
                record.level(),
                record.args()
            )
        })
        .init();
}

/// Resolve the RUST_LOG level from CLI flags; trace wins over debug.
fn requested_log_level(args: &Args) -> Option<&'static str> {
    if args.trace {
        Some("trace")
    } else if args.debug {
        Some("debug")
    } else {
        None
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
    fn test_trace_flag_parsed() {
        let args = parse(&["stunbench", "127.0.0.1", "--qps", "10", "--trace"]);
        assert!(args.trace);
    }

    #[test]
    fn test_level_trace_only() {
        let args = parse(&["stunbench", "127.0.0.1", "--qps", "10", "--trace"]);
        assert_eq!(requested_log_level(&args), Some("trace"));
    }

    #[test]
    fn test_level_debug_only() {
        let args = parse(&["stunbench", "127.0.0.1", "--qps", "10", "--debug"]);
        assert_eq!(requested_log_level(&args), Some("debug"));
    }

    #[test]
    fn test_level_trace_wins_over_debug() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--debug",
            "--trace",
        ]);
        assert_eq!(requested_log_level(&args), Some("trace"));
    }

    #[test]
    fn test_level_unset_without_flags() {
        let args = parse(&["stunbench", "127.0.0.1", "--qps", "10"]);
        assert_eq!(requested_log_level(&args), None);
    }

    #[test]
    fn test_fingerprint_flag_parsed() {
        let args = parse(&["stunbench", "127.0.0.1", "--qps", "10", "--fingerprint"]);
        assert!(args.fingerprint);
    }

    #[test]
    fn test_fingerprint_defaults_off() {
        let args = parse(&["stunbench", "127.0.0.1", "--qps", "10"]);
        assert!(!args.fingerprint);
    }

    #[test]
    fn test_help_documents_fingerprint() {
        let help = Args::command().render_help().to_string();
        assert!(help.contains("--fingerprint"));
        assert!(help.contains("FINGERPRINT"));
    }

    #[test]
    fn test_help_documents_trace() {
        let help = Args::command().render_help().to_string();
        assert!(help.contains("--trace"));
        assert!(help.contains("prints raw packet bytes"));
    }

    #[test]
    fn test_auth_mode_defaults_to_disabled() {
        let args = parse(&["stunbench", "127.0.0.1", "--qps", "10"]);
        assert_eq!(args.auth_mode, AuthModeArg::Disabled);
    }

    #[test]
    fn test_auth_mode_accepts_short_term_and_long_term() {
        let short = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--auth-mode",
            "short-term",
        ]);
        assert_eq!(short.auth_mode, AuthModeArg::ShortTerm);
        let long = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--auth-mode",
            "long-term",
        ]);
        assert_eq!(long.auth_mode, AuthModeArg::LongTerm);
    }

    #[test]
    fn test_auth_mode_rejects_unknown_value() {
        assert!(Args::try_parse_from([
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--auth-mode",
            "bogus",
        ])
        .is_err());
    }

    #[test]
    fn test_resolve_auth_requires_username() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--auth-mode",
            "short-term",
            "--password",
            "s3cret",
        ]);
        assert!(resolve_auth(&args, None).is_err());
    }

    #[test]
    fn test_resolve_auth_requires_password() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--auth-mode",
            "long-term",
            "--username",
            "alice",
        ]);
        assert!(resolve_auth(&args, None).is_err());
    }

    #[test]
    fn test_resolve_auth_prefers_password_flag_over_env() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--auth-mode",
            "short-term",
            "--username",
            "alice",
            "--password",
            "from-flag",
        ]);
        let auth = resolve_auth(&args, Some("from-env")).unwrap();
        assert_eq!(auth.mode(), bench::AuthMode::ShortTerm);
        assert_eq!(auth.username(), Some("alice"));
        assert_eq!(auth.password(), Some("from-flag"));
    }

    #[test]
    fn test_resolve_auth_reads_password_from_env() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--auth-mode",
            "long-term",
            "--username",
            "alice",
        ]);
        let auth = resolve_auth(&args, Some("from-env")).unwrap();
        assert_eq!(auth.mode(), bench::AuthMode::LongTerm);
        assert_eq!(auth.password(), Some("from-env"));
    }

    #[test]
    fn test_resolve_auth_rejects_credentials_with_disabled() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--username",
            "alice",
            "--password",
            "s3cret",
        ]);
        assert!(resolve_auth(&args, None).is_err());
    }

    #[test]
    fn test_resolve_auth_disabled_has_no_credentials() {
        let args = parse(&["stunbench", "127.0.0.1", "--qps", "10"]);
        let auth = resolve_auth(&args, None).unwrap();
        assert_eq!(auth.mode(), bench::AuthMode::Disabled);
        assert_eq!(auth.username(), None);
        assert_eq!(auth.password(), None);
    }

    #[test]
    fn test_password_warning_only_with_debug_flag() {
        let plain = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--password",
            "s3cret",
        ]);
        assert!(password_warning(&plain).is_none());

        let debug = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--debug",
            "--password",
            "s3cret",
        ]);
        assert!(password_warning(&debug).is_some());
    }

    #[test]
    fn test_help_documents_auth_options() {
        let help = Args::command().render_help().to_string();
        assert!(help.contains("--auth-mode"));
        assert!(help.contains("--username"));
        assert!(help.contains("--password"));
        assert!(help.contains("STUNBENCH_PASSWORD"));
    }

    #[test]
    fn test_realm_flag_parsed() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--realm",
            "example.org",
        ]);
        assert_eq!(args.realm.as_deref(), Some("example.org"));
    }

    #[test]
    fn test_help_documents_realm() {
        let help = Args::command().render_help().to_string();
        assert!(help.contains("--realm"));
    }

    #[test]
    fn test_resolve_auth_rejects_realm_with_disabled() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--realm",
            "example.org",
        ]);
        let err = resolve_auth(&args, None).unwrap_err();
        assert_eq!(err, "--realm requires --auth-mode long-term");
    }

    #[test]
    fn test_resolve_auth_rejects_realm_with_short_term() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--auth-mode",
            "short-term",
            "--username",
            "alice",
            "--password",
            "s3cret",
            "--realm",
            "example.org",
        ]);
        let err = resolve_auth(&args, None).unwrap_err();
        assert_eq!(err, "--realm requires --auth-mode long-term");
    }

    #[test]
    fn test_resolve_auth_rejects_empty_realm() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--auth-mode",
            "long-term",
            "--username",
            "alice",
            "--password",
            "s3cret",
            "--realm",
            "",
        ]);
        let err = resolve_auth(&args, None).unwrap_err();
        assert_eq!(err, "--realm must not be empty");
    }

    #[test]
    fn test_resolve_auth_applies_long_term_realm_override() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--auth-mode",
            "long-term",
            "--username",
            "alice",
            "--password",
            "s3cret",
            "--realm",
            "example.org",
        ]);
        let auth = resolve_auth(&args, None).unwrap();
        assert_eq!(auth.mode(), bench::AuthMode::LongTerm);
        assert_eq!(auth.realm(), Some("example.org"));
    }

    #[test]
    fn test_resolve_auth_long_term_without_realm_has_no_override() {
        let args = parse(&[
            "stunbench",
            "127.0.0.1",
            "--qps",
            "10",
            "--auth-mode",
            "long-term",
            "--username",
            "alice",
            "--password",
            "s3cret",
        ]);
        let auth = resolve_auth(&args, None).unwrap();
        assert_eq!(auth.realm(), None);
    }
}
