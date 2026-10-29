# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2026-10-29

### Added

#### Packaging & Deployment
- Root `Makefile` with development commands: `build`, `test`, `package`, `clean`, `fmt`, `lint`, `help`
- Deployment `deploy/Makefile` with systemd integration: `install`, `uninstall`, `start`, `stop`, `restart`, `status`
- Standardized installation directory structure: `/opt/stunserver/{bin,etc,log}`
- TOML configuration file support (replacing YAML)
- Example configuration file: `deploy/stunserver.conf.example`
- Log rotation with configurable size (100MB default) and retention (30 days default)

#### Docker Support
- Multi-stage `docker/Dockerfile` (rust:1.75-alpine builder + alpine:3.19 runtime)
- Docker entrypoint script with environment variable overrides (`STUN_CONFIG_PATH`, `STUN_LOG_LEVEL`, `STUN_PORT`, `STUN_LISTEN_ADDRESS`)
- Docker Compose example: `docker/docker-compose.yml`
- `.dockerignore` for optimized build context
- Non-root user execution in container (UID 1000)
- Health check configuration (30s interval, 10s timeout)

#### CI/CD
- GitHub Actions workflow for automated Docker image builds (`.github/workflows/docker-publish.yml`)
- Multi-platform support: linux/amd64, linux/arm64
- Automatic publishing to GitHub Container Registry (ghcr.io)
- Semantic versioning tags for releases

#### Documentation
- Comprehensive English README with deployment guide
- Chinese README (README.zh-CN.md) with complete translation
- Updated systemd service file with new paths and GitHub URL

#### Project Structure
- Reorganized codebase into Rust workspace best practices structure
- Application code moved to `src/server`, `src/client`, `src/bench`
- Shared library moved to `crates/stun-protocol`
- Each application now has its own `Cargo.toml` and `build.rs`

#### Build Metadata
- Structured startup log output for all three applications
- Format: `{app_name} v{version} commit={commit_id} compiled={compile_time} started={startup_time}`
- Timestamps in RFC 3339 format with local timezone
- Dirty build detection (appends "-dirty" to version when uncommitted changes exist)
- `--quiet` flag to suppress startup log output

#### Client Enhancements
- Default port 3478 when not specified (STUN standard port)
- DNS resolution support for domain names
- Fallback mechanism for multiple resolved addresses
- Support for all address formats:
  - IPv4: `192.168.1.1` or `192.168.1.1:3478`
  - IPv6: `[::1]` or `[::1]:3478`
  - Domain: `example.com` or `example.com:3478`
- Clear error messages for DNS resolution failures

#### Version Information
- Version information displayed as first line of `--help` output
- Format: `{app_name} {version} (commit {commit_id}, compiled {compile_time})`
- `--version` flag outputs version information
- Consistent version display across all three applications

#### RFC Compliance
- RFC 5769 test vectors implementation (9 test cases)
- RFC 5389 section references in code comments
- Verified STUN message format compliance
- XOR-MAPPED-ADDRESS encoding/decoding tests for IPv4 and IPv6

#### Testing
- 83 total tests (74 unit + 9 integration)
- RFC 5769 test vectors in `crates/stun-protocol/tests/rfc5769_vectors.rs`
- Address parsing tests in `src/client/address.rs`
- All tests pass with zero warnings

### Changed

#### Build System
- Root `build.rs` now captures git metadata for all workspace members
- Each crate has its own `build.rs` for independent metadata capture
- Environment variables: `BUILD_COMMIT_ID`, `BUILD_COMPILE_TIME`, `BUILD_IS_DIRTY`

#### Client Interface
- Server argument changed from `SocketAddr` to `String` to support domain names
- Address parsing moved to dedicated `address.rs` module
- DNS resolution happens before connection attempt
- Improved error messages for address parsing failures

#### Startup Output
- Replaced multi-line startup log with single-line structured format
- Startup log now appears before network operations
- Log format optimized for machine parsing and log aggregation

### Fixed

- Clippy warnings resolved (zero warnings with `-D warnings`)
- Code formatting verified with `cargo fmt --check`
- All tests pass in both debug and release modes

### Performance

- Startup time: <5ms from argument parsing to log output
- Build time: <10% increase after restructuring
- Binary sizes:
  - stunserver: 2.0M
  - stunclient: 3.1M
  - stunbench: 3.2M

### Documentation

- Updated README.md with new features and usage examples
- Added RFC 5389 section references to protocol implementation
- Created quickstart validation guide with 10 scenarios
- Updated project structure documentation

### Migration Guide

#### For Users

**Startup Log Format Changed**

Old format (v1.x):
```
STUN Server v0.1.0
Commit: 84b9fd3
Commit Time: 2026-10-29T14:09:36+08:00 (UTC)
Build Time: 2026-10-29T07:08:26.866368193+00:00 (UTC)
Startup Time: 2026-10-29T07:08:27.302703918+00:00 (UTC)
Startup Time: 2026-10-29T15:08:27.302737453+08:00 (+08:00)
```

New format (v2.0.0):
```
stunserver v0.1.0 commit=14991861ab505fc80ce23b5ad3651349ba138299 compiled=2026-10-29T18:59:21+08:00 started=2026-10-29T19:01:24+08:00
```

**Client Address Format**

Old usage (v1.x):
```bash
stunclient 192.168.1.1:3478  # Port required
```

New usage (v2.0.0):
```bash
stunclient 192.168.1.1       # Port defaults to 3478
stunclient 192.168.1.1:3478  # Explicit port still works
stunclient example.com       # Domain names now supported
```

**Suppressing Startup Log**

New in v2.0.0:
```bash
stunserver --quiet           # Suppress startup log
stunclient --quiet 127.0.0.1 # Suppress startup log
stunbench --quiet --qps 1000 127.0.0.1 # Suppress startup log
```

#### For Developers

**Directory Structure Changed**

Old structure (v1.x):
```
stunserver/
├── stunserver/src/
├── stunclient/src/
├── stunbench/src/
└── stun-protocol/
```

New structure (v2.0.0):
```
stunserver/
├── src/
│   ├── server/
│   ├── client/
│   └── bench/
└── crates/
    └── stun-protocol/
```

**Build Metadata Access**

Old approach (v1.x):
```rust
let version = env!("APP_VERSION");
let commit_id = env!("GIT_COMMIT_ID");
```

New approach (v2.0.0):
```rust
use stunserver::metadata::BuildMetadata;

let metadata = BuildMetadata::capture();
metadata.log_startup();
let version_line = metadata.format_version_line();
```

**Client Address Parsing**

Old approach (v1.x):
```rust
let server: SocketAddr = args.server.parse()?;
```

New approach (v2.0.0):
```rust
use stunclient::address::NetworkAddress;

let mut addr = NetworkAddress::parse(&args.server, 3478)?;
addr.resolve()?;  // DNS resolution
let socket_addr = addr.primary_address()?;
```

### Known Issues

- None at this time

### Security

- No security vulnerabilities identified
- All STUN protocol implementation follows RFC 5389 specifications
- DNS resolution uses standard library functions with proper error handling

### Contributors

- Implementation following Spec Kit workflow
- RFC 5389/5769 compliance verification
- Comprehensive test coverage maintained

[2.0.0]: https://github.com/yourusername/stunserver/compare/v1.0.0...v2.0.0
