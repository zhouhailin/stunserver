# CLI Contracts: Rust STUN Server Project

**Date**: 2026-10-29 | **Feature**: 001-rust-stun-project-init

## Overview

This document defines the command-line interface contracts for all three executables: stunserver, stunclient, and stunbench. Each contract specifies the command syntax, arguments, options, exit codes, and output format.

---

## 1. stunserver

**Description**: High-performance STUN server for NAT traversal.

### Command Syntax

```bash
stunserver [OPTIONS]
```

### Arguments

None (all configuration via options or config file).

### Options

| Short | Long       | Type     | Default        | Description                          |
|-------|------------|----------|----------------|--------------------------------------|
| -c    | --config   | PATH     | (none)         | Path to YAML configuration file      |
| -l    | --listen   | ADDR     | 0.0.0.0:3478   | Listen address and port              |
| -h    | --help     | (flag)   | -              | Print help information               |
| -V    | --version  | (flag)   | -              | Print version information            |

### Exit Codes

| Code | Meaning                          |
|------|----------------------------------|
| 0    | Success (graceful shutdown)      |
| 1    | Configuration error              |
| 2    | Network binding error            |
| 3    | Runtime error                    |
| 130  | Interrupted (Ctrl+C)             |

### Output Format

#### Startup Output

```
STUN Server v{version}
Commit: {commit_id}
Commit Time: {commit_time} (UTC)
Build Time: {build_time} (UTC)
Startup Time: {startup_time_utc} (UTC)
Startup Time: {startup_time_local} ({timezone})
Listening on {listen_address}
```

#### Runtime Output (Logging)

```
[{timestamp}] [{level}] {message}
```

Example:
```
[2026-10-29T14:30:00Z] [INFO] Server started successfully
[2026-10-29T14:30:05Z] [DEBUG] Received binding request from 192.168.1.100:54321
[2026-10-29T14:30:05Z] [INFO] Processed 1000 requests
```

### Examples

```bash
# Start with default configuration
stunserver

# Start with custom configuration file
stunserver --config /etc/stunserver/config.yaml

# Start with custom listen address
stunserver --listen 0.0.0.0:3478

# Show help
stunserver --help

# Show version
stunserver --version
```

---

## 2. stunclient

**Description**: STUN client for testing server functionality.

### Command Syntax

```bash
stunclient [OPTIONS] <SERVER>
```

### Arguments

| Position | Name    | Type | Required | Description              |
|----------|---------|------|----------|--------------------------|
| 1        | SERVER  | ADDR | Yes      | Server address and port  |

### Options

| Short | Long       | Type     | Default | Description                          |
|-------|------------|----------|---------|--------------------------------------|
| -t    | --timeout  | SECONDS  | 5       | Request timeout in seconds           |
| -h    | --help     | (flag)   | -       | Print help information               |
| -V    | --version  | (flag)   | -       | Print version information            |

### Exit Codes

| Code | Meaning                          |
|------|----------------------------------|
| 0    | Success (response received)      |
| 1    | Invalid arguments                |
| 2    | Connection error                 |
| 3    | Timeout error                    |
| 4    | Protocol error                   |
| 130  | Interrupted (Ctrl+C)             |

### Output Format

#### Startup Output

```
STUN Client v{version}
Commit: {commit_id}
Commit Time: {commit_time} (UTC)
Build Time: {build_time} (UTC)
Startup Time: {startup_time_utc} (UTC)
Startup Time: {startup_time_local} ({timezone})
```

#### Success Output

```
Sending binding request to {server_address}...
Response received in {latency}ms
Mapped Address: {ip_address}:{port}
```

Example:
```
Sending binding request to 192.168.1.1:3478...
Response received in 2.5ms
Mapped Address: 203.0.113.50:12345
```

#### Error Output

```
Error: {error_message}
```

Examples:
```
Error: Connection timeout after 5 seconds
Error: Server returned error: 400 Bad Request
Error: Invalid server address: invalid IP address syntax
```

### Examples

```bash
# Send binding request to server
stunclient 192.168.1.1:3478

# Send with custom timeout
stunclient --timeout 10 192.168.1.1:3478

# Show help
stunclient --help

# Show version
stunclient --version
```

---

## 3. stunbench

**Description**: STUN server benchmark tool for performance testing.

### Command Syntax

```bash
stunbench [OPTIONS] <SERVER>
```

### Arguments

| Position | Name    | Type | Required | Description              |
|----------|---------|------|----------|--------------------------|
| 1        | SERVER  | ADDR | Yes      | Server address and port  |

### Options

| Short | Long       | Type     | Default | Description                          |
|-------|------------|----------|---------|--------------------------------------|
| -q    | --qps      | NUMBER | Yes      | Target queries per second            |
| -d    | --duration | SECONDS| 60      | Test duration in seconds             |
| -n    | --requests | NUMBER | -       | Total number of requests (alternative to duration) |
| -h    | --help     | (flag)   | -       | Print help information               |
| -V    | --version  | (flag)   | -       | Print version information            |

**Note**: Either `--duration` or `--requests` must be specified, but not both.

### Exit Codes

| Code | Meaning                          |
|------|----------------------------------|
| 0    | Success (test completed)         |
| 1    | Invalid arguments                |
| 2    | Connection error                 |
| 3    | Test aborted                     |
| 130  | Interrupted (Ctrl+C)             |

### Output Format

#### Startup Output

```
STUN Benchmark v{version}
Commit: {commit_id}
Commit Time: {commit_time} (UTC)
Build Time: {build_time} (UTC)
Startup Time: {startup_time_utc} (UTC)
Startup Time: {startup_time_local} ({timezone})
```

#### Progress Output (Real-time)

```
Starting benchmark: {target_qps} QPS for {duration} seconds
Target: {server_address}

[00:10] Sent: 10000, Success: 9995, Failed: 5, QPS: 1000
[00:20] Sent: 20000, Success: 19990, Failed: 10, QPS: 1000
...
```

#### Final Results

```
Benchmark Complete
==================
Duration: {actual_duration}s
Target QPS: {target_qps}
Actual QPS: {actual_qps:.2f}
Total Requests: {total_requests}
Successful: {successful_requests} ({success_rate:.2f}%)
Failed: {failed_requests}

Latency Percentiles:
  p50: {latency_p50}ms
  p95: {latency_p95}ms
  p99: {latency_p99}ms
```

Example:
```
Benchmark Complete
==================
Duration: 60.0s
Target QPS: 10000
Actual QPS: 9995.23
Total Requests: 599714
Successful: 599654 (99.99%)
Failed: 60

Latency Percentiles:
  p50: 0.52ms
  p95: 1.23ms
  p99: 2.45ms
```

### Examples

```bash
# Run benchmark at 10000 QPS for 60 seconds (default)
stunbench --qps 10000 192.168.1.1:3478

# Run benchmark for 30 seconds
stunbench --qps 10000 --duration 30 192.168.1.1:3478

# Run benchmark with 100000 total requests
stunbench --qps 10000 --requests 100000 192.168.1.1:3478

# Show help
stunbench --help

# Show version
stunbench --version
```

---

## Common Behavior

### Build Metadata Display

All three executables display build metadata at startup in the following format:

```
{Executable Name} v{version}
Commit: {commit_id}
Commit Time: {commit_time} (UTC)
Build Time: {build_time} (UTC)
Startup Time: {startup_time_utc} (UTC)
Startup Time: {startup_time_local} ({timezone})
```

Where:
- `{version}`: Semantic version from Cargo.toml (e.g., 1.0.0)
- `{commit_id}`: Short git commit hash (7+ characters)
- `{commit_time}`: Git commit timestamp in ISO 8601 format
- `{build_time}`: Build timestamp in ISO 8601 format (UTC)
- `{startup_time_utc}`: Startup timestamp in ISO 8601 format (UTC)
- `{startup_time_local}`: Startup timestamp in local timezone
- `{timezone}`: System timezone identifier (e.g., "Asia/Shanghai", "America/New_York")

### Help Output

All executables support `--help` flag with auto-generated help text from clap:

```
{Executable Name} {version}
{Description}

USAGE:
    {command_syntax}

ARGS:
    {arguments}

OPTIONS:
    {options}
```

### Version Output

All executables support `--version` flag:

```
{Executable Name} {version}
```

### Signal Handling

All executables handle SIGINT (Ctrl+C) gracefully:
- Log shutdown message
- Clean up resources
- Exit with code 130

### Error Messages

Error messages follow the format:
```
Error: {human_readable_message}
```

Examples:
```
Error: Configuration file not found: /etc/stunserver/config.yaml
Error: Failed to bind to 0.0.0.0:3478: Address already in use
Error: Invalid server address: invalid IP address syntax
```

---

## Validation Rules

### Argument Validation

1. **stunserver**:
   - `--config` file must exist and be readable
   - `--listen` must be valid socket address
   - Port must be available (not in use)

2. **stunclient**:
   - `SERVER` argument is required
   - `SERVER` must be valid socket address
   - `--timeout` must be > 0

3. **stunbench**:
   - `SERVER` argument is required
   - `SERVER` must be valid socket address
   - `--qps` is required and must be > 0
   - Either `--duration` or `--requests` must be specified (not both)
   - `--duration` must be > 0
   - `--requests` must be > 0

### Error Handling

All executables:
- Print error messages to stderr
- Exit with non-zero code on error
- Provide actionable error messages (suggest fixes when possible)

---

## Testing Scenarios

### Scenario 1: Default Server Startup

```bash
$ stunserver
STUN Server v1.0.0
Commit: abc1234
Commit Time: 2026-10-29T10:00:00Z (UTC)
Build Time: 2026-10-29T14:00:00Z (UTC)
Startup Time: 2026-10-29T14:30:00Z (UTC)
Startup Time: 2026-10-29T22:30:00+08:00 (Asia/Shanghai)
Listening on 0.0.0.0:3478
```

**Expected**: Server starts, displays metadata, listens on default address.

### Scenario 2: Client Success

```bash
$ stunclient 192.168.1.1:3478
STUN Client v1.0.0
...
Sending binding request to 192.168.1.1:3478...
Response received in 2.5ms
Mapped Address: 203.0.113.50:12345
```

**Expected**: Client sends request, receives response, displays mapped address.

### Scenario 3: Benchmark Success

```bash
$ stunbench --qps 10000 --duration 10 192.168.1.1:3478
STUN Benchmark v1.0.0
...
Starting benchmark: 10000 QPS for 10 seconds
...
Benchmark Complete
==================
...
Actual QPS: 9995.23
...
```

**Expected**: Benchmark runs for 10 seconds, reports results.

### Scenario 4: Error Handling

```bash
$ stunclient invalid-address
Error: Invalid server address: invalid IP address syntax
```

**Expected**: Clear error message, non-zero exit code.
