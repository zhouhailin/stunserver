# Data Model: Rust STUN Server Project

**Date**: 2026-10-29 | **Feature**: 001-rust-stun-project-init

## Overview

The STUN server project has a relatively simple data model focused on protocol messages, configuration, and runtime state. This document defines the core entities, their attributes, relationships, and validation rules.

## Core Entities

### 1. STUN Message

**Description**: Represents a STUN protocol message as defined in RFC 5389.

**Fields**:
- `message_type`: StunMessageType (enum) - Request, Response, or Error indication
- `message_length`: u16 - Length of message body (excluding 20-byte header)
- `magic_cookie`: u32 - Fixed value 0x2112A442 (RFC 5389)
- `transaction_id`: [u8; 12] - Unique transaction identifier
- `attributes`: Vec<StunAttribute> - List of message attributes

**Validation Rules**:
- `magic_cookie` MUST be 0x2112A442
- `message_length` MUST be multiple of 4 (32-bit alignment)
- `transaction_id` MUST be 12 bytes
- `attributes` MUST be properly padded to 4-byte boundaries

**State Transitions**:
- Request → Response (server processing)
- Request → Error Response (server error)
- No state persistence (stateless protocol)

**Relationships**:
- Contains 0..N StunAttribute entities
- Parsed from/build to byte buffer

---

### 2. STUN Attribute

**Description**: Represents a STUN message attribute (type-length-value format).

**Fields**:
- `attribute_type`: StunAttributeType (enum) - MAPPED-ADDRESS, XOR-MAPPED-ADDRESS, ERROR-CODE, etc.
- `attribute_length`: u16 - Length of attribute value
- `attribute_value`: Vec<u8> - Raw attribute value (parsed based on type)

**Validation Rules**:
- `attribute_length` MUST match actual value length
- Attribute value format depends on `attribute_type`
- MUST be padded to 4-byte boundary

**Attribute Types** (RFC 5389):
- `MAPPED-ADDRESS` (0x0001): IPv4/IPv6 address and port
- `XOR-MAPPED-ADDRESS` (0x0020): XOR-encoded address and port
- `ERROR-CODE` (0x0009): Error code and reason phrase
- `SOFTWARE` (0x8022): Software identifier (optional)

**Relationships**:
- Belongs to 1 STUN Message
- Value parsed into specific attribute types

---

### 3. Mapped Address

**Description**: Represents a network address (IP + port) in STUN attributes.

**Fields**:
- `family`: AddressFamily (enum) - IPv4 or IPv6
- `port`: u16 - Network port number
- `address`: IpAddr - IPv4 or IPv6 address

**Validation Rules**:
- `family` MUST match `address` type (V4 → IPv4, V6 → IPv6)
- `port` MUST be valid port number (0-65535)
- `address` MUST be valid IP address

**Relationships**:
- Used in MAPPED-ADDRESS and XOR-MAPPED-ADDRESS attributes
- Returned to client in STUN response

---

### 4. Configuration

**Description**: Server configuration loaded from YAML file.

**Fields**:
- `server`: ServerConfig
  - `listen`: SocketAddr - Listen address and port (default: 0.0.0.0:3478)
- `logging`: LoggingConfig
  - `level`: LogLevel (enum) - Error, Warn, Info, Debug
  - `output`: LogOutput (enum) - Console, File, Both
  - `file`: Option<FileLogConfig>
    - `path`: PathBuf - Log file path
    - `max_size`: u64 - Maximum file size in bytes
    - `rotation`: LogRotation (enum) - Daily
    - `max_files`: u32 - Maximum number of log files to retain

**Validation Rules**:
- `listen` MUST be valid socket address
- `level` MUST be one of: error, warn, info, debug
- `output` MUST be one of: console, file, both
- If `output` is "file" or "both", `file` MUST be present
- `file.path` MUST be writable
- `file.max_size` MUST be > 0
- `file.max_files` MUST be > 0

**Relationships**:
- Loaded once at server startup
- Immutable during runtime
- Provides defaults for command-line arguments

---

### 5. Build Metadata

**Description**: Immutable build information embedded at compile time.

**Fields**:
- `version`: String - Application version from Cargo.toml
- `commit_id`: String - Git commit short hash
- `commit_time`: String - Git commit timestamp (ISO 8601)
- `build_time`: String - Build timestamp (ISO 8601, UTC)

**Validation Rules**:
- `version` MUST follow semantic versioning (MAJOR.MINOR.PATCH)
- `commit_id` MUST be 7+ character hex string
- `commit_time` MUST be valid ISO 8601 timestamp
- `build_time` MUST be valid ISO 8601 timestamp

**Relationships**:
- Embedded at compile time via build.rs
- Displayed at startup for all executables
- Immutable during runtime

---

### 6. Benchmark Result

**Description**: Results from a benchmark test run.

**Fields**:
- `target_qps`: u32 - Target queries per second
- `actual_qps`: f64 - Achieved queries per second
- `total_requests`: u64 - Total requests sent
- `successful_requests`: u64 - Successful responses received
- `failed_requests`: u64 - Failed or timed out requests
- `success_rate`: f64 - Percentage of successful requests (0.0-100.0)
- `latency_p50`: Duration - 50th percentile latency
- `latency_p95`: Duration - 95th percentile latency
- `latency_p99`: Duration - 99th percentile latency
- `duration`: Duration - Total test duration

**Validation Rules**:
- `target_qps` MUST be > 0
- `actual_qps` MUST be >= 0
- `total_requests` = `successful_requests` + `failed_requests`
- `success_rate` = (`successful_requests` / `total_requests`) * 100
- `latency_p50` <= `latency_p95` <= `latency_p99`
- `duration` MUST be > 0

**Relationships**:
- Generated by stunbench after test completion
- Reported to user via console output
- Not persisted (display only)

---

### 7. Server State

**Description**: Runtime state of the STUN server (in-memory, not persisted).

**Fields**:
- `start_time`: DateTime<Utc> - Server startup timestamp
- `timezone`: String - System timezone identifier
- `request_count`: AtomicU64 - Total requests processed (thread-safe counter)
- `error_count`: AtomicU64 - Total errors encountered (thread-safe counter)

**Validation Rules**:
- `start_time` set once at startup, immutable
- `timezone` determined from system configuration
- Counters are thread-safe (AtomicU64)
- No persistence (reset on restart)

**Relationships**:
- Shared across all request handler tasks
- Updated atomically for each request
- Used for logging and monitoring

---

## Entity Relationships

```
STUN Message
├── Contains 0..N STUN Attribute
│   └── May contain Mapped Address
└── Processed by Server State

Configuration
└── Provides settings for Server State

Build Metadata
└── Displayed by all executables at startup

Benchmark Result
└── Generated by stunbench from test run
```

## Data Flow

### Request Processing Flow

1. **Input**: Raw UDP packet (byte buffer)
2. **Parse**: STUN Message (validate magic cookie, length, attributes)
3. **Process**: Extract transaction ID, determine response type
4. **Build Response**: Create STUN Message with MAPPED-ADDRESS attribute
5. **Serialize**: Convert response to byte buffer
6. **Output**: Send UDP response to client

### Configuration Loading Flow

1. **Input**: YAML file path (from CLI or default)
2. **Read**: File contents
3. **Parse**: Deserialize into Configuration struct
4. **Validate**: Check all validation rules
5. **Apply**: Use configuration to initialize server

### Benchmark Execution Flow

1. **Input**: Target QPS, duration, server address
2. **Execute**: Send requests at target rate
3. **Collect**: Track success/failure, measure latency
4. **Calculate**: Compute statistics (QPS, percentiles, success rate)
5. **Output**: Display Benchmark Result to console

---

## Implementation Notes

### Memory Management

- Pre-allocate buffer pool for STUN message parsing (avoid per-request allocation)
- Use `Arc<ServerState>` for shared state (minimize cloning)
- Configuration loaded once, cloned only if needed (immutable)

### Thread Safety

- Server state counters use `AtomicU64` (lock-free updates)
- Configuration is immutable after loading (safe to share)
- Each UDP request handled in separate Tokio task

### Serialization

- STUN messages: Custom binary serialization (network byte order)
- Configuration: serde_yaml (YAML format)
- Build metadata: Compile-time environment variables

### Testing Strategy

- Unit tests for each entity (validation, serialization)
- Integration tests for request/response flow
- Property-based tests for STUN message encoding/decoding
- Coverage target: 95%+ across all entities
