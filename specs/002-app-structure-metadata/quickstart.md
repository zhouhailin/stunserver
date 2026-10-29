# Quickstart Validation Guide

**Feature**: 002-app-structure-metadata  
**Date**: 2026-10-29  
**Status**: Complete

## Overview

This guide provides runnable validation scenarios to verify the feature works end-to-end. Each scenario tests specific requirements from the specification and can be executed independently.

## Prerequisites

- Rust toolchain (1.75+ with edition 2021)
- Git (for commit ID capture)
- Network access (for DNS resolution and STUN server testing)
- Standard Unix tools: `grep`, `awk`, `sed` (for log parsing validation)

## Setup

### 1. Build All Binaries

```bash
cd /opt/stunserver
cargo build --release
```

**Expected**: All three binaries build successfully with zero errors and zero warnings.

**Verify**:
```bash
ls -lh target/release/stunserver target/release/stunclient target/release/stunbench
```

### 2. Verify Directory Structure

```bash
tree -L 2 -d src crates
```

**Expected output**:
```
crates
└── stun-protocol
src
├── bench
├── client
└── server
```

### 3. Run Test Suite

```bash
cargo test
```

**Expected**: All tests pass (unit tests + integration tests).

---

## Validation Scenarios

### Scenario 1: Startup Log Format and Timing

**Objective**: Verify startup log output format, timing, and suppressibility.

**Steps**:

1. **Test stunserver startup log**:
   ```bash
   ./target/release/stunserver --bind 127.0.0.1:3478 &
   SERVER_PID=$!
   sleep 1
   kill $SERVER_PID
   ```

   **Expected output** (first line):
   ```
   stunserver v0.1.0 commit=<40-char-hash> compiled=<RFC3339-timestamp> started=<RFC3339-timestamp>
   ```

2. **Verify log format with regex**:
   ```bash
   ./target/release/stunserver --bind 127.0.0.1:3478 2>&1 | head -1 | grep -P '^\w+ v\d+\.\d+\.\d+ commit=[a-f0-9]{40} compiled=\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{2}:\d{2} started=\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{2}:\d{2}$'
   ```

   **Expected**: Regex matches (exit code 0).

3. **Test --quiet flag**:
   ```bash
   ./target/release/stunserver --bind 127.0.0.1:3478 --quiet 2>&1 | head -1
   ```

   **Expected**: First line is NOT a startup log (should be "Listening on..." or similar).

4. **Test stunclient startup log**:
   ```bash
   timeout 2 ./target/release/stunclient 127.0.0.1:3478 2>&1 | head -1
   ```

   **Expected output**:
   ```
   stunclient v0.1.0 commit=<40-char-hash> compiled=<RFC3339-timestamp> started=<RFC3339-timestamp>
   ```

5. **Test stunbench startup log**:
   ```bash
   timeout 2 ./target/release/stunbench --duration 1 127.0.0.1:3478 2>&1 | head -1
   ```

   **Expected output**:
   ```
   stunbench v0.1.0 commit=<40-char-hash> compiled=<RFC3339-timestamp> started=<RFC3339-timestamp>
   ```

**Success Criteria**:
- ✅ All three applications output startup log as first line
- ✅ Log format matches specification (5 fields, correct separators)
- ✅ Timestamps are RFC 3339 format with timezone
- ✅ --quiet flag suppresses startup log
- ✅ Commit ID is 40 hex characters

---

### Scenario 2: Version Information in --help Output

**Objective**: Verify version information appears in --help output.

**Steps**:

1. **Test stunserver --help**:
   ```bash
   ./target/release/stunserver --help | head -1
   ```

   **Expected output**:
   ```
   stunserver 0.1.0 (commit <hash>, compiled <timestamp>)
   ```

2. **Test stunclient --help**:
   ```bash
   ./target/release/stunclient --help | head -1
   ```

   **Expected output**:
   ```
   stunclient 0.1.0 (commit <hash>, compiled <timestamp>)
   ```

3. **Test stunbench --help**:
   ```bash
   ./target/release/stunbench --help | head -1
   ```

   **Expected output**:
   ```
   stunbench 0.1.0 (commit <hash>, compiled <timestamp>)
   ```

4. **Test -h short form**:
   ```bash
   ./target/release/stunserver -h | head -1
   ```

   **Expected**: Same as --help output.

5. **Test --version flag**:
   ```bash
   ./target/release/stunserver --version
   ```

   **Expected output**:
   ```
   stunserver 0.1.0 (commit <hash>, compiled <timestamp>)
   ```

6. **Verify consistency**:
   ```bash
   STARTUP_LOG=$(./target/release/stunserver --bind 127.0.0.1:3478 2>&1 | head -1 | grep -oP 'v\K[0-9.]+')
   HELP_VERSION=$(./target/release/stunserver --help | head -1 | grep -oP '\d+\.\d+\.\d+')
   [ "$STARTUP_LOG" = "$HELP_VERSION" ] && echo "Versions match" || echo "Versions differ"
   ```

   **Expected**: "Versions match"

**Success Criteria**:
- ✅ All three applications show version as first line of --help
- ✅ Version format is consistent: `{app} {version} (commit {hash}, compiled {timestamp})`
- ✅ -h short form works identically to --help
- ✅ --version flag outputs version information
- ✅ Version in startup log matches version in --help

---

### Scenario 3: Default Port 3478

**Objective**: Verify stunclient defaults to port 3478 when not specified.

**Prerequisites**: Start a STUN server on port 3478.

**Steps**:

1. **Start stunserver on default port**:
   ```bash
   ./target/release/stunserver --bind 0.0.0.0:3478 &
   SERVER_PID=$!
   sleep 1
   ```

2. **Query with IP only (no port)**:
   ```bash
   ./target/release/stunclient 127.0.0.1 2>&1 | grep -i "connecting to"
   ```

   **Expected output**:
   ```
   Connecting to 127.0.0.1:3478...
   ```

3. **Query with domain name (no port)**:
   ```bash
   ./target/release/stunclient localhost 2>&1 | grep -i "connecting to"
   ```

   **Expected output**:
   ```
   Connecting to 127.0.0.1:3478...
   ```

4. **Clean up**:
   ```bash
   kill $SERVER_PID
   ```

**Success Criteria**:
- ✅ stunclient connects to port 3478 when no port specified
- ✅ Works with both IP addresses and domain names

---

### Scenario 4: DNS Resolution

**Objective**: Verify stunclient supports DNS resolution and handles multiple addresses.

**Prerequisites**: Network access to resolve domain names.

**Steps**:

1. **Test with public STUN server (domain name)**:
   ```bash
   ./target/release/stunclient stunserver2025.stunprotocol.org 2>&1 | head -10
   ```

   **Expected output** (key lines):
   ```
   stunclient v0.1.0 commit=... compiled=... started=...
   Resolving stunserver2025.stunprotocol.org...
   Connecting to <IP>:3478...
   Mapped address: <IP>:<port>
   ```

2. **Verify DNS resolution message**:
   ```bash
   ./target/release/stunclient stunserver2025.stunprotocol.org 2>&1 | grep -i "resolving"
   ```

   **Expected output**:
   ```
   Resolving stunserver2025.stunprotocol.org...
   ```

3. **Test with explicit port**:
   ```bash
   ./target/release/stunclient stunserver2025.stunprotocol.org:3478 2>&1 | grep -i "connecting to"
   ```

   **Expected output**:
   ```
   Connecting to <IP>:3478...
   ```

4. **Test DNS failure handling**:
   ```bash
   ./target/release/stunclient invalid-domain-that-does-not-exist.example 2>&1
   echo "Exit code: $?"
   ```

   **Expected output**:
   ```
   stunclient v0.1.0 commit=... compiled=... started=...
   Resolving invalid-domain-that-does-not-exist.example...
   Error: DNS resolution failed: ...
   Exit code: 3
   ```

**Success Criteria**:
- ✅ stunclient resolves domain names to IP addresses
- ✅ DNS resolution message is displayed
- ✅ Default port 3478 is used with domain names
- ✅ Explicit port works with domain names
- ✅ DNS failures produce clear error messages and non-zero exit code

---

### Scenario 5: Address Format Support

**Objective**: Verify stunclient handles all address formats correctly.

**Prerequisites**: Start a STUN server on multiple ports.

**Steps**:

1. **Start stunserver on port 3478**:
   ```bash
   ./target/release/stunserver --bind 127.0.0.1:3478 &
   SERVER_PID=$!
   sleep 1
   ```

2. **Test IPv4 without port**:
   ```bash
   ./target/release/stunclient 127.0.0.1 2>&1 | grep "Mapped address"
   ```
   **Expected**: Returns mapped address

3. **Test IPv4 with port**:
   ```bash
   ./target/release/stunclient 127.0.0.1:3478 2>&1 | grep "Mapped address"
   ```
   **Expected**: Returns mapped address

4. **Test domain without port**:
   ```bash
   ./target/release/stunclient localhost 2>&1 | grep "Mapped address"
   ```
   **Expected**: Returns mapped address

5. **Test domain with port**:
   ```bash
   ./target/release/stunclient localhost:3478 2>&1 | grep "Mapped address"
   ```
   **Expected**: Returns mapped address

6. **Clean up**:
   ```bash
   kill $SERVER_PID
   ```

**Success Criteria**:
- ✅ All four address formats work correctly
- ✅ Each format returns a mapped address

---

### Scenario 6: RFC 5389 Compliance (Test Vectors)

**Objective**: Verify STUN protocol implementation passes RFC 5769 test vectors.

**Steps**:

1. **Run RFC 5769 test vectors**:
   ```bash
   cargo test rfc5769 --release
   ```

   **Expected output**:
   ```
   running 4 tests
   test rfc5769_tests::test_vector_1_request ... ok
   test rfc5769_tests::test_vector_2_response ... ok
   test rfc5769_tests::test_vector_3_response ... ok
   test rfc5769_tests::test_vector_4_request ... ok
   
   test result: ok. 4 passed; 0 failed
   ```

2. **Verify test coverage**:
   ```bash
   cargo test --release -- --list | grep rfc5769
   ```

   **Expected**: At least 4 test cases listed (one per RFC 5769 section).

**Success Criteria**:
- ✅ All RFC 5769 test vectors pass
- ✅ Tests cover all four sample messages from RFC 5769

---

### Scenario 7: Directory Structure Validation

**Objective**: Verify project structure follows Rust workspace best practices.

**Steps**:

1. **Verify src/ structure**:
   ```bash
   ls -la src/
   ```

   **Expected**:
   ```
   bench/
   client/
   server/
   ```

2. **Verify crates/ structure**:
   ```bash
   ls -la crates/
   ```

   **Expected**:
   ```
   stun-protocol/
   ```

3. **Verify Cargo.toml workspace members**:
   ```bash
   grep -A 5 "workspace.members" Cargo.toml
   ```

   **Expected**:
   ```toml
   members = [
       "crates/stun-protocol",
       "src/server",
       "src/client",
       "src/bench",
   ]
   ```

4. **Verify each application has metadata.rs**:
   ```bash
   ls src/server/metadata.rs src/client/metadata.rs src/bench/metadata.rs
   ```

   **Expected**: All three files exist.

5. **Verify build.rs exists**:
   ```bash
   ls build.rs
   ```

   **Expected**: File exists.

**Success Criteria**:
- ✅ Application code is in src/server, src/client, src/bench
- ✅ Shared library is in crates/stun-protocol
- ✅ Cargo workspace configuration is correct
- ✅ Each application has metadata module
- ✅ Root build.rs exists

---

### Scenario 8: Build Metadata Capture

**Objective**: Verify build metadata (commit ID, compile time) is captured correctly.

**Steps**:

1. **Get current git commit**:
   ```bash
   CURRENT_COMMIT=$(git rev-parse HEAD)
   echo "Current commit: $CURRENT_COMMIT"
   ```

2. **Build and check startup log**:
   ```bash
   cargo build --release
   ./target/release/stunserver --bind 127.0.0.1:3478 2>&1 | head -1 | grep -oP 'commit=\K[a-f0-9]+'
   ```

   **Expected**: Commit ID matches `$CURRENT_COMMIT`.

3. **Verify compile time is recent**:
   ```bash
   COMPILE_TIME=$(./target/release/stunserver --bind 127.0.0.1:3478 2>&1 | head -1 | grep -oP 'compiled=\K[^\s]+')
   echo "Compile time: $COMPILE_TIME"
   ```

   **Expected**: Timestamp is within last few minutes.

4. **Test dirty build detection**:
   ```bash
   # Make a temporary change
   echo "// test" >> src/server/main.rs
   
   # Rebuild
   cargo build --release
   
   # Check if version includes "-dirty" (implementation-defined)
   ./target/release/stunserver --bind 127.0.0.1:3478 2>&1 | head -1
   
   # Revert change
   git checkout src/server/main.rs
   ```

   **Expected**: Version string MAY include "-dirty" suffix.

**Success Criteria**:
- ✅ Commit ID in startup log matches git HEAD
- ✅ Compile time is recent (within minutes of build)
- ✅ Dirty builds are detected (optional "-dirty" suffix)

---

### Scenario 9: Performance Validation

**Objective**: Verify startup log is output within 100ms of argument parsing.

**Steps**:

1. **Measure startup time**:
   ```bash
   time ./target/release/stunserver --bind 127.0.0.1:3478 --quiet &
   SERVER_PID=$!
   sleep 0.2
   kill $SERVER_PID
   ```

   **Expected**: Process starts and outputs log within 100ms.

2. **Verify with strace (Linux)**:
   ```bash
   strace -e trace=write -f ./target/release/stunserver --bind 127.0.0.1:3478 2>&1 | grep -A 2 "write(1," | head -20
   ```

   **Expected**: First write to stdout (fd 1) contains startup log.

**Success Criteria**:
- ✅ Startup log appears within 100ms of process start
- ✅ Log is output before network operations

---

### Scenario 10: End-to-End Integration Test

**Objective**: Verify complete workflow from build to STUN transaction.

**Steps**:

1. **Clean build**:
   ```bash
   cargo clean
   cargo build --release
   ```

2. **Start server**:
   ```bash
   ./target/release/stunserver --bind 127.0.0.1:3478 &
   SERVER_PID=$!
   sleep 1
   ```

3. **Run client**:
   ```bash
   ./target/release/stunclient 127.0.0.1:3478
   ```

   **Expected output**:
   ```
   stunclient v0.1.0 commit=... compiled=... started=...
   Connecting to 127.0.0.1:3478...
   Mapped address: 127.0.0.1:<port>
   ```

4. **Run benchmark**:
   ```bash
   ./target/release/stunbench --duration 5 --concurrency 2 --requests 10 127.0.0.1:3478
   ```

   **Expected output**:
   ```
   stunbench v0.1.0 commit=... compiled=... started=...
   Starting benchmark: 2 concurrent clients, 5 seconds, 10 requests/client
   Connecting to 127.0.0.1:3478...
   
   Benchmark Results:
     Total requests: 20
     Successful: 20 (100%)
     ...
   ```

5. **Clean up**:
   ```bash
   kill $SERVER_PID
   ```

**Success Criteria**:
- ✅ Server starts and outputs startup log
- ✅ Client connects and retrieves mapped address
- ✅ Benchmark runs successfully
- ✅ All three applications work together

---

## Validation Checklist

Use this checklist to track validation progress:

- [ ] **Scenario 1**: Startup log format and timing
- [ ] **Scenario 2**: Version information in --help
- [ ] **Scenario 3**: Default port 3478
- [ ] **Scenario 4**: DNS resolution
- [ ] **Scenario 5**: Address format support
- [ ] **Scenario 6**: RFC 5389 compliance (test vectors)
- [ ] **Scenario 7**: Directory structure validation
- [ ] **Scenario 8**: Build metadata capture
- [ ] **Scenario 9**: Performance validation
- [ ] **Scenario 10**: End-to-end integration test

## Troubleshooting

### Build Fails

**Problem**: `cargo build` fails with compilation errors.

**Solution**:
1. Ensure Rust toolchain is up to date: `rustup update`
2. Check that all dependencies are available: `cargo fetch`
3. Verify directory structure matches specification

### Tests Fail

**Problem**: `cargo test` fails.

**Solution**:
1. Run tests with verbose output: `cargo test -- --nocapture`
2. Check that RFC 5769 test vector files are present
3. Verify stun-protocol library is correctly referenced

### Startup Log Not Appearing

**Problem**: No startup log output.

**Solution**:
1. Check that --quiet flag is NOT set
2. Verify build.rs is capturing metadata correctly
3. Check that metadata.rs is calling `log_startup()`

### DNS Resolution Fails

**Problem**: stunclient cannot resolve domain names.

**Solution**:
1. Check network connectivity: `ping stunserver2025.stunprotocol.org`
2. Verify system DNS configuration: `cat /etc/resolv.conf`
3. Test with IP address instead of domain name

### Port Binding Fails

**Problem**: stunserver cannot bind to port 3478.

**Solution**:
1. Check if port is already in use: `netstat -tlnp | grep 3478`
2. Try a different port: `stunserver --bind 0.0.0.0:3479`
3. Check permissions (ports < 1024 require root on Unix)

## Next Steps

After all validation scenarios pass:
1. Run `/speckit-tasks` to generate detailed task breakdown
2. Run `/speckit-implement` to execute implementation
3. Run `/verify` before marking tasks complete
