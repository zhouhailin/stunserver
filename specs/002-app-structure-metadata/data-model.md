# Data Model: Application Structure and Metadata Enhancement

**Feature**: 002-app-structure-metadata  
**Date**: 2026-10-29  
**Status**: Complete

## Overview

This document defines the core entities, their attributes, relationships, and validation rules for the application structure and metadata enhancement feature.

## Entity Definitions

### 1. BuildMetadata

**Description**: Immutable collection of build-time and runtime information for each application binary.

**Attributes**:
| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `app_name` | `&'static str` | Application name (stunserver, stunclient, stunbench) | Must match `CARGO_PKG_NAME` |
| `version` | `&'static str` | Semantic version string | Must match `CARGO_PKG_VERSION` format (MAJOR.MINOR.PATCH) |
| `commit_id` | `&'static str` | Git commit hash at build time | 40-character hex string or "unknown" |
| `compile_time` | `&'static str` | ISO 8601 timestamp of compilation | RFC 3339 format (YYYY-MM-DDTHH:MM:SS±HH:MM) |
| `is_dirty` | `bool` | Whether build had uncommitted changes | Derived from git status |

**Relationships**:
- Used by: `StunServer`, `StunClient`, `StunBench` (all three applications)
- Captured by: `build.rs` (root build script)
- Displayed in: Startup log, --help output

**Lifecycle**:
1. **Build time**: Captured by `build.rs` via git commands and environment variables
2. **Compile time**: Embedded into binary via `env!()` macros
3. **Runtime**: Immutable, read-only access via `BuildMetadata::capture()`
4. **Display**: Formatted into startup log and help output

**Validation Rules**:
- `app_name` MUST be one of: "stunserver", "stunclient", "stunbench"
- `version` MUST follow semantic versioning (e.g., "0.1.0")
- `commit_id` MUST be 40 hex characters OR "unknown" (if git unavailable)
- `compile_time` MUST be valid RFC 3339 format
- All fields MUST be populated at compile time (no runtime computation except `startup_time`)

**Example**:
```rust
BuildMetadata {
    app_name: "stunserver",
    version: "0.1.0",
    commit_id: "abc123def456789012345678901234567890abcd",
    compile_time: "2026-10-29T10:00:00+08:00",
    is_dirty: false,
}
```

---

### 2. NetworkAddress

**Description**: Parsed representation of a target server address, supporting multiple input formats.

**Attributes**:
| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `original` | `String` | Original user-provided address string | Non-empty |
| `host` | `Host` | Parsed hostname or IP address | Valid DNS name or IP |
| `port` | `u16` | Port number | 1-65535, defaults to 3478 |
| `resolved_addrs` | `Vec<SocketAddr>` | Resolved socket addresses (after DNS) | At least one address |

**Host Enum**:
```rust
enum Host {
    IPv4(Ipv4Addr),
    IPv6(Ipv6Addr),
    Domain(String),
}
```

**Relationships**:
- Input from: CLI arguments (`stunclient <address>`)
- Resolved by: `ToSocketAddrs` trait (DNS resolution)
- Used by: `StunClient::connect()`

**Lifecycle**:
1. **Input**: User provides address string (e.g., "stun.example.com:3479")
2. **Parsing**: Split into host and port (default port 3478 if omitted)
3. **Resolution**: DNS lookup via `ToSocketAddrs` → `Vec<SocketAddr>`
4. **Connection**: Try addresses in order with fallback
5. **Success**: Return connected socket and report which IP was used

**Validation Rules**:
- `original` MUST be non-empty string
- `host` MUST be valid IPv4, IPv6, or DNS-resolvable domain
- `port` MUST be in range 1-65535
- `resolved_addrs` MUST contain at least one address after DNS resolution
- DNS resolution failures MUST produce clear error messages

**Supported Formats**:
| Input Format | Parsed Host | Parsed Port |
|--------------|-------------|-------------|
| `"192.168.1.1"` | IPv4(192.168.1.1) | 3478 (default) |
| `"192.168.1.1:3479"` | IPv4(192.168.1.1) | 3479 |
| `"stun.example.com"` | Domain("stun.example.com") | 3478 (default) |
| `"stun.example.com:3479"` | Domain("stun.example.com") | 3479 |
| `"[::1]:3478"` | IPv6(::1) | 3478 |
| `"::1"` | IPv6(::1) | 3478 (default) |

**Example**:
```rust
NetworkAddress {
    original: "stunserver2025.stunprotocol.org".to_string(),
    host: Host::Domain("stunserver2025.stunprotocol.org".to_string()),
    port: 3478,
    resolved_addrs: vec![
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)), 3478),
        SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0x2606, 0x2800, 0x220, 0x1, 0x248, 0x1893, 0x25c8, 0x1946)), 3478),
    ],
}
```

---

### 3. StartupLogEntry

**Description**: Single-line, structured log message output by each application at startup.

**Attributes**:
| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `metadata` | `BuildMetadata` | Build metadata (name, version, commit, compile time) | Valid BuildMetadata |
| `startup_time` | `DateTime<Local>` | Runtime timestamp when log is output | RFC 3339 format, server local timezone |

**Format**: Plain text with fixed positions
```
{app_name} v{version} commit={commit_id} compiled={compile_time} started={startup_time}
```

**Relationships**:
- Contains: `BuildMetadata`
- Output by: `StunServer`, `StunClient`, `StunBench` (all three applications)
- Suppressed by: `--quiet` flag

**Lifecycle**:
1. **Build**: `BuildMetadata` captured at compile time
2. **Startup**: Application parses args, loads config
3. **Log**: `StartupLogEntry` created with current time
4. **Output**: Single line written to stdout
5. **Suppress**: If `--quiet` flag present, skip output

**Validation Rules**:
- MUST be output after argument parsing and config loading
- MUST be output before network binding or main event loop
- MUST use server's local timezone (not UTC)
- MUST use consistent format across all three applications
- MUST be suppressible via `--quiet` flag
- MUST be written to stdout (not stderr)

**Example Output**:
```
stunserver v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
```

---

### 4. ServerConfig

**Description**: Configuration for stunserver application.

**Attributes**:
| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `bind_address` | `SocketAddr` | Address to bind server socket | Valid socket address |
| `workers` | `usize` | Number of worker threads | >= 1, defaults to CPU count |
| `log_level` | `LevelFilter` | Logging verbosity | Off, Error, Warn, Info, Debug, Trace |
| `quiet` | `bool` | Suppress startup log | Defaults to false |

**Relationships**:
- Loaded from: CLI arguments, config file
- Used by: `StunServer::start()`
- Contains: Startup log suppression flag

**Validation Rules**:
- `bind_address` MUST be valid socket address
- `workers` MUST be >= 1
- `log_level` MUST be valid log level
- `quiet` defaults to false if not specified

**Example**:
```rust
ServerConfig {
    bind_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 3478),
    workers: 4,
    log_level: LevelFilter::Info,
    quiet: false,
}
```

---

### 5. ClientConfig

**Description**: Configuration for stunclient application.

**Attributes**:
| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `server_address` | `NetworkAddress` | Target STUN server address | Valid NetworkAddress |
| `timeout` | `Duration` | Connection timeout | > 0, defaults to 5 seconds |
| `retries` | `u32` | Number of retry attempts | >= 0, defaults to 3 |
| `quiet` | `bool` | Suppress startup log | Defaults to false |

**Relationships**:
- Loaded from: CLI arguments
- Used by: `StunClient::connect()`
- Contains: `NetworkAddress` for DNS resolution

**Validation Rules**:
- `server_address` MUST be valid NetworkAddress
- `timeout` MUST be > 0
- `retries` MUST be >= 0
- `quiet` defaults to false if not specified

**Example**:
```rust
ClientConfig {
    server_address: NetworkAddress::parse("stunserver2025.stunprotocol.org").unwrap(),
    timeout: Duration::from_secs(5),
    retries: 3,
    quiet: false,
}
```

---

### 6. BenchConfig

**Description**: Configuration for stunbench application.

**Attributes**:
| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `server_address` | `NetworkAddress` | Target STUN server address | Valid NetworkAddress |
| `concurrency` | `usize` | Number of concurrent clients | >= 1, defaults to 10 |
| `duration` | `Duration` | Test duration | > 0, defaults to 60 seconds |
| `requests_per_client` | `u64` | Requests per client | >= 1, defaults to 100 |
| `quiet` | `bool` | Suppress startup log | Defaults to false |

**Relationships**:
- Loaded from: CLI arguments
- Used by: `StunBench::run()`
- Contains: `NetworkAddress` for DNS resolution

**Validation Rules**:
- `server_address` MUST be valid NetworkAddress
- `concurrency` MUST be >= 1
- `duration` MUST be > 0
- `requests_per_client` MUST be >= 1
- `quiet` defaults to false if not specified

**Example**:
```rust
BenchConfig {
    server_address: NetworkAddress::parse("stunserver2025.stunprotocol.org").unwrap(),
    concurrency: 10,
    duration: Duration::from_secs(60),
    requests_per_client: 100,
    quiet: false,
}
```

---

## Entity Relationships

```
┌─────────────────┐
│  BuildMetadata  │
│  (immutable)    │
└────────┬────────┘
         │ used by
         ├──────────────────┬──────────────────┐
         │                  │                  │
         ▼                  ▼                  ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  StartupLogEntry│ │  StartupLogEntry│ │  StartupLogEntry│
│  (stunserver)   │ │  (stunclient)   │ │  (stunbench)    │
└─────────────────┘ └────────┬────────┘ └────────┬────────┘
                             │                   │
                             │ contains          │ contains
                             ▼                   ▼
                    ┌─────────────────┐ ┌─────────────────┐
                    │ NetworkAddress  │ │ NetworkAddress  │
                    │ (DNS resolution)│ │ (DNS resolution)│
                    └────────┬────────┘ └────────┬────────┘
                             │                   │
                             │ used by           │ used by
                             ▼                   ▼
                    ┌─────────────────┐ ┌─────────────────┐
                    │  ClientConfig   │ │   BenchConfig   │
                    └─────────────────┘ └─────────────────┘
```

## State Transitions

### NetworkAddress State Machine

```
[Input String]
      │
      ▼
[Parsing] ──(invalid format)──> [Error]
      │
      ▼
[Host + Port]
      │
      ▼
[DNS Resolution] ──(failure)──> [Error with message]
      │
      ▼
[Vec<SocketAddr>]
      │
      ▼
[Connection Attempt 1] ──(failure)──> [Try Next]
      │                                    │
      │ (success)                          │ (all failed)
      ▼                                    ▼
[Connected Socket]                    [Error: all failed]
```

### StartupLogEntry Lifecycle

```
[Build Time]
      │
      ▼
[BuildMetadata Captured]
      │
      ▼
[Compile Time]
      │
      ▼
[Embedded in Binary]
      │
      ▼
[Runtime: Process Start]
      │
      ▼
[Parse Arguments]
      │
      ▼
[Load Configuration]
      │
      ▼
[Check --quiet flag] ──(true)──> [Skip Log Output]
      │
      │ (false)
      ▼
[Create StartupLogEntry]
      │
      ▼
[Output to stdout]
      │
      ▼
[Continue Initialization]
```

## Validation Summary

| Entity | Key Validations |
|--------|----------------|
| BuildMetadata | app_name in allowed list, version is semver, commit_id is 40 hex or "unknown", timestamps are RFC 3339 |
| NetworkAddress | host is valid IP/domain, port in 1-65535, resolved_addrs non-empty |
| StartupLogEntry | output timing (after config, before network), format consistency, timezone correctness |
| ServerConfig | bind_address valid, workers >= 1, log_level valid |
| ClientConfig | server_address valid, timeout > 0, retries >= 0 |
| BenchConfig | server_address valid, concurrency >= 1, duration > 0, requests >= 1 |

## Design Decisions

1. **BuildMetadata is immutable**: Captured at compile time, embedded in binary, no runtime computation (except startup_time)
2. **NetworkAddress supports fallback**: DNS resolution returns multiple addresses, connection attempts try each in order
3. **StartupLogEntry is suppressible**: --quiet flag allows operators to disable log output for security-sensitive deployments
4. **Configs are separate per application**: ServerConfig, ClientConfig, BenchConfig have distinct fields but share common patterns (quiet flag, network address)
5. **Plain text log format**: Human-readable, parseable by standard tools (grep, awk), no JSON overhead
6. **Default port 3478**: RFC 5389 standard, reduces user friction for common case
