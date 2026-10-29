# Build Metadata

All STUN Server executables display build metadata at startup for operational visibility and troubleshooting.

## Overview

Build metadata provides precise version tracking and correlation with source code, enabling:

- **Version Identification**: Know exactly which version is running
- **Debugging**: Correlate runtime behavior with specific code commits
- **Auditing**: Track deployments and changes over time
- **Troubleshooting**: Identify when and where binaries were built

## Metadata Fields

Each executable displays the following information at startup:

```
STUN Server v0.1.0
Commit: 84b9fd3
Commit Time: 2026-10-29T14:09:36+08:00 (UTC)
Build Time: 2026-10-29T07:08:26.866368193+00:00 (UTC)
Startup Time: 2026-10-29T07:08:27.302703918+00:00 (UTC)
Startup Time: 2026-10-29T15:08:27.302737453+08:00 (+08:00)
```

### Field Descriptions

| Field | Description | Format |
|-------|-------------|--------|
| **Version** | Application version from Cargo.toml | Semantic versioning (MAJOR.MINOR.PATCH) |
| **Commit** | Git commit short hash | 7+ character hexadecimal |
| **Commit Time** | Timestamp of the git commit | ISO 8601 (UTC) |
| **Build Time** | Timestamp when binary was compiled | ISO 8601 (UTC) |
| **Startup Time (UTC)** | Current time when server started | ISO 8601 (UTC) |
| **Startup Time (Local)** | Current time in system timezone | ISO 8601 (timezone offset) |

## Implementation

### Build Script (build.rs)

Build metadata is captured at compile time using Cargo's build script system:

```rust
// build.rs
use std::process::Command;

fn main() {
    // Get git commit ID
    let output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .expect("Failed to execute git");
    let commit_id = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .to_string();

    // Get git commit time
    let output = Command::new("git")
        .args(&["log", "-1", "--format=%cI"])
        .output()
        .expect("Failed to execute git");
    let commit_time = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .to_string();

    // Get build time
    let build_time = chrono::Utc::now().to_rfc3339();

    // Get version from Cargo.toml
    let version = env!("CARGO_PKG_VERSION");

    // Export to Rust code
    println!("cargo:rustc-env=GIT_COMMIT_ID={}", commit_id);
    println!("cargo:rustc-env=GIT_COMMIT_TIME={}", commit_time);
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
    println!("cargo:rustc-env=APP_VERSION={}", version);
}
```

### Runtime Display

The metadata is displayed at startup using environment variables:

```rust
// main.rs
fn print_build_info() {
    let version = env!("APP_VERSION");
    let commit_id = env!("GIT_COMMIT_ID");
    let commit_time = env!("GIT_COMMIT_TIME");
    let build_time = env!("BUILD_TIME");
    
    let startup_time_utc = chrono::Utc::now().to_rfc3339();
    let startup_time_local = chrono::Local::now().to_rfc3339();
    let timezone = chrono::Local::now().offset().to_string();

    println!("STUN Server v{}", version);
    println!("Commit: {}", commit_id);
    println!("Commit Time: {} (UTC)", commit_time);
    println!("Build Time: {} (UTC)", build_time);
    println!("Startup Time: {} (UTC)", startup_time_utc);
    println!("Startup Time: {} ({})", startup_time_local, timezone);
}
```

## Timezone Handling

The server displays startup time in two formats:

1. **UTC**: Universal Coordinated Time for consistency across deployments
2. **Local**: System's local timezone with offset for operational convenience

Example:
```
Startup Time: 2026-10-29T07:08:27.302703918+00:00 (UTC)
Startup Time: 2026-10-29T15:08:27.302737453+08:00 (+08:00)
```

The timezone offset is automatically detected from the system configuration.

## Use Cases

### 1. Version Verification

Confirm which version is running in production:

```bash
$ systemctl status stunserver
# Or check logs
$ journalctl -u stunserver | head -20
```

### 2. Debugging

Correlate runtime behavior with source code:

```bash
# Get commit ID from logs
COMMIT=84b9fd3

# View code at that commit
git show $COMMIT

# Or checkout that version
git checkout $COMMIT
```

### 3. Deployment Tracking

Track when binaries were built and deployed:

- **Build Time**: When the binary was compiled
- **Commit Time**: When the source code was last changed
- **Startup Time**: When the server was started

### 4. Audit Compliance

Demonstrate version control for compliance requirements:

- Exact version running in production
- Source code correlation via commit ID
- Timestamps for all relevant events

## Customization

### Changing Metadata Format

To customize the metadata display, modify `print_build_info()` in each executable's `main.rs`.

### Adding Custom Fields

Add custom environment variables in `build.rs`:

```rust
// Example: Add build host
let hostname = hostname::get()
    .unwrap()
    .to_string_lossy()
    .to_string();
println!("cargo:rustc-env=BUILD_HOST={}", hostname);
```

Then use in `main.rs`:

```rust
let build_host = env!("BUILD_HOST");
println!("Build Host: {}", build_host);
```

### Conditional Metadata

Add metadata only in certain build configurations:

```rust
#[cfg(debug_assertions)]
println!("Build Type: Debug");

#[cfg(not(debug_assertions))]
println!("Build Type: Release");
```

## Troubleshooting

### Missing Commit ID

If commit ID shows as empty or "unknown":

- Ensure git is installed and in PATH
- Verify the build is happening in a git repository
- Check build.rs output for errors

### Incorrect Timestamps

If timestamps are incorrect:

- Verify system clock is synchronized (NTP)
- Check timezone configuration
- Ensure chrono crate is correctly handling timezones

### Build Script Errors

If build.rs fails:

- Check git commands work manually
- Verify permissions to execute git
- Review build output for specific errors

## Testing

Integration tests verify build metadata is correctly embedded:

```bash
# Run build metadata tests
cargo test --test build_metadata

# Tests verify:
# - All executables display version
# - Commit ID is embedded
# - Help output works
# - All executables build successfully
```

## Best Practices

1. **Always use tagged releases** in production for clear version tracking
2. **Monitor build metadata** in logs to detect unexpected restarts
3. **Correlate commit IDs** with deployment logs for change tracking
4. **Use UTC timestamps** for consistency across timezones
5. **Keep build scripts simple** to avoid build failures

## References

- [Cargo Build Scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
- [Semantic Versioning](https://semver.org/)
- [ISO 8601 Date Format](https://en.wikipedia.org/wiki/ISO_8601)
- [RFC 3339 Date/Time Format](https://tools.ietf.org/html/rfc3339)
