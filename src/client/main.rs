//! STUN Client
//!
//! Command-line STUN client for testing server functionality.
//! Sends a STUN Binding Request and displays the mapped address.

use chrono::Local;
use clap::{Parser, ValueEnum};
use std::io::Write;
use std::process;
use std::time::Duration;
use stunclient::{
    address::{AddressFamily, NetworkAddress},
    client,
    metadata::BuildMetadata,
};

/// Credential mode requested on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum AuthModeArg {
    Disabled,
    ShortTerm,
    LongTerm,
}

impl From<AuthModeArg> for client::AuthMode {
    fn from(mode: AuthModeArg) -> Self {
        match mode {
            AuthModeArg::Disabled => client::AuthMode::Disabled,
            AuthModeArg::ShortTerm => client::AuthMode::ShortTerm,
            AuthModeArg::LongTerm => client::AuthMode::LongTerm,
        }
    }
}

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

    /// Enable debug logging (prints network events).
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

    /// Credential mode: disabled, short-term, or long-term.
    #[arg(long, value_enum, default_value = "disabled")]
    auth_mode: AuthModeArg,

    /// Username for short-term or long-term credentials.
    #[arg(long)]
    username: Option<String>,

    /// Password for short-term or long-term credentials. Visible in process
    /// listings; prefer the STUNCLIENT_PASSWORD environment variable.
    #[arg(long)]
    password: Option<String>,
}

fn main() {
    let args = Args::parse();

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

    let auth = match resolve_auth(&args, std::env::var("STUNCLIENT_PASSWORD").ok().as_deref()) {
        Ok(auth) => auth,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };
    if let Some(warning) = password_warning(&args) {
        log::warn!("{}", warning);
    }

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
    let result = runtime.block_on(client::send_binding_request_with_fallback_and_auth(
        &resolved_addrs,
        timeout,
        &auth,
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
                client::ClientError::ServerError { .. } => process::exit(5),
                client::ClientError::InvalidAddress(_) => process::exit(1),
            }
        }
    }
}

/// Build client credentials from CLI flags plus the optional env-var password.
fn resolve_auth(args: &Args, env_password: Option<&str>) -> Result<client::ClientAuth, String> {
    match args.auth_mode {
        AuthModeArg::Disabled => {
            if args.username.is_some() || args.password.is_some() {
                return Err(
                    "--username/--password require --auth-mode short-term or long-term".to_string(),
                );
            }
            Ok(client::ClientAuth::disabled())
        }
        AuthModeArg::ShortTerm | AuthModeArg::LongTerm => {
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
                    "--password or STUNCLIENT_PASSWORD is required when authentication is enabled"
                        .to_string()
                })?;
            let mode = client::AuthMode::from(args.auth_mode);
            Ok(match mode {
                client::AuthMode::ShortTerm => client::ClientAuth::short_term(username, password),
                client::AuthMode::LongTerm => client::ClientAuth::long_term(username, password),
                client::AuthMode::Disabled => client::ClientAuth::disabled(),
            })
        }
    }
}

/// Warn that `--password` leaks through process listings when debug logging is on.
fn password_warning(args: &Args) -> Option<String> {
    if !args.debug || args.password.is_none() {
        return None;
    }
    Some(
        "--password is visible in process listings; prefer STUNCLIENT_PASSWORD instead".to_string(),
    )
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
        let args = parse(&["stunclient", "--trace"]);
        assert!(args.trace);
        assert!(!args.debug);
    }

    #[test]
    fn test_level_trace_only() {
        let args = parse(&["stunclient", "--trace"]);
        assert_eq!(requested_log_level(&args), Some("trace"));
    }

    #[test]
    fn test_level_debug_only() {
        let args = parse(&["stunclient", "--debug"]);
        assert_eq!(requested_log_level(&args), Some("debug"));
    }

    #[test]
    fn test_level_trace_wins_over_debug() {
        let args = parse(&["stunclient", "--debug", "--trace"]);
        assert_eq!(requested_log_level(&args), Some("trace"));
    }

    #[test]
    fn test_level_unset_without_flags() {
        let args = parse(&["stunclient"]);
        assert_eq!(requested_log_level(&args), None);
    }

    #[test]
    fn test_help_documents_trace() {
        let help = Args::command().render_help().to_string();
        assert!(help.contains("--trace"));
        assert!(help.contains("prints raw packet bytes"));
    }

    #[test]
    fn test_auth_mode_defaults_to_disabled() {
        let args = parse(&["stunclient"]);
        assert_eq!(args.auth_mode, AuthModeArg::Disabled);
    }

    #[test]
    fn test_auth_mode_accepts_short_term_and_long_term() {
        let short = parse(&["stunclient", "--auth-mode", "short-term"]);
        assert_eq!(short.auth_mode, AuthModeArg::ShortTerm);
        let long = parse(&["stunclient", "--auth-mode", "long-term"]);
        assert_eq!(long.auth_mode, AuthModeArg::LongTerm);
    }

    #[test]
    fn test_auth_mode_rejects_unknown_value() {
        assert!(Args::try_parse_from(["stunclient", "--auth-mode", "bogus"]).is_err());
    }

    #[test]
    fn test_resolve_auth_requires_username() {
        let args = parse(&[
            "stunclient",
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
            "stunclient",
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
            "stunclient",
            "--auth-mode",
            "short-term",
            "--username",
            "alice",
            "--password",
            "from-flag",
        ]);
        let auth = resolve_auth(&args, Some("from-env")).unwrap();
        assert_eq!(auth.mode(), client::AuthMode::ShortTerm);
        assert_eq!(auth.username(), Some("alice"));
        assert_eq!(auth.password(), Some("from-flag"));
    }

    #[test]
    fn test_resolve_auth_reads_password_from_env() {
        let args = parse(&[
            "stunclient",
            "--auth-mode",
            "long-term",
            "--username",
            "alice",
        ]);
        let auth = resolve_auth(&args, Some("from-env")).unwrap();
        assert_eq!(auth.mode(), client::AuthMode::LongTerm);
        assert_eq!(auth.password(), Some("from-env"));
    }

    #[test]
    fn test_resolve_auth_rejects_credentials_with_disabled() {
        let args = parse(&["stunclient", "--username", "alice", "--password", "s3cret"]);
        assert!(resolve_auth(&args, None).is_err());
    }

    #[test]
    fn test_resolve_auth_disabled_has_no_credentials() {
        let args = parse(&["stunclient"]);
        let auth = resolve_auth(&args, None).unwrap();
        assert_eq!(auth.mode(), client::AuthMode::Disabled);
        assert_eq!(auth.username(), None);
        assert_eq!(auth.password(), None);
    }

    #[test]
    fn test_debug_password_warning_mentions_process_listing_and_env_var() {
        let args = parse(&[
            "stunclient",
            "--debug",
            "--auth-mode",
            "short-term",
            "--username",
            "alice",
            "--password",
            "s3cret",
        ]);
        let warning = password_warning(&args).expect("warning should be emitted");
        assert!(warning.contains("process"));
        assert!(warning.contains("STUNCLIENT_PASSWORD"));

        let quiet_args = parse(&["stunclient"]);
        assert!(password_warning(&quiet_args).is_none());
    }

    #[test]
    fn test_help_documents_auth_options() {
        let help = Args::command().render_help().to_string();
        assert!(help.contains("--auth-mode"));
        assert!(help.contains("--username"));
        assert!(help.contains("--password"));
        assert!(help.contains("STUNCLIENT_PASSWORD"));
    }
}
