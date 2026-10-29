# Research: Rust STUN Server Project Initialization

**Date**: 2026-10-29 | **Feature**: 001-rust-stun-project-init

## Research Tasks

### 1. Tokio Async Runtime for UDP Server

**Decision**: Use Tokio 1.x with `tokio::net::UdpSocket` for async UDP operations.

**Rationale**: 
- Tokio is the de facto standard async runtime for Rust
- `UdpSocket` provides non-blocking UDP operations
- Supports dual-stack IPv4/IPv6 via `SocketAddr::V4` and `SocketAddr::V6`
- Proven performance for high-throughput network applications
- Excellent documentation and community support

**Alternatives Considered**:
- async-std: Less mature ecosystem, fewer networking libraries
- std::net::UdpSocket (blocking): Cannot achieve 10K QPS target
- mio: Lower-level, more complex, no significant benefit over Tokio

**Best Practices**:
- Use `tokio::spawn` for concurrent request handling
- Pre-allocate buffer pool to minimize allocations
- Use `Arc<SharedState>` for shared server state
- Implement graceful shutdown with `tokio::signal`

---

### 2. STUN Protocol Implementation (RFC 5389)

**Decision**: Implement STUN message parsing/building from scratch following RFC 5389 specification.

**Rationale**:
- STUN message format is relatively simple (20-byte header + attributes)
- No mature Rust STUN library with active maintenance
- Full control over protocol compliance and performance
- Easier to achieve 95% test coverage with custom implementation

**RFC 5389 Key Points**:
- Message format: 2-byte type, 2-byte length, 4-byte magic cookie, 12-byte transaction ID
- Attribute format: 2-byte type, 2-byte length, variable value
- MAPPED-ADDRESS and XOR-MAPPED-ADDRESS attributes for response
- Error codes: 400 (Bad Request), 401 (Unauthorized), 420 (Unknown Attribute), 500 (Server Error)

**Alternatives Considered**:
- stun-client crate: Limited functionality, not actively maintained
- coturn (C library): Requires FFI, adds complexity

**Implementation Approach**:
- Define message types as enums with serde serialization
- Use byteorder crate for network byte order conversion
- Implement attribute parsing as trait-based system for extensibility
- Comprehensive unit tests for message encoding/decoding

---

### 3. Command-Line Argument Parsing

**Decision**: Use clap 4.x with derive API for CLI argument parsing.

**Rationale**:
- Clap is the standard CLI library for Rust
- Derive API reduces boilerplate
- Built-in help generation and validation
- Supports subcommands if needed in future

**CLI Structure**:
```rust
// stunserver
#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    config: Option<PathBuf>,
    
    #[arg(short, long, default_value = "0.0.0.0:3478")]
    listen: SocketAddr,
}

// stunclient
#[derive(Parser)]
struct Args {
    #[arg()]
    server: SocketAddr,
    
    #[arg(short, long, default_value = "5")]
    timeout: u64,
}

// stunbench
#[derive(Parser)]
struct Args {
    #[arg()]
    server: SocketAddr,
    
    #[arg(short, long)]
    qps: u32,
    
    #[arg(short, long, default_value = "60")]
    duration: u64,
}
```

**Alternatives Considered**:
- structopt: Merged into clap 3.x, no longer maintained separately
- argh: Less feature-rich, smaller binary size (not critical for this project)

---

### 4. Configuration Management

**Decision**: Use serde + serde_yaml for YAML configuration parsing.

**Rationale**:
- YAML is human-readable and DevOps-friendly (per user requirement)
- serde is the standard serialization framework for Rust
- Type-safe configuration with derive macros
- Supports complex nested structures

**Configuration Schema**:
```yaml
# stunserver.yaml
server:
  listen: "0.0.0.0:3478"
  
logging:
  level: "info"  # error, warn, info, debug
  output: "both"  # console, file, both
  file:
    path: "/var/log/stunserver/stunserver.log"
    max_size: "100MB"
    rotation: "daily"
    max_files: 7
```

**Alternatives Considered**:
- TOML: Rust ecosystem standard, but user specified YAML
- JSON: Less human-friendly for manual editing
- config crate: Adds abstraction layer, not necessary for this use case

---

### 5. Logging Framework

**Decision**: Use log facade + env_logger for console, custom file logger for file output.

**Rationale**:
- log is the standard logging facade in Rust
- env_logger provides flexible console logging with environment variable control
- File logging requires custom implementation for rotation and size limits
- Separation of concerns: facade (log) vs implementation (env_logger/custom)

**Implementation Approach**:
- Use `log::{error, warn, info, debug}` macros throughout code
- Initialize env_logger for console output
- Implement custom file logger with:
  - File size checking and rotation
  - Date-based file naming (stunserver-2026-10-29.log)
  - Maximum file count enforcement (delete oldest)
  - Buffered writes for performance

**Alternatives Considered**:
- tracing: More advanced (spans, structured logging), but overkill for this project
- slog: Structured logging, but steeper learning curve
- log4rs: Full-featured but heavy dependencies

---

### 6. Build Metadata Embedding

**Decision**: Use build.rs build script to capture git information and embed via environment variables.

**Rationale**:
- build.rs runs before compilation and can execute shell commands
- Environment variables set in build.rs are available via `env!()` macro
- Standard approach for embedding build metadata in Rust

**Implementation**:
```rust
// build.rs
use std::process::Command;

fn main() {
    // Get git commit hash
    let output = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .expect("Failed to get git commit");
    let commit_id = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=GIT_COMMIT_ID={}", commit_id.trim());
    
    // Get commit timestamp
    let output = Command::new("git")
        .args(&["log", "-1", "--format=%cd", "--date=iso-strict"])
        .output()
        .expect("Failed to get commit time");
    let commit_time = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=GIT_COMMIT_TIME={}", commit_time.trim());
    
    // Get build timestamp (UTC)
    let build_time = chrono::Utc::now().to_rfc3339();
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
    
    // Get version from Cargo.toml
    let version = env!("CARGO_PKG_VERSION");
    println!("cargo:rustc-env=APP_VERSION={}", version);
}
```

**Usage in code**:
```rust
fn print_build_info() {
    println!("Version: {}", env!("APP_VERSION"));
    println!("Commit: {}", env!("GIT_COMMIT_ID"));
    println!("Commit Time: {}", env!("GIT_COMMIT_TIME"));
    println!("Build Time: {}", env!("BUILD_TIME"));
    println!("Startup Time: {}", chrono::Utc::now().to_rfc3339());
    println!("Timezone: {}", get_system_timezone());
}
```

**Alternatives Considered**:
- vergen: More feature-rich but adds dependency
- Manual embedding: More control but more boilerplate

---

### 7. GitHub Actions CI/CD

**Decision**: Use GitHub Actions with cross-compilation via cross-rs for multi-platform builds.

**Rationale**:
- GitHub Actions is native to GitHub, no external service needed
- cross-rs simplifies cross-compilation with Docker
- Supports all target platforms (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64)
- Free for public repositories

**Workflow Structure**:
```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
      - run: cargo test -- --coverage  # Generate coverage report

# .github/workflows/release.yml
name: Release
on:
  push:
    tags:
      - 'v*'
jobs:
  build:
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-gnu
          - aarch64-unknown-linux-gnu
          - x86_64-apple-darwin
          - aarch64-apple-darwin
          - x86_64-pc-windows-gnu
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/cross@v1
        with:
          target: ${{ matrix.target }}
          args: --release
      - uses: actions/upload-artifact@v3
        with:
          name: binaries-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/
```

**Alternatives Considered**:
- Travis CI: Less integrated with GitHub, slower
- CircleCI: More complex configuration
- Native cross-compilation: More setup, less reliable than cross-rs

---

### 8. Systemd Service Integration

**Decision**: Provide systemd service file with standard Linux service conventions.

**Rationale**:
- systemd is the standard init system for modern Linux distributions
- Service file enables standard systemctl commands (start, stop, restart, status)
- Integrates with systemd journal for logging
- Production deployment requirement (FR-007)

**Service File**:
```ini
# systemd/stunserver.service
[Unit]
Description=STUN Server
After=network.target

[Service]
Type=simple
User=stunserver
Group=stunserver
ExecStart=/usr/local/bin/stunserver --config /etc/stunserver/config.yaml
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=stunserver

[Install]
WantedBy=multi-user.target
```

**Key Points**:
- Type=simple for foreground process (server runs in foreground)
- StandardOutput=journal sends logs to systemd journal
- Restart=on-failure for automatic recovery
- User/Group for security (run as non-root)

---

### 9. Test Coverage Measurement

**Decision**: Use cargo-tarpaulin for test coverage measurement.

**Rationale**:
- cargo-tarpaulin is the standard coverage tool for Rust
- Supports line, branch, and function coverage
- Integrates with CI pipelines
- Can generate reports in multiple formats (HTML, JSON, LCOV)

**Usage**:
```bash
# Install
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --out Html

# CI integration
cargo tarpaulin --out Xml --output-dir coverage/
```

**Target**: 95%+ coverage enforced via CI check

**Alternatives Considered**:
- grcov: More complex setup, requires LLVM source-based coverage
- tarpaulin: Simpler, sufficient for this project

---

### 10. Error Handling Strategy

**Decision**: Use anyhow for application-level errors, custom error types for library code.

**Rationale**:
- anyhow simplifies error handling in applications
- Custom error types in stun-protocol library for precise error handling
- thiserror for deriving custom error types
- Separation: library (precise errors) vs application (convenient errors)

**Implementation**:
```rust
// In stun-protocol library
#[derive(Debug, thiserror::Error)]
pub enum StunError {
    #[error("Invalid message type: {0}")]
    InvalidMessageType(u16),
    
    #[error("Missing required attribute: {0}")]
    MissingAttribute(String),
    
    #[error("Malformed message: {0}")]
    MalformedMessage(String),
}

// In stunserver application
use anyhow::{Context, Result};

fn load_config(path: &Path) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .context("Failed to read configuration file")?;
    let config: Config = serde_yaml::from_str(&content)
        .context("Failed to parse configuration")?;
    Ok(config)
}
```

**Alternatives Considered**:
- Only anyhow: Less precise error handling in library
- Only custom errors: More boilerplate in application code

---

## Summary of Decisions

| Area | Decision | Key Benefit |
|------|----------|-------------|
| Async Runtime | Tokio 1.x | High performance, proven |
| STUN Protocol | Custom implementation | Full control, testable |
| CLI Parsing | clap 4.x derive | Standard, low boilerplate |
| Configuration | serde + serde_yaml | Type-safe, YAML format |
| Logging | log + env_logger + custom | Flexible, meets requirements |
| Build Metadata | build.rs + env vars | Standard approach |
| CI/CD | GitHub Actions + cross-rs | Native, multi-platform |
| Systemd | Service file | Production deployment |
| Coverage | cargo-tarpaulin | 95%+ enforcement |
| Error Handling | anyhow + thiserror | Application + library |

All research tasks complete. Ready for Phase 1: Design & Contracts.
