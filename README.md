# STUN Server

A high-performance STUN (Session Traversal Utilities for NAT) server implementation in Rust, compliant with RFC 5389.

[中文文档](README.zh-CN.md)

## Features

- 🚀 **High Performance**: Built with Rust and Tokio for async I/O
- 🔒 **RFC 5389 Compliant**: Full STUN protocol implementation
- 📦 **Easy Deployment**: Standardized packaging with systemd integration
- 🐳 **Docker Support**: Multi-stage builds with minimal image size
- 🔄 **CI/CD Ready**: GitHub Actions workflow for automated builds
- 📝 **Comprehensive Logging**: Built-in log rotation and management
- 📋 **RFC Conformance**: Traceable audit matrix against RFC 5389/5769/5780 in [docs/rfc-conformance.md](docs/rfc-conformance.md)

## Protocol Scope

This server implements STUN Binding (RFC 5389) over UDP. It is a single-address
server: it offers no NAT behavior discovery, and no response ever includes
OTHER-ADDRESS or RESPONSE-ORIGIN. TCP, TLS, and legacy RFC 3489 handling are out
of scope.

## Quick Start

### Prerequisites

- Rust 1.83+ (for building from source)
- Linux system with systemd (for deployment)
- Docker 20.10+ (for container deployment)

### Build from Source

```bash
# Clone the repository
git clone https://github.com/zhouhailin/stunserver.git
cd stunserver

# Build release binary
make build

# Run tests
make test

# Create distribution package
make package
```

### Installation (Linux with systemd)

```bash
# Build first
make build

# Install (requires root; also installs /etc/sysctl.d/99-stunserver.conf)
cd deploy
sudo make install

# Start the service
sudo make start

# Check status
sudo make status
```

`make install` also applies the kernel UDP buffer limits from
`systemd/99-stunserver.conf`; see [System Tuning](#system-tuning) for the values,
verification, and container notes.

### Docker Deployment

```bash
# Build Docker image
docker build -t stunserver:latest -f docker/Dockerfile .

# Run container
docker run -d \
  --name stunserver \
  -p 3478:3478/udp \
  -p 3478:3478/tcp \
  -v $(pwd)/logs:/opt/stunserver/log \
  stunserver:latest
```

## Configuration

The server uses a TOML configuration file located at `/opt/stunserver/etc/stunserver.conf`.

### Example Configuration

```toml
[server]
listen_address = "0.0.0.0"
port = 3478
worker_threads = 0  # 0 = auto-detect CPU cores
max_connections = 10000  # max in-flight request handlers; excess datagrams are dropped

[logging]
level = "info"
log_dir = "/opt/stunserver/log"
file_prefix = "stunserver"
max_file_size_mb = 100
retention_days = 30
enable_console = true
compress_rotated_logs = true  # gzip rotated files into *.gz

[network]
udp_recv_buffer_size = 1048576
udp_send_buffer_size = 1048576
tcp_connection_timeout_sec = 30
tcp_idle_timeout_sec = 300
```

`udp_recv_buffer_size` and `udp_send_buffer_size` are applied as `SO_RCVBUF`/`SO_SNDBUF`
when the socket is created. The kernel silently caps them at `net.core.rmem_max` and
`net.core.wmem_max`; see [System Tuning](#system-tuning) for the recommended limits,
persistence, and the systemd resource settings.

### Environment Variables (Docker)

When running in Docker, you can override configuration with environment variables:

- `STUN_CONFIG_PATH`: Configuration file path (default: `/opt/stunserver/etc/stunserver.conf`)
- `STUN_LOG_LEVEL`: Log level (trace, debug, info, warn, error)
- `STUN_PORT`: Listening port (default: 3478)
- `STUN_LISTEN_ADDRESS`: Listen address (default: 0.0.0.0)

### Authentication

The server supports `disabled` (default), `short-term`, and `long-term` credentials through the `[auth]` section. An enabled mode never falls back to unauthenticated success.

```toml
[auth]
mode = "short-term"   # "disabled", "short-term", or "long-term"

[[auth.users]]
username = "alice"
password = "change-me"

# long-term only: realm is required; nonce_lifetime_sec defaults to 600
# mode = "long-term"
# realm = "example.org"
# nonce_lifetime_sec = 600
```

With `long-term`, the client first receives an unsigned 401 challenge carrying REALM and NONCE and then retries with a signed request. A stale NONCE yields 438 with a fresh NONCE, which the client adopts for at most two signed retries; a 401 after a signed request is terminal because retrying without changing credentials is forbidden by RFC 5389 §10.2.3. `nonce_lifetime_sec` must be between 1 and 86400.

USERNAME, REALM, and passwords are processed with SASLprep (RFC 5389 §15.3, §15.4, §15.7); USERNAME is limited to 512 bytes and REALM/NONCE to fewer than 128 characters. The bundled UDP client retransmits requests with a 500 ms initial RTO that doubles for at most seven transmissions, then waits 16×RTO (8 s) for the final response; `--timeout` is a hard cap on the whole exchange (§7.2.1). Responses from another source, with a mismatched transaction ID, or failing validation are silently discarded while the exchange waits for a valid response (§7.3.3, §10.1.3). A 400 error is accepted without MESSAGE-INTEGRITY because the server may not be able to compute one (§15.6). REALM/NONCE caching (§10.2.1.2) and RTO caching per server (§7.2.1) are not applicable: `stunclient` performs a single discovery exchange per invocation.

The configuration file stores passwords in plaintext, so restrict it to the service account:

```bash
chmod 600 /opt/stunserver/etc/stunserver.conf
```

Never commit production credentials. For the bundled client, prefer `STUNCLIENT_PASSWORD` over `--password`, because command-line arguments are visible in process listings:

```bash
STUNCLIENT_PASSWORD='change-me' stunclient --auth-mode long-term --username alice stun.example.org:3478
```

`--realm` overrides the REALM proposed by a long-term challenge for diagnostics; the challenge NONCE is still used. A live server derives its key from its configured realm, so a mismatched override normally produces 401.

MESSAGE-INTEGRITY provides integrity only; UDP traffic is not encrypted.

## System Tuning

High packet rates need kernel-side headroom in addition to the `[network]` settings. The
recommended values are shipped in `systemd/99-stunserver.conf` and installed to
`/etc/sysctl.d/99-stunserver.conf` by `deploy/Makefile`:

| Parameter | Value | Purpose |
|-----------|-------|---------|
| `net.core.rmem_max` | `16777216` | Upper bound for `SO_RCVBUF`; without it the configured `udp_recv_buffer_size` is silently capped (often at 212992 bytes, a few milliseconds of traffic at tens of thousands of packets per second). |
| `net.core.wmem_max` | `16777216` | Upper bound for `SO_SNDBUF`; same cap for `udp_send_buffer_size`. |
| `net.core.netdev_max_backlog` | `16384` | Queue between the NIC driver and the network stack; enlarge it when bursts are dropped in the "dropped" column of `/proc/net/softnet_stat`. |

Apply and verify:

```bash
sudo cp systemd/99-stunserver.conf /etc/sysctl.d/99-stunserver.conf
sudo sysctl --system
sysctl net.core.rmem_max net.core.wmem_max net.core.netdev_max_backlog
```

The kernel accounts roughly twice the requested socket size internally, so a 4 MiB request
is bounded by `2 x net.core.rmem_max` / `2 x net.core.wmem_max`. In containers these limits
come from the host kernel: set them on the host (containers share the network namespace
settings unless a separate netns is configured). `stunbench` requests 4 MiB buffers for its
own UDP socket and is subject to the same caps.

The systemd unit (`systemd/stunserver.service`) adds `LimitNOFILE=1048576` and writes to the
journal. Kernel limits are global, so restart the service after changing the sysctl drop-in
or `[network]` buffer sizes to recreate the listening socket with the new values:

```bash
sudo systemctl restart stunserver
```

Removing `/etc/sysctl.d/99-stunserver.conf` stops future boots from raising the limits, but
already running sessions keep the current values until reboot.

### Example: 50k QPS benchmark

With the values above applied and `udp_recv_buffer_size`/`udp_send_buffer_size` set to
1048576, `stunbench --qps 50000` completed 3,000,000 requests with 0 failures and
p99 = 0.92 ms on a loopback test.

## Directory Structure

After installation, the server uses the following directory structure:

```
/opt/stunserver/
├── bin/
│   └── stunserver              # Binary executable
├── etc/
│   └── stunserver.conf         # Configuration file
└── log/
    ├── stunserver.log                    # Current log file
    └── stunserver.log.2026-10-31.1       # Rotated log file
```

## Management Commands

### Development Commands (Root Makefile)

```bash
make build      # Build release binary
make test       # Run all tests
make package    # Create distribution package
make clean      # Clean build artifacts
make fmt        # Format code
make lint       # Run clippy lints
make help       # Show help
```

### Deployment Commands (deploy/Makefile)

```bash
sudo make install      # Install and enable service
sudo make uninstall    # Remove service and files
sudo make start        # Start service
sudo make stop         # Stop service
sudo make restart      # Restart service
sudo make status       # Show service status
sudo make help         # Show help
```

## Docker Images

### Automated Builds

Docker images are automatically built and published to GitHub Container Registry (ghcr.io) via GitHub Actions:

- **Latest**: `ghcr.io/zhouhailin/stunserver:latest` (from main branch)
- **Version tags**: `ghcr.io/zhouhailin/stunserver:v0.1.1` (from releases)

### Pull and Run

```bash
# Pull latest image
docker pull ghcr.io/zhouhailin/stunserver:latest

# Run with custom configuration
docker run -d \
  --name stunserver \
  -p 3478:3478/udp \
  -p 3478:3478/tcp \
  -v $(pwd)/config/stunserver.conf:/opt/stunserver/etc/stunserver.conf:ro \
  -v $(pwd)/logs:/opt/stunserver/log \
  ghcr.io/zhouhailin/stunserver:latest
```

### Docker Compose

```yaml
version: '3.8'

services:
  stunserver:
    image: ghcr.io/zhouhailin/stunserver:latest
    container_name: stunserver
    ports:
      - "3478:3478/udp"
      - "3478:3478/tcp"
    volumes:
      - ./config/stunserver.conf:/opt/stunserver/etc/stunserver.conf:ro
      - ./logs:/opt/stunserver/log
    environment:
      - STUN_LOG_LEVEL=info
    restart: unless-stopped
```

## Log Rotation

The server includes built-in log rotation with the following features:

- **Size-based rotation**: Logs rotate when reaching 100MB (configurable)
- **Time-based rotation**: Daily rotation at midnight
- **Retention**: Keeps logs for 30 days (configurable)
- **Automatic cleanup**: Old logs are automatically deleted

Log files are named:
- `stunserver.log` - Current active log
- `stunserver.log.YYYY-MM-DD.1` - First rotated log of that day
- `stunserver.log.YYYY-MM-DD.2` - Second rotated log of that day
- ...

Sequence numbers start at 1 each day, are not zero-padded, and existing files are never overwritten.

## Troubleshooting

### Service won't start

```bash
# Check service status
sudo systemctl status stunserver

# View logs
sudo journalctl -u stunserver -n 50

# Check configuration
cat /opt/stunserver/etc/stunserver.conf
```

### Port already in use

```bash
# Check what's using port 3478
sudo netstat -tulnp | grep 3478

# Change port in configuration
sudo vim /opt/stunserver/etc/stunserver.conf
# Modify: port = 3479

# Restart service
sudo make restart
```

### Permission denied

```bash
# Ensure correct ownership
sudo chown -R stunserver:stunserver /opt/stunserver

# Check file permissions
ls -la /opt/stunserver/
```

### Docker container issues

```bash
# View container logs
docker logs stunserver

# Check container status
docker ps -a | grep stunserver

# Restart container
docker restart stunserver
```

## Development

### Project Structure

```
stunserver/
├── src/
│   ├── server/         # STUN server implementation
│   ├── client/         # STUN client implementation
│   └── bench/          # Benchmarking tools
├── crates/
│   └── stun-protocol/  # Protocol implementation
├── deploy/             # Deployment scripts (systemd installer, config example)
├── docker/             # Docker files
├── systemd/            # Systemd unit and kernel tuning drop-in
└── .github/workflows/  # CI/CD workflows
```

### Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Run lints
cargo clippy -- -D warnings

# Check formatting
cargo fmt -- --check
```

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Write tests for new functionality
4. Ensure all tests pass
5. Run `cargo fmt` and `cargo clippy`
6. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Links

- **Repository**: https://github.com/zhouhailin/stunserver
- **Issues**: https://github.com/zhouhailin/stunserver/issues
- **RFC 5389**: https://tools.ietf.org/html/rfc5389
