# CLI Interface Contract

**Feature**: 002-app-structure-metadata  
**Date**: 2026-10-29  
**Status**: Complete

## Overview

This document defines the command-line interface (CLI) contracts for all three STUN applications: stunserver, stunclient, and stunbench. All interfaces must maintain backward compatibility while adding new features (startup logging, --quiet flag, version display).

## Common Patterns

### Version Display

All three applications MUST display version information as the first line of `--help` output:

```
{app_name} {version} (commit {commit_id}, compiled {compile_time})
```

**Example**:
```
$ stunserver --help
stunserver 0.1.0 (commit abc123def, compiled 2026-10-29T10:00:00+08:00)

High-performance STUN server implementation compliant with RFC 5389.

USAGE:
    stunserver [OPTIONS]

OPTIONS:
    ...
```

### Startup Log Output

All three applications MUST output a startup log line to stdout after argument parsing and configuration loading, but before network operations:

```
{app_name} v{version} commit={commit_id} compiled={compile_time} started={startup_time}
```

**Suppression**: The `--quiet` flag MUST suppress this output.

### Common Flags

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--help` | `-h` | Display help information | N/A |
| `--version` | `-V` | Display version information | N/A |
| `--quiet` | `-q` | Suppress startup log output | `false` |
| `--config` | `-c` | Configuration file path | `config/default.yaml` |
| `--log-level` | `-l` | Log verbosity (off, error, warn, info, debug, trace) | `info` |

---

## Application: stunserver

### Synopsis

```
stunserver [OPTIONS]
```

### Description

High-performance STUN server implementation compliant with RFC 5389. Listens for STUN Binding Requests and responds with Binding Success Responses containing the client's mapped address.

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| (none) | No | Server runs with default or config-file settings |

### Options

| Option | Short | Argument | Default | Description |
|--------|-------|----------|---------|-------------|
| `--bind` | `-b` | `<ADDRESS:PORT>` | `0.0.0.0:3478` | Address and port to bind server socket |
| `--workers` | `-w` | `<NUM>` | CPU count | Number of worker threads |
| `--config` | `-c` | `<FILE>` | `config/default.yaml` | Configuration file path |
| `--log-level` | `-l` | `<LEVEL>` | `info` | Log verbosity |
| `--quiet` | `-q` | (none) | `false` | Suppress startup log output |
| `--help` | `-h` | (none) | N/A | Display help information |
| `--version` | `-V` | (none) | N/A | Display version information |

### Examples

**Start server with defaults**:
```bash
$ stunserver
stunserver v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
Listening on 0.0.0.0:3478 with 4 workers
```

**Start server on custom port**:
```bash
$ stunserver --bind 0.0.0.0:3479
stunserver v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:01+08:00
Listening on 0.0.0.0:3479 with 4 workers
```

**Start server with config file**:
```bash
$ stunserver --config /etc/stun/server.yaml
stunserver v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:02+08:00
Listening on 192.168.1.100:3478 with 8 workers
```

**Start server quietly (no startup log)**:
```bash
$ stunserver --quiet
Listening on 0.0.0.0:3478 with 4 workers
```

**Display help**:
```bash
$ stunserver --help
stunserver 0.1.0 (commit abc123def, compiled 2026-10-29T10:00:00+08:00)

High-performance STUN server implementation compliant with RFC 5389.

USAGE:
    stunserver [OPTIONS]

OPTIONS:
    -b, --bind <ADDRESS:PORT>    Address and port to bind server socket [default: 0.0.0.0:3478]
    -w, --workers <NUM>          Number of worker threads [default: CPU count]
    -c, --config <FILE>          Configuration file path [default: config/default.yaml]
    -l, --log-level <LEVEL>      Log verbosity [default: info] [possible values: off, error, warn, info, debug, trace]
    -q, --quiet                  Suppress startup log output
    -h, --help                   Print help information
    -V, --version                Print version information
```

**Display version**:
```bash
$ stunserver --version
stunserver 0.1.0 (commit abc123def, compiled 2026-10-29T10:00:00+08:00)
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (clean shutdown) |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | Configuration file not found or invalid |
| 4 | Failed to bind to address (port in use or permission denied) |
| 5 | Runtime error (network failure, etc.) |

---

## Application: stunclient

### Synopsis

```
stunclient [OPTIONS] <SERVER_ADDRESS>
```

### Description

STUN client implementation for testing STUN servers. Sends STUN Binding Requests to a specified server and displays the mapped address returned by the server. Supports DNS resolution and defaults to port 3478 (RFC 5389 standard).

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `<SERVER_ADDRESS>` | Yes | Target STUN server address (IP or domain, with optional port) |

**Address Formats**:
- `IP` - IPv4 or IPv6 address, uses default port 3478
- `IP:PORT` - IPv4 or IPv6 address with explicit port
- `DOMAIN` - Domain name, uses default port 3478
- `DOMAIN:PORT` - Domain name with explicit port
- `[IPv6]:PORT` - IPv6 address with explicit port (brackets required)

### Options

| Option | Short | Argument | Default | Description |
|--------|-------|----------|---------|-------------|
| `--timeout` | `-t` | `<SECONDS>` | `5` | Connection timeout in seconds |
| `--retries` | `-r` | `<NUM>` | `3` | Number of retry attempts |
| `--config` | `-c` | `<FILE>` | `config/default.yaml` | Configuration file path |
| `--log-level` | `-l` | `<LEVEL>` | `info` | Log verbosity |
| `--quiet` | `-q` | (none) | `false` | Suppress startup log output |
| `--help` | `-h` | (none) | N/A | Display help information |
| `--version` | `-V` | (none) | N/A | Display version information |

### Examples

**Query server by IP (default port 3478)**:
```bash
$ stunclient 192.168.1.1
stunclient v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
Connecting to 192.168.1.1:3478...
Mapped address: 203.0.113.50:12345
```

**Query server by IP with explicit port**:
```bash
$ stunclient 192.168.1.1:3479
stunclient v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:01+08:00
Connecting to 192.168.1.1:3479...
Mapped address: 203.0.113.50:12345
```

**Query server by domain name (DNS resolution)**:
```bash
$ stunclient stunserver2025.stunprotocol.org
stunclient v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:02+08:00
Resolving stunserver2025.stunprotocol.org...
Connecting to 93.184.216.34:3478...
Mapped address: 203.0.113.50:12345
```

**Query server by domain with explicit port**:
```bash
$ stunclient stunserver2025.stunprotocol.org:3479
stunclient v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:03+08:00
Resolving stunserver2025.stunprotocol.org...
Connecting to 93.184.216.34:3479...
Mapped address: 203.0.113.50:12345
```

**Query with custom timeout and retries**:
```bash
$ stunclient --timeout 10 --retries 5 stunserver2025.stunprotocol.org
stunclient v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:04+08:00
Resolving stunserver2025.stunprotocol.org...
Connecting to 93.184.216.34:3478...
Mapped address: 203.0.113.50:12345
```

**Query quietly (no startup log)**:
```bash
$ stunclient --quiet stunserver2025.stunprotocol.org
Resolving stunserver2025.stunprotocol.org...
Connecting to 93.184.216.34:3478...
Mapped address: 203.0.113.50:12345
```

**Display help**:
```bash
$ stunclient --help
stunclient 0.1.0 (commit abc123def, compiled 2026-10-29T10:00:00+08:00)

STUN client for testing STUN servers (RFC 5389 compliant)

USAGE:
    stunclient [OPTIONS] <SERVER_ADDRESS>

ARGS:
    <SERVER_ADDRESS>    Target STUN server address (IP or domain, with optional port)

OPTIONS:
    -t, --timeout <SECONDS>    Connection timeout in seconds [default: 5]
    -r, --retries <NUM>        Number of retry attempts [default: 3]
    -c, --config <FILE>        Configuration file path [default: config/default.yaml]
    -l, --log-level <LEVEL>    Log verbosity [default: info] [possible values: off, error, warn, info, debug, trace]
    -q, --quiet                Suppress startup log output
    -h, --help                 Print help information
    -V, --version              Print version information
```

**Display version**:
```bash
$ stunclient --version
stunclient 0.1.0 (commit abc123def, compiled 2026-10-29T10:00:00+08:00)
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (mapped address retrieved) |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | DNS resolution failed |
| 4 | Connection failed (all addresses tried) |
| 5 | Timeout (no response from server) |
| 6 | STUN protocol error (malformed response) |

### DNS Resolution Behavior

When a domain name is provided:
1. Resolve domain to IP addresses via system DNS resolver
2. If multiple addresses returned, try each in order
3. Fall back to next address if connection fails
4. Report which IP address was successfully used
5. If all addresses fail, exit with code 3 or 4

**Example with multiple IPs**:
```bash
$ stunclient example.com
stunclient v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
Resolving example.com...
Found 3 addresses: [93.184.216.34, 93.184.216.35, 93.184.216.36]
Trying 93.184.216.34:3478... Failed: Connection refused
Trying 93.184.216.35:3478... Success!
Mapped address: 203.0.113.50:12345
```

---

## Application: stunbench

### Synopsis

```
stunbench [OPTIONS] <SERVER_ADDRESS>
```

### Description

STUN benchmarking tool for load testing STUN servers. Sends concurrent STUN Binding Requests to measure server performance (throughput, latency, error rate).

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `<SERVER_ADDRESS>` | Yes | Target STUN server address (same formats as stunclient) |

### Options

| Option | Short | Argument | Default | Description |
|--------|-------|----------|---------|-------------|
| `--concurrency` | `-c` | `<NUM>` | `10` | Number of concurrent clients |
| `--duration` | `-d` | `<SECONDS>` | `60` | Test duration in seconds |
| `--requests` | `-n` | `<NUM>` | `100` | Requests per client |
| `--timeout` | `-t` | `<SECONDS>` | `5` | Connection timeout in seconds |
| `--config` | `-C` | `<FILE>` | `config/default.yaml` | Configuration file path |
| `--log-level` | `-l` | `<LEVEL>` | `info` | Log verbosity |
| `--quiet` | `-q` | (none) | `false` | Suppress startup log output |
| `--help` | `-h` | (none) | N/A | Display help information |
| `--version` | `-V` | (none) | N/A | Display version information |

### Examples

**Run benchmark with defaults**:
```bash
$ stunbench stunserver2025.stunprotocol.org
stunbench v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
Resolving stunserver2025.stunprotocol.org...
Starting benchmark: 10 concurrent clients, 60 seconds, 100 requests/client
Connecting to 93.184.216.34:3478...

Benchmark Results:
  Total requests: 1000
  Successful: 995 (99.5%)
  Failed: 5 (0.5%)
  Avg latency: 12.5ms
  P95 latency: 25.3ms
  P99 latency: 45.8ms
  Throughput: 16.6 req/s
```

**Run benchmark with custom parameters**:
```bash
$ stunbench --concurrency 50 --duration 120 --requests 200 stunserver2025.stunprotocol.org
stunbench v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:01+08:00
Resolving stunserver2025.stunprotocol.org...
Starting benchmark: 50 concurrent clients, 120 seconds, 200 requests/client
Connecting to 93.184.216.34:3478...

Benchmark Results:
  Total requests: 10000
  Successful: 9850 (98.5%)
  Failed: 150 (1.5%)
  Avg latency: 15.2ms
  P95 latency: 30.1ms
  P99 latency: 55.7ms
  Throughput: 83.3 req/s
```

**Display help**:
```bash
$ stunbench --help
stunbench 0.1.0 (commit abc123def, compiled 2026-10-29T10:00:00+08:00)

STUN benchmarking tool for load testing STUN servers

USAGE:
    stunbench [OPTIONS] <SERVER_ADDRESS>

ARGS:
    <SERVER_ADDRESS>    Target STUN server address (IP or domain, with optional port)

OPTIONS:
    -c, --concurrency <NUM>      Number of concurrent clients [default: 10]
    -d, --duration <SECONDS>     Test duration in seconds [default: 60]
    -n, --requests <NUM>         Requests per client [default: 100]
    -t, --timeout <SECONDS>      Connection timeout in seconds [default: 5]
    -C, --config <FILE>          Configuration file path [default: config/default.yaml]
    -l, --log-level <LEVEL>      Log verbosity [default: info] [possible values: off, error, warn, info, debug, trace]
    -q, --quiet                  Suppress startup log output
    -h, --help                   Print help information
    -V, --version                Print version information
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (benchmark completed) |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | DNS resolution failed |
| 4 | Connection failed |
| 5 | Benchmark interrupted (Ctrl+C) |

---

## Backward Compatibility

### Breaking Changes

**None**. All existing command-line interfaces remain functional. New features are additive:
- `--quiet` flag is optional (default: false)
- Startup log output is new but doesn't affect existing workflows
- Version display in `--help` is informational only

### Migration Path

Users do not need to change existing scripts or workflows. The new features are opt-in:
- To suppress startup logs: add `--quiet` flag
- To see version info: run with `--help` or `--version`
- Default behavior unchanged: startup log is output, version info available in help

---

## Validation Checklist

- [x] All three applications support `--help` and `--version` flags
- [x] Version information appears as first line of `--help` output
- [x] Startup log format is consistent across all three applications
- [x] `--quiet` flag suppresses startup log output
- [x] stunclient defaults to port 3478 when not specified
- [x] stunclient supports DNS resolution for domain names
- [x] stunclient tries multiple DNS results in order with fallback
- [x] All address formats (IP, IP:port, domain, domain:port, IPv6) are supported
- [x] Exit codes are documented and consistent
- [x] Examples demonstrate all major use cases
- [x] Backward compatibility maintained (no breaking changes)
