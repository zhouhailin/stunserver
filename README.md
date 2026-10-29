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
# Send a binding request to the server
./target/release/stunclient 127.0.0.1:3478

# With custom timeout
./target/release/stunclient --timeout 10 127.0.0.1:3478
```

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

All executables display build metadata at startup:

```
STUN Server v0.1.0
Commit: 84b9fd3
Commit Time: 2026-10-29T14:09:36+08:00 (UTC)
Build Time: 2026-10-29T07:08:26.866368193+00:00 (UTC)
Startup Time: 2026-10-29T07:08:27.302703918+00:00 (UTC)
Startup Time: 2026-10-29T15:08:27.302737453+08:00 (+08:00)
```

This includes:
- Application version (from Cargo.toml)
- Git commit ID (short hash)
- Git commit timestamp
- Build timestamp
- Startup time (UTC and local timezone)

## Development

### Project Structure

```
stunserver/
├── stun-protocol/      # STUN protocol library
├── stunserver/         # STUN server binary
├── stunclient/         # STUN client binary
├── stunbench/          # Benchmark tool binary
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
