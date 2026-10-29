# Configuration Schema Contract: STUN Server

**Date**: 2026-10-29 | **Feature**: 001-rust-stun-project-init

## Overview

This document defines the YAML configuration schema for stunserver. The configuration file is optional; if not provided, the server uses default values.

---

## Schema Definition

### Root Structure

```yaml
server: ServerConfig
logging: LoggingConfig
```

---

## ServerConfig

**Description**: Server network and operational settings.

### Fields

```yaml
server:
  listen: string  # Socket address (IP:port)
```

### Field Details

| Field    | Type   | Required | Default      | Description                     |
|----------|--------|----------|--------------|---------------------------------|
| listen   | string | No       | 0.0.0.0:3478 | Listen address and port         |

### Validation Rules

- `listen` MUST be a valid socket address in format `IP:PORT`
- `IP` can be IPv4 (e.g., 0.0.0.0, 192.168.1.1) or IPv6 (e.g., ::, ::1)
- `PORT` must be a valid port number (1-65535)
- Port 3478 is the standard STUN port (recommended)

### Examples

```yaml
# Default (listen on all interfaces, port 3478)
server:
  listen: "0.0.0.0:3478"

# Specific IPv4 address
server:
  listen: "192.168.1.100:3478"

# IPv6 address
server:
  listen: "[::]:3478"

# Custom port
server:
  listen: "0.0.0.0:8080"
```

---

## LoggingConfig

**Description**: Logging configuration for server operational events.

### Fields

```yaml
logging:
  level: string
  output: string
  file: FileLogConfig  # Optional
```

### Field Details

| Field   | Type          | Required | Default   | Description                          |
|---------|---------------|----------|-----------|--------------------------------------|
| level   | string        | No       | info      | Logging level                        |
| output  | string        | No       | both      | Log output destination               |
| file    | FileLogConfig | Conditional | -      | File logging configuration (required if output is "file" or "both") |

### Validation Rules

- `level` MUST be one of: "error", "warn", "info", "debug"
- `output` MUST be one of: "console", "file", "both"
- If `output` is "file" or "both", `file` section MUST be present
- If `output` is "console", `file` section is ignored

### Examples

```yaml
# Console only (development)
logging:
  level: "debug"
  output: "console"

# File only (production)
logging:
  level: "info"
  output: "file"
  file:
    path: "/var/log/stunserver/stunserver.log"
    max_size: "100MB"
    rotation: "daily"
    max_files: 7

# Both console and file (debugging)
logging:
  level: "debug"
  output: "both"
  file:
    path: "/var/log/stunserver/stunserver.log"
    max_size: "50MB"
    rotation: "daily"
    max_files: 3
```

---

## FileLogConfig

**Description**: Configuration for file-based logging with rotation.

### Fields

```yaml
file:
  path: string
  max_size: string
  rotation: string
  max_files: integer
```

### Field Details

| Field      | Type    | Required | Default | Description                          |
|------------|---------|----------|---------|--------------------------------------|
| path       | string  | Yes      | -       | Log file path                        |
| max_size   | string  | Yes      | -       | Maximum file size before rotation    |
| rotation   | string  | Yes      | -       | Rotation strategy                    |
| max_files  | integer | Yes      | -       | Maximum number of log files to retain|

### Validation Rules

- `path` MUST be a valid file path (absolute or relative)
- `path` directory MUST exist and be writable
- `max_size` MUST be in format: `<number><unit>` where unit is "B", "KB", "MB", "GB"
- `max_size` MUST be > 0
- `rotation` MUST be "daily" (only supported strategy)
- `max_files` MUST be > 0

### Size Format

The `max_size` field accepts the following formats:
- `"100MB"` - 100 megabytes
- `"1GB"` - 1 gigabyte
- `"500KB"` - 500 kilobytes
- `"10000B"` - 10000 bytes

Supported units:
- `B` - Bytes
- `KB` - Kilobytes (1024 bytes)
- `MB` - Megabytes (1024 KB)
- `GB` - Gigabytes (1024 MB)

### Rotation Strategy

**daily**: Log file rotated at midnight (00:00) local time.

Rotation behavior:
1. At midnight, current log file is renamed to `{path}.{date}` (e.g., stunserver.log.2026-10-29)
2. New log file created with original name
3. If number of log files exceeds `max_files`, oldest file is deleted

### Examples

```yaml
# Standard production logging
file:
  path: "/var/log/stunserver/stunserver.log"
  max_size: "100MB"
  rotation: "daily"
  max_files: 7  # Keep 7 days of logs

# Compact logging (limited disk space)
file:
  path: "/tmp/stunserver.log"
  max_size: "10MB"
  rotation: "daily"
  max_files: 3

# Verbose logging (debugging)
file:
  path: "./logs/stunserver-debug.log"
  max_size: "1GB"
  rotation: "daily"
  max_files: 30
```

---

## Complete Configuration Examples

### Example 1: Minimal Configuration (Development)

```yaml
# Minimal configuration - uses defaults for most settings
server:
  listen: "127.0.0.1:3478"

logging:
  level: "debug"
  output: "console"
```

### Example 2: Production Configuration

```yaml
# Production configuration with file logging
server:
  listen: "0.0.0.0:3478"

logging:
  level: "info"
  output: "file"
  file:
    path: "/var/log/stunserver/stunserver.log"
    max_size: "100MB"
    rotation: "daily"
    max_files: 7
```

### Example 3: High-Volume Production

```yaml
# High-volume production with verbose logging
server:
  listen: "0.0.0.0:3478"

logging:
  level: "warn"  # Reduce log volume
  output: "both"  # Console for monitoring, file for audit
  file:
    path: "/var/log/stunserver/stunserver.log"
    max_size: "1GB"
    rotation: "daily"
    max_files: 30
```

### Example 4: IPv6 Configuration

```yaml
# IPv6 dual-stack configuration
server:
  listen: "[::]:3478"

logging:
  level: "info"
  output: "file"
  file:
    path: "/var/log/stunserver/stunserver.log"
    max_size: "100MB"
    rotation: "daily"
    max_files: 7
```

---

## Default Values

If configuration file is not provided or fields are omitted, the following defaults are used:

```yaml
server:
  listen: "0.0.0.0:3478"

logging:
  level: "info"
  output: "both"
  file:
    path: "/var/log/stunserver/stunserver.log"
    max_size: "100MB"
    rotation: "daily"
    max_files: 7
```

---

## Configuration Loading

### Priority Order

1. Command-line arguments (highest priority)
2. Configuration file
3. Default values (lowest priority)

### Loading Process

1. Parse command-line arguments
2. If `--config` specified, load configuration file
3. Merge configuration: CLI args override file values, file values override defaults
4. Validate final configuration
5. Initialize server with merged configuration

### Error Handling

| Error Condition | Behavior |
|-----------------|----------|
| Config file not found | Fail with error message |
| Config file unreadable | Fail with error message |
| Invalid YAML syntax | Fail with error message and line number |
| Invalid field value | Fail with error message and valid options |
| Missing required field (when conditional) | Fail with error message |

### Error Messages

```
Error: Configuration file not found: /etc/stunserver/config.yaml
Error: Failed to read configuration file: Permission denied
Error: Invalid YAML syntax at line 5, column 10: expected ':'
Error: Invalid logging level: 'verbose'. Valid options: error, warn, info, debug
Error: File logging configuration required when output is 'file' or 'both'
```

---

## Validation

### Schema Validation

Configuration is validated against the schema before server startup:

1. **Type validation**: All fields have correct types
2. **Required field validation**: Conditional requirements checked
3. **Value validation**: Enum values, ranges, formats checked
4. **Path validation**: File paths checked for existence and writability

### Validation Timing

- Configuration validated once at server startup
- Invalid configuration prevents server from starting
- Clear error messages indicate what needs to be fixed

---

## Testing Scenarios

### Scenario 1: Default Configuration

```bash
$ stunserver
# Uses all default values
```

**Expected**: Server starts with default configuration (0.0.0.0:3478, info level, both outputs).

### Scenario 2: Custom Configuration File

```bash
$ stunserver --config /etc/stunserver/config.yaml
```

**Expected**: Server loads and applies configuration from file.

### Scenario 3: CLI Override

```bash
$ stunserver --config /etc/stunserver/config.yaml --listen 0.0.0.0:8080
```

**Expected**: Server uses config file but overrides listen address with CLI argument.

### Scenario 4: Invalid Configuration

```yaml
# invalid.yaml
logging:
  level: "verbose"  # Invalid value
```

```bash
$ stunserver --config invalid.yaml
Error: Invalid logging level: 'verbose'. Valid options: error, warn, info, debug
```

**Expected**: Server fails to start with clear error message.

### Scenario 5: Missing File Configuration

```yaml
# missing-file.yaml
logging:
  output: "file"
  # Missing 'file' section
```

```bash
$ stunserver --config missing-file.yaml
Error: File logging configuration required when output is 'file' or 'both'
```

**Expected**: Server fails to start with clear error message.

---

## Implementation Notes

### Parsing

- Use `serde_yaml` for YAML parsing
- Define Rust structs with `serde::Deserialize` derive
- Use `#[serde(default)]` for optional fields with defaults
- Use custom deserializers for size parsing (e.g., "100MB" → bytes)

### Type Definitions

```rust
#[derive(Deserialize, Debug)]
struct Config {
    #[serde(default)]
    server: ServerConfig,
    #[serde(default)]
    logging: LoggingConfig,
}

#[derive(Deserialize, Debug)]
struct ServerConfig {
    #[serde(default = "default_listen")]
    listen: SocketAddr,
}

#[derive(Deserialize, Debug)]
struct LoggingConfig {
    #[serde(default = "default_log_level")]
    level: LogLevel,
    #[serde(default = "default_log_output")]
    output: LogOutput,
    file: Option<FileLogConfig>,
}

#[derive(Deserialize, Debug)]
struct FileLogConfig {
    path: PathBuf,
    max_size: FileSize,
    rotation: LogRotation,
    max_files: u32,
}
```

### Custom Deserializers

- `FileSize`: Parse "100MB" → `u64` bytes
- `LogLevel`: Parse string → enum
- `LogOutput`: Parse string → enum
- `LogRotation`: Parse string → enum

### Testing

- Unit tests for configuration parsing
- Unit tests for validation rules
- Integration tests for configuration loading
- Property-based tests for edge cases
