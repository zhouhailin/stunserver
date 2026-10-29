# STUN Server

High-performance STUN (Session Traversal Utilities for NAT) server implementation in Rust, RFC 5389 compliant.

[![CI](https://github.com/yourusername/stunserver/actions/workflows/ci.yml/badge.svg)](https://github.com/yourusername/stunserver/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/yourusername/stunserver/branch/main/graph/badge.svg)](https://codecov.io/gh/yourusername/stunserver)

## Features

- **High Performance**: Async I/O with Tokio, supports 10,000+ QPS
- **RFC 5389 Compliant**: Full STUN protocol implementation
- **Dual-Stack**: IPv4 and IPv6 support
- **Flexible Configuration**: YAML config files with sensible defaults
- **Production Ready**: systemd integration, file logging with rotation
- **Cross-Platform**: Linux, macOS, Windows support

## Components

This workspace contains three executables:

- **stunserver**: High-performance STUN server
- **stunclient**: Command-line STUN client for testing
- **stunbench**: Benchmark tool for performance testing

## Quick Start

### Build from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/stunserver.git
cd stunserver

# Build all executables
cargo build --release

# Run tests
cargo test --all
```

### Run the Server

```bash
# Start with default configuration (0.0.0.0:3478)
./target/release/stunserver

# Start with custom configuration
./target/release/stunserver --config config/example.yaml

# Start on custom port
./target/release/stunserver --listen 0.0.0.0:8080
```

### Test with Client

```bash
# Send a binding request with IP and port
./target/release/stunclient 127.0.0.1:3478

# Send a binding request with IP only (defaults to port 3478)
./target/release/stunclient 127.0.0.1

# Send a binding request with domain name (DNS resolution, defaults to port 3478)
./target/release/stunclient stunserver2025.stunprotocol.org

# Send a binding request with domain name and explicit port
./target/release/stunclient stunserver2025.stunprotocol.org:3478

# With custom timeout
./target/release/stunclient --timeout 10 127.0.0.1:3478
```

The client supports:
- **IPv4 addresses**: `192.168.1.1` or `192.168.1.1:3478`
- **IPv6 addresses**: `[::1]` or `[::1]:3478`
- **Domain names**: `example.com` or `example.com:3478`
- **Default port**: 3478 (STUN standard port) when not specified
- **DNS resolution**: Automatically resolves domain names with fallback support
- **Quiet mode**: Use `--quiet` to suppress startup log

### Run Benchmark

```bash
# Benchmark at 10,000 QPS for 60 seconds
./target/release/stunbench --qps 10000 127.0.0.1:3478

# Benchmark with specific request count
./target/release/stunbench --qps 10000 --requests 100000 127.0.0.1:3478
```

## Configuration

The server supports YAML configuration files. See `config/example.yaml` for a complete example.

### Minimal Configuration

```yaml
server:
  listen: "0.0.0.0:3478"

logging:
  level: info
  output: console
```

### Production Configuration

```yaml
server:
  listen: "0.0.0.0:3478"

logging:
  level: info
  output: file
  file:
    path: "/var/log/stunserver/stunserver.log"
    max_size: "100MB"
    rotation: daily
    max_files: 7
```

## Systemd Integration

Install the systemd service for production deployments:

```bash
# Copy service file
sudo cp systemd/stunserver.service /etc/systemd/system/

# Copy configuration
sudo mkdir -p /etc/stunserver
sudo cp config/example.yaml /etc/stunserver/config.yaml

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable stunserver
sudo systemctl start stunserver

# Check status
sudo systemctl status stunserver
```

## Build Metadata

All executables display build metadata at startup in a concise, machine-readable format:

```
stunserver v0.1.0 commit=14991861ab505fc80ce23b5ad3651349ba138299 compiled=2026-10-29T18:59:21+08:00 started=2026-10-29T19:01:24+08:00
```

This includes:
- Application name and version (from Cargo.toml)
- Git commit ID (full 40-character hash)
- Compilation timestamp (RFC 3339 format with local timezone)
- Startup timestamp (RFC 3339 format with local timezone)
- "-dirty" suffix appended to version if uncommitted changes exist

Use `--quiet` flag to suppress startup log output.

### Version Information

All executables display version information in `--help` output:

```bash
$ ./target/release/stunserver --help
stunserver 0.1.0-dirty (commit 14991861ab505fc80ce23b5ad3651349ba138299, compiled 2026-10-29T18:59:21+08:00)

High-performance STUN server

Usage: stunserver [OPTIONS]
...
```

The `--version` flag also outputs version information:

```bash
$ ./target/release/stunserver --version
stunserver 0.1.0
```

## Development

### Project Structure

```
stunserver/
├── crates/
│   └── stun-protocol/  # STUN protocol library
├── src/
│   ├── server/         # STUN server binary
│   ├── client/         # STUN client binary
│   └── bench/          # Benchmark tool binary
├── config/             # Example configurations
├── systemd/            # systemd service files
└── .github/workflows/  # CI/CD workflows
```

### Testing

```bash
# Run all tests
cargo test --all

# Run with coverage (requires cargo-tarpaulin)
cargo tarpaulin --all

# Check formatting
cargo fmt --all -- --check

# Run linter
cargo clippy --all-targets --all-features -- -D warnings
```

### Code Coverage

The project maintains 95%+ test coverage. Coverage reports are generated automatically in CI.

## Documentation

- [Release Process](docs/release.md)
- [Build Metadata](docs/build-metadata.md)
- [Configuration Reference](config/example.yaml)

## Performance

The server is designed for high performance:

- **Target**: 10,000+ QPS
- **Latency**: < 1ms p99 (local network)
- **Memory**: Minimal footprint with pre-allocated buffers
- **Concurrency**: Async I/O with Tokio runtime

Run the benchmark tool to test performance on your system:

```bash
./target/release/stunbench --qps 10000 --duration 60 127.0.0.1:3478
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow and guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- RFC 5389: Session Traversal Utilities for NAT (STUN)
- Tokio: Async runtime for Rust
- The Rust community for excellent tooling and libraries
