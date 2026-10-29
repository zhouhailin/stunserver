# Research: Application Structure and Metadata Enhancement

**Feature**: 002-app-structure-metadata  
**Date**: 2026-10-29  
**Status**: Complete

## Overview

This document captures research findings for technical decisions required by the implementation plan. All "NEEDS CLARIFICATION" items from the technical context have been resolved through research.

## Research Topics

### 1. Build Metadata Capture Strategy

**Decision**: Use a root-level `build.rs` script with environment variables and git commands to capture build metadata at compile time.

**Rationale**: 
- Build scripts (`build.rs`) are the standard Rust mechanism for compile-time code generation
- Environment variables (`CARGO_PKG_VERSION`, `CARGO_PKG_NAME`) provide version info from Cargo.toml
- Git commands (`git rev-parse HEAD`, `git status --porcelain`) provide commit ID and dirty state
- `chrono::Utc::now()` at build time provides compilation timestamp
- Build metadata is embedded into the binary via `include!()` or `env!()` macros

**Implementation Approach**:
```rust
// build.rs (root level)
use std::process::Command;

fn main() {
    // Capture git commit ID
    let commit_id = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    
    // Check for dirty working tree
    let dirty = Command::new("git")
        .args(&["status", "--porcelain"])
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    
    // Capture compilation timestamp
    let compile_time = chrono::Local::now().to_rfc3339();
    
    // Pass to Rust code via environment variables
    println!("cargo:rustc-env=BUILD_COMMIT_ID={}", commit_id);
    println!("cargo:rustc-env=BUILD_COMPILE_TIME={}", compile_time);
    println!("cargo:rustc-env=BUILD_DIRTY={}", dirty);
    
    // Re-run build script if .git/HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
}
```

**Alternatives Considered**:
- ❌ **Compile-time macros only**: Cannot capture git info without build script
- ❌ **External metadata files**: Requires runtime file I/O, not embedded in binary
- ❌ **Manual version entry**: Error-prone, easy to forget to update

**References**:
- Cargo build scripts: https://doc.rust-lang.org/cargo/reference/build-scripts.html
- `env!()` macro: https://doc.rust-lang.org/std/macro.env.html

---

### 2. Startup Log Format and Timing

**Decision**: Use plain text format with fixed positions, output after argument parsing but before network binding.

**Rationale**:
- Plain text format is human-readable and parseable by grep/awk/sed
- Fixed positions ensure consistent format across all three applications
- Timing (after config, before network) ensures version info is available even if network binding fails
- Using existing `log` crate with `env_logger` backend maintains consistency with Rust ecosystem

**Format Specification**:
```
{app_name} v{version} commit={commit_id} compiled={compile_time} started={startup_time}
```

**Example Output**:
```
stunserver v0.1.0 commit=abc123def compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00
```

**Implementation Approach**:
```rust
// src/server/metadata.rs (similar for client and bench)
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
        let startup_time = chrono::Local::now().to_rfc3339();
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

**Timing Sequence**:
1. Process starts
2. Parse command-line arguments (clap)
3. Load configuration files
4. **Output startup log** ← This point
5. Initialize network sockets
6. Bind to ports
7. Enter main event loop

**Alternatives Considered**:
- ❌ **JSON format**: Overkill for single log line, requires jq for parsing
- ❌ **Key-value pairs**: Less readable than fixed positions
- ❌ **Immediately at process start**: Config parsing might fail, log wouldn't reflect actual running version
- ❌ **After network binding**: If binding fails, no log output at all

**References**:
- chrono crate: https://crates.io/crates/chrono
- RFC 3339 (ISO 8601 profile): https://tools.ietf.org/html/rfc3339

---

### 3. DNS Resolution and Address Parsing

**Decision**: Use Rust's standard `ToSocketAddrs` trait for DNS resolution, with fallback to next address on connection failure.

**Rationale**:
- `ToSocketAddrs` is the standard Rust trait for address resolution
- It handles both IP addresses and domain names transparently
- It respects system DNS configuration (/etc/resolv.conf, /etc/hosts)
- It returns an iterator of socket addresses, enabling fallback logic
- No external dependencies required (e.g., no `trust-dns-resolver`)

**Implementation Approach**:
```rust
// src/client/address.rs
use std::net::{ToSocketAddrs, SocketAddr};

pub fn resolve_address(addr_str: &str, default_port: u16) -> Result<Vec<SocketAddr>, String> {
    // Parse address string
    let addr_with_port = if addr_str.contains(':') {
        addr_str.to_string()
    } else {
        format!("{}:{}", addr_str, default_port)
    };
    
    // Resolve to socket addresses
    addr_with_port
        .to_socket_addrs()
        .map(|iter| iter.collect())
        .map_err(|e| format!("DNS resolution failed: {}", e))
}

pub fn connect_with_fallback(addresses: &[SocketAddr]) -> Result<TcpStream, String> {
    let mut last_error = None;
    
    for addr in addresses {
        match TcpStream::connect(addr) {
            Ok(stream) => {
                println!("Connected to {}", addr);
                return Ok(stream);
            }
            Err(e) => {
                eprintln!("Failed to connect to {}: {}", addr, e);
                last_error = Some(e);
            }
        }
    }
    
    Err(last_error
        .map(|e| format!("All connection attempts failed: {}", e))
        .unwrap_or_else(|| "No addresses to try".to_string()))
}
```

**Address Format Support**:
- `"192.168.1.1"` → IP only, use default port 3478
- `"192.168.1.1:3479"` → IP with explicit port
- `"stun.example.com"` → Domain only, use default port 3478
- `"stun.example.com:3479"` → Domain with explicit port
- `"[::1]:3478"` → IPv6 with port
- `"::1"` → IPv6 only, use default port 3478

**Fallback Behavior**:
1. DNS returns multiple IP addresses (e.g., [A, B, C])
2. Try to connect to A
3. If A fails, try B
4. If B fails, try C
5. If all fail, return error
6. Report which IP was successfully used

**Alternatives Considered**:
- ❌ **trust-dns-resolver crate**: External dependency, overkill for basic DNS
- ❌ **First address only**: No fallback, less reliable
- ❌ **Random selection**: Unpredictable, harder to debug
- ❌ **IPv4-only**: Doesn't support modern IPv6 networks

**References**:
- ToSocketAddrs trait: https://doc.rust-lang.org/std/net/trait.ToSocketAddrs.html
- STUN port 3478: RFC 5389 Section 4

---

### 4. CLI Version Display Convention

**Decision**: Display version information as the first line of --help output, using clap's built-in version support.

**Rationale**:
- clap 4.x has built-in support for version display via `.version()` method
- Placing version as first line is the most common CLI convention (git, rustc, cargo)
- Consistent format across all three applications improves user experience
- No additional dependencies required

**Implementation Approach**:
```rust
// src/server/main.rs
use clap::{Command, Arg};

fn main() {
    let metadata = BuildMetadata::capture();
    
    let cmd = Command::new("stunserver")
        .version(metadata.version)
        .about("STUN server implementation (RFC 5389)")
        .long_about(format!(
            "{} v{} (commit {}, compiled {})\n\nHigh-performance STUN server implementation compliant with RFC 5389.",
            metadata.app_name,
            metadata.version,
            metadata.commit_id,
            metadata.compile_time
        ))
        // ... other arguments
        ;
    
    cmd.get_matches();
}
```

**Output Format**:
```
$ stunserver --help
stunserver 0.1.0 (commit abc123def, compiled 2026-10-29T10:00:00+08:00)

High-performance STUN server implementation compliant with RFC 5389.

USAGE:
    stunserver [OPTIONS]

OPTIONS:
    -c, --config <FILE>    Configuration file path
    -q, --quiet            Suppress startup log output
    -h, --help             Print help information
    -V, --version          Print version information
```

**Consistency Across Applications**:
- stunserver: `stunserver {version} (commit {commit_id}, compiled {compile_time})`
- stunclient: `stunclient {version} (commit {commit_id}, compiled {compile_time})`
- stunbench: `stunbench {version} (commit {commit_id}, compiled {compile_time})`

**Alternatives Considered**:
- ❌ **Separate --version flag only**: Less discoverable, users expect version in --help
- ❌ **Version in footer**: Less visible, users typically scan top of help first
- ❌ **JSON version output**: Not standard CLI convention, harder to parse with grep

**References**:
- clap documentation: https://docs.rs/clap/latest/clap/
- CLI design guidelines: https://clig.dev/

---

### 5. RFC Compliance Verification Strategy

**Decision**: Implement RFC 5769 test vectors as integration tests, with code comments referencing specific RFC sections.

**Rationale**:
- RFC 5769 provides official test vectors for STUN message parsing/serialization
- Integration tests validate end-to-end protocol compliance
- Code comments referencing RFC sections improve maintainability
- RFC 5389 compliance is verified through test vector validation
- RFC 5780 (optional) will have separate feature-gated tests

**Implementation Approach**:

**RFC 5769 Test Vectors** (mandatory):
```rust
// tests/rfc5769_vectors.rs
#[cfg(test)]
mod rfc5769_tests {
    use stun_protocol::message::StunMessage;
    
    #[test]
    fn test_vector_1_request() {
        // RFC 5769 Section 2.1: Sample Request
        let bytes = include_bytes!("rfc5769/vector1.bin");
        let msg = StunMessage::parse(bytes).unwrap();
        assert_eq!(msg.message_type, 0x0001); // Binding Request
        assert_eq!(msg.transaction_id, [/* expected value */]);
        // ... validate all attributes per RFC 5769
    }
    
    #[test]
    fn test_vector_2_response() {
        // RFC 5769 Section 2.2: Sample IPv4 Response
        // ... similar validation
    }
    
    #[test]
    fn test_vector_3_response() {
        // RFC 5769 Section 2.3: Sample IPv6 Response
        // ... similar validation
    }
    
    #[test]
    fn test_vector_4_request() {
        // RFC 5769 Section 2.4: Sample Request with MD5 Authentication
        // ... similar validation
    }
}
```

**Code Comments Referencing RFC Sections**:
```rust
// crates/stun-protocol/src/message.rs

/// STUN message header (RFC 5389 Section 6)
/// 
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |0 0|     STUN Message Type     |         Message Length        |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                         Magic Cookie                          |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                                                               |
/// |                     Transaction ID (96 bits)                  |
/// |                                                               |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
pub struct StunHeader {
    pub message_type: u16,    // RFC 5389 Section 6.1
    pub message_length: u16,  // RFC 5389 Section 6
    pub magic_cookie: u32,    // RFC 5389 Section 6 (must be 0x2112A442)
    pub transaction_id: [u8; 12], // RFC 5389 Section 6
}
```

**RFC 5780 Optional Tests** (feature-gated):
```rust
// tests/rfc5780_nat_discovery.rs
#[cfg(feature = "rfc5780")]
mod rfc5780_tests {
    #[test]
    fn test_nat_behavior_discovery() {
        // RFC 5780 Section 4.1: NAT Behavior Discovery
        // ... test implementation
    }
}
```

**Compliance Verification Checklist**:
- ✅ RFC 5389 Section 6: Message format (header, magic cookie, transaction ID)
- ✅ RFC 5389 Section 7: Message types (Request, Indication, Success Response, Error Response)
- ✅ RFC 5389 Section 15: Mandatory attributes (MAPPED-ADDRESS, XOR-MAPPED-ADDRESS)
- ✅ RFC 5389 Section 18: Error codes and reasons
- ✅ RFC 5769: All four test vectors pass validation
- ⚠️ RFC 5780: Optional, feature-gated (NAT behavior discovery)
- ❌ RFC 3489: Obsoleted, not implemented

**Alternatives Considered**:
- ❌ **External STUN test suite**: Not available, must implement our own
- ❌ **Manual protocol inspection**: Error-prone, not automated
- ❌ **Third-party RFC validation tools**: Not available for STUN protocol
- ❌ **Skip RFC 5769 test vectors**: Would miss official validation, non-compliant

**References**:
- RFC 5389: https://tools.ietf.org/html/rfc5389
- RFC 5769: https://tools.ietf.org/html/rfc5769
- RFC 5780: https://tools.ietf.org/html/rfc5780
- RFC 3489: https://tools.ietf.org/html/rfc3489 (obsoleted)

---

### 6. Directory Restructuring Migration Strategy

**Decision**: Perform restructuring in a single commit with careful path updates to Cargo.toml workspace configuration.

**Rationale**:
- Incremental restructuring creates merge conflicts and makes review difficult
- Cargo workspace configuration tightly couples directory structure and build system
- Single commit allows atomic testing (all-or-nothing)
- Git history remains clean with clear before/after state

**Migration Steps**:
1. Create new directory structure (src/server, src/client, src/bench, crates/)
2. Move files to new locations:
   - `stunserver/src/*` → `src/server/*`
   - `stunclient/src/*` → `src/client/*`
   - `stunbench/src/*` → `src/bench/*`
   - `stun-protocol/` → `crates/stun-protocol/`
3. Update root `Cargo.toml` workspace members:
   ```toml
   [workspace]
   members = [
       "crates/stun-protocol",
       "src/server",
       "src/client",
       "src/bench",
   ]
   ```
4. Update each application's `Cargo.toml` to reference `crates/stun-protocol`
5. Update `build.rs` paths if needed
6. Run `cargo build` to verify compilation
7. Run `cargo test` to verify all tests pass
8. Commit as single atomic change

**Risk Mitigation**:
- Create backup branch before restructuring
- Test build and tests after each major step
- Use `git mv` to preserve file history
- Update CI/CD configuration in same commit (if applicable)

**Alternatives Considered**:
- ❌ **Incremental restructuring (multiple PRs)**: Creates merge conflicts, harder to review
- ❌ **Symlinks for backward compatibility**: Confusing, non-standard
- ❌ **Keep old structure, add new alongside**: Duplicate code, maintenance burden
- ❌ **Automated migration script**: Overkill for 4 directories, manual is clearer

**References**:
- Cargo workspaces: https://doc.rust-lang.org/cargo/reference/workspaces.html
- Git mv: https://git-scm.com/docs/git-mv

---

## Summary of Decisions

| Topic | Decision | Rationale |
|-------|----------|-----------|
| Build metadata | Root build.rs with git commands | Standard Rust approach, embedded in binary |
| Startup log format | Plain text, fixed positions | Human-readable, parseable by grep/awk |
| Startup log timing | After config, before network | Version info available even if network fails |
| DNS resolution | ToSocketAddrs with fallback | Standard library, no external deps |
| Default port | 3478 (RFC 5389) | STUN protocol standard |
| CLI version display | First line of --help | Common convention, discoverable |
| RFC compliance | RFC 5769 test vectors + code comments | Official validation, maintainable |
| Directory restructuring | Single commit migration | Atomic, clean git history |

## Next Steps

All research items are complete. Proceeding to Phase 1 design artifacts:
- data-model.md
- contracts/cli-interface.md
- contracts/log-format.md
- quickstart.md
