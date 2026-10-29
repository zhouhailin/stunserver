# Startup Log Format Contract

**Feature**: 002-app-structure-metadata  
**Date**: 2026-10-29  
**Status**: Complete

## Overview

This document defines the startup log format contract for all three STUN applications. The startup log is a single-line, structured message output to stdout when each application begins execution.

## Log Format Specification

### Format Pattern

```
{app_name} v{version} commit={commit_id} compiled={compile_time} started={startup_time}
```

### Field Definitions

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `app_name` | string | Application name (stunserver, stunclient, stunbench) | `stunserver` |
| `version` | string | Semantic version (MAJOR.MINOR.PATCH) | `0.1.0` |
| `commit_id` | string | Git commit hash (40 hex chars) or "unknown" | `abc123def456789012345678901234567890abcd` |
| `compile_time` | string | ISO 8601 timestamp of compilation (RFC 3339) | `2026-10-29T10:00:00+08:00` |
| `startup_time` | string | ISO 8601 timestamp of startup (RFC 3339) | `2026-10-29T10:05:00+08:00` |

### Field Separators

- Fields are separated by single spaces
- Key-value pairs use `=` with no spaces around the equals sign
- Version field is prefixed with `v` (lowercase)

### Timestamp Format

All timestamps MUST use RFC 3339 format (ISO 8601 profile):

```
YYYY-MM-DDTHH:MM:SS±HH:MM
```

**Examples**:
- `2026-10-29T10:00:00+08:00` (China Standard Time)
- `2026-10-29T02:00:00+00:00` (UTC)
- `2026-10-28T19:00:00-07:00` (Pacific Daylight Time)

**Timezone**: Timestamps MUST reflect the server's local timezone at the time of logging, not UTC.

## Example Outputs

### stunserver

```
stunserver v0.1.0 commit=abc123def456789012345678901234567890abcd compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
```

### stunclient

```
stunclient v0.1.0 commit=abc123def456789012345678901234567890abcd compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:01+08:00
```

### stunbench

```
stunbench v0.1.0 commit=abc123def456789012345678901234567890abcd compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:02+08:00
```

### Dirty Build (uncommitted changes)

When built from a dirty git working tree, the commit_id remains the same, but the version string MAY include a `-dirty` suffix (implementation-defined):

```
stunserver v0.1.0-dirty commit=abc123def456789012345678901234567890abcd compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
```

### Unknown Commit (git unavailable)

When git is not available during build:

```
stunserver v0.1.0 commit=unknown compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
```

## Timing Requirements

### When to Output

The startup log MUST be output:
1. **After** command-line argument parsing completes
2. **After** configuration file loading completes
3. **Before** network socket binding or connection attempts
4. **Before** main event loop or primary application logic starts

### Timing Sequence

```
Process Start
    ↓
Parse CLI Arguments
    ↓
Load Configuration Files
    ↓
[OUTPUT STARTUP LOG] ← This point
    ↓
Initialize Network Sockets
    ↓
Bind/Connect to Network
    ↓
Enter Main Event Loop
```

### Performance Target

The startup log MUST be output within **100 milliseconds** of completing argument parsing and configuration loading.

**Rationale**: Ensures version information is available quickly for operators and log aggregation systems, even if subsequent initialization steps are slow.

## Suppression

### --quiet Flag

The startup log MUST be suppressed when the `--quiet` (or `-q`) flag is provided.

**Example**:
```bash
$ stunserver --quiet
Listening on 0.0.0.0:3478 with 4 workers
```

**No startup log line appears in the output.**

### Implementation

```rust
if !config.quiet {
    metadata.log_startup();
}
```

## Output Destination

### stdout vs stderr

The startup log MUST be written to **stdout** (standard output), not stderr.

**Rationale**:
- Startup log is informational, not an error
- Allows log aggregation tools to capture stdout separately from stderr
- Consistent with other CLI tools (git, rustc, cargo)
- Can be redirected: `stunserver > server.log`

### Line Termination

The startup log MUST be a single line terminated by a newline character (`\n`).

**Example**:
```
stunserver v0.1.0 commit=abc123 compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00\n
```

## Parsing and Aggregation

### Machine Parseability

The log format is designed to be parseable by standard tools:

**grep** (extract specific field):
```bash
$ stunserver 2>&1 | grep -oP 'commit=\K\w+'
abc123def456789012345678901234567890abcd
```

**awk** (extract fields):
```bash
$ stunserver 2>&1 | awk '{print $1, $2}'
stunserver v0.1.0
```

**sed** (extract version):
```bash
$ stunserver 2>&1 | sed -n 's/.*v\([0-9.]*\).*/\1/p'
0.1.0
```

**jq** (if converted to JSON):
```bash
$ stunserver 2>&1 | sed 's/\([^ ]*\) v\([^ ]*\) commit=\([^ ]*\) compiled=\([^ ]*\) started=\([^ ]*\)/{"app":"\1","version":"\2","commit":"\3","compiled":"\4","started":"\5"}/' | jq .
{
  "app": "stunserver",
  "version": "0.1.0",
  "commit": "abc123def456789012345678901234567890abcd",
  "compiled": "2026-10-29T10:00:00+08:00",
  "started": "2026-10-29T10:05:00+08:00"
}
```

### Log Aggregation Systems

The format is compatible with common log aggregation patterns:

**Fluentd / Fluent Bit**:
```conf
<source>
  @type exec
  command stunserver --quiet
  tag stun.server
  <parse>
    @type regexp
    expression /^(?<app>\w+) v(?<version>[0-9.]+) commit=(?<commit>\w+) compiled=(?<compiled>[^\s]+) started=(?<started>[^\s]+)$/
  </parse>
</source>
```

**Logstash**:
```conf
input {
  exec {
    command => "stunserver --quiet"
    interval => 60
  }
}
filter {
  grok {
    match => { "message" => "%{WORD:app} v%{DATA:version} commit=%{WORD:commit} compiled=%{TIMESTAMP_ISO8601:compiled} started=%{TIMESTAMP_ISO8601:started}" }
  }
}
```

## Validation Rules

### Format Validation

The startup log MUST pass the following validation checks:

1. **Single line**: No newline characters except at the end
2. **Five fields**: app_name, version, commit_id, compile_time, startup_time
3. **Field order**: Fields MUST appear in the specified order
4. **Separators**: Single space between fields, `=` for key-value pairs
5. **Version prefix**: Version field MUST start with `v`
6. **Timestamp format**: Both timestamps MUST be valid RFC 3339
7. **Timezone**: Timestamps MUST include timezone offset (±HH:MM)
8. **Commit ID**: MUST be 40 hex characters OR "unknown"

### Test Cases

**Valid examples**:
```
✅ stunserver v0.1.0 commit=abc123def456789012345678901234567890abcd compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
✅ stunclient v1.2.3 commit=unknown compiled=2026-10-29T02:00:00+00:00 started=2026-10-29T02:05:00+00:00
✅ stunbench v0.0.1 commit=1234567890abcdef1234567890abcdef12345678 compiled=2026-10-28T19:00:00-07:00 started=2026-10-28T19:05:00-07:00
```

**Invalid examples**:
```
❌ stunserver v0.1.0 commit=abc123 compiled=2026-10-29T10:00:00 started=2026-10-29T10:05:00
   (missing timezone offset in timestamps)

❌ stunserver 0.1.0 commit=abc123 compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
   (missing 'v' prefix on version)

❌ stunserver v0.1.0 commit=abc123 compiled=2026-10-29T10:00:00+08:00
   (missing startup_time field)

❌ stunserver v0.1.0 commit=abc123 compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00 extra_field=value
   (extra field not allowed)

❌ stunserver v0.1.0 commit=abc123 compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
   (commit_id must be 40 hex chars or "unknown", "abc123" is only 6 chars)
```

## Consistency Requirements

### Cross-Application Consistency

All three applications (stunserver, stunclient, stunbench) MUST:
1. Use the exact same log format pattern
2. Use the same field order and separators
3. Use the same timestamp format (RFC 3339)
4. Use the same timezone handling (server local time)
5. Output to stdout (not stderr)
6. Respect the --quiet flag

### Version Information Consistency

The version information in the startup log MUST match the version information in the --help output:

**Startup log**:
```
stunserver v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
```

**Help output**:
```
stunserver 0.1.0 (commit abc123def, compiled 2026-10-29T10:00:00+08:00)
```

**Note**: The format differs slightly (startup log uses `v` prefix and `commit=` key-value, help output uses parentheses), but the VALUES must be identical.

## Implementation Notes

### Rust Implementation

```rust
use chrono::Local;

pub struct BuildMetadata {
    pub app_name: &'static str,
    pub version: &'static str,
    pub commit_id: &'static str,
    pub compile_time: &'static str,
}

impl BuildMetadata {
    pub fn capture() -> Self {
        Self {
            app_name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
            commit_id: env!("BUILD_COMMIT_ID"),
            compile_time: env!("BUILD_COMPILE_TIME"),
        }
    }
    
    pub fn log_startup(&self) {
        let startup_time = Local::now().to_rfc3339();
        println!(
            "{} v{} commit={} compiled={} started={}",
            self.app_name,
            self.version,
            self.commit_id,
            self.compile_time,
            startup_time
        );
    }
}
```

### Build Script (build.rs)

```rust
use std::process::Command;
use chrono::Local;

fn main() {
    // Capture git commit ID
    let commit_id = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());
    
    // Capture compilation timestamp
    let compile_time = Local::now().to_rfc3339();
    
    // Pass to Rust code via environment variables
    println!("cargo:rustc-env=BUILD_COMMIT_ID={}", commit_id);
    println!("cargo:rustc-env=BUILD_COMPILE_TIME={}", compile_time);
    
    // Re-run build script if .git/HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
}
```

## Compliance Checklist

- [x] Format pattern defined and documented
- [x] All five fields specified with types and examples
- [x] Timestamp format (RFC 3339) specified
- [x] Timezone handling (server local time) specified
- [x] Timing requirements (when to output) specified
- [x] Performance target (100ms) specified
- [x] Suppression mechanism (--quiet flag) specified
- [x] Output destination (stdout) specified
- [x] Line termination (newline) specified
- [x] Parsing examples (grep, awk, sed, jq) provided
- [x] Log aggregation examples (Fluentd, Logstash) provided
- [x] Validation rules defined
- [x] Test cases (valid and invalid) provided
- [x] Cross-application consistency requirements defined
- [x] Version information consistency requirements defined
- [x] Rust implementation example provided
- [x] Build script example provided
