# Quickstart Validation Guide: Rust STUN Server Project

**Date**: 2026-10-29 | **Feature**: 001-rust-stun-project-init

## Overview

This guide provides step-by-step validation scenarios to verify the STUN server project works correctly end-to-end. Each scenario tests a specific aspect of the system and can be run independently.

---

## Prerequisites

### Required Software

- Rust toolchain (stable, 1.75 or later)
- Git
- cargo-tarpaulin (for coverage testing)

### Installation

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install cargo-tarpaulin for coverage
cargo install cargo-tarpaulin

# Clone repository
git clone <repository-url>
cd stunserver
```

---

## Validation Scenarios

### Scenario 1: Build and Basic Execution

**Objective**: Verify all three executables build successfully and display build metadata.

**Steps**:

```bash
# Build all executables
cargo build --release

# Verify executables exist
ls -lh target/release/stunserver
ls -lh target/release/stunclient
ls -lh target/release/stunbench

# Run stunserver (will fail to bind if port 3478 in use, but shows metadata)
./target/release/stunserver --listen 127.0.0.1:13478

# In another terminal, check version
./target/release/stunclient --version
./target/release/stunbench --version
```

**Expected Output**:

```
STUN Server v0.1.0
Commit: abc1234
Commit Time: 2026-10-29T10:00:00Z (UTC)
Build Time: 2026-10-29T14:00:00Z (UTC)
Startup Time: 2026-10-29T14:30:00Z (UTC)
Startup Time: 2026-10-29T22:30:00+08:00 (Asia/Shanghai)
Listening on 127.0.0.1:13478
```

**Success Criteria**:
- ✅ All three executables build without errors
- ✅ Build metadata displayed correctly (version, commit ID, timestamps)
- ✅ Server starts and listens on specified address

---

### Scenario 2: STUN Protocol Compliance

**Objective**: Verify server correctly handles STUN binding requests per RFC 5389.

**Steps**:

```bash
# Terminal 1: Start server
./target/release/stunserver --listen 127.0.0.1:13478

# Terminal 2: Send binding request
./target/release/stunclient 127.0.0.1:13478
```

**Expected Output**:

```
STUN Client v0.1.0
...
Sending binding request to 127.0.0.1:13478...
Response received in 1.23ms
Mapped Address: 127.0.0.1:54321
```

**Success Criteria**:
- ✅ Client sends STUN binding request
- ✅ Server responds with valid STUN binding response
- ✅ Response includes MAPPED-ADDRESS attribute
- ✅ Response time < 100ms (local network)

**Validation**:
- Check server logs for request processing
- Verify response format matches RFC 5389
- Confirm transaction ID matches between request and response

---

### Scenario 3: Configuration File Loading

**Objective**: Verify server loads and applies YAML configuration correctly.

**Steps**:

```bash
# Create test configuration
cat > /tmp/test-config.yaml << 'EOF'
server:
  listen: "127.0.0.1:23478"

logging:
  level: "debug"
  output: "console"
EOF

# Start server with configuration
./target/release/stunserver --config /tmp/test-config.yaml
```

**Expected Output**:

```
STUN Server v0.1.0
...
Listening on 127.0.0.1:23478
[DEBUG] Server initialized with debug logging
```

**Success Criteria**:
- ✅ Configuration file loaded successfully
- ✅ Server listens on configured address (127.0.0.1:23478)
- ✅ Debug logging enabled as configured

**Validation**:
- Verify server uses configured listen address
- Check log level matches configuration
- Test with invalid configuration to verify error handling

---

### Scenario 4: File Logging with Rotation

**Objective**: Verify file logging works with size and date-based rotation.

**Steps**:

```bash
# Create configuration with file logging
cat > /tmp/file-log-config.yaml << 'EOF'
server:
  listen: "127.0.0.1:33478"

logging:
  level: "info"
  output: "file"
  file:
    path: "/tmp/stunserver-test.log"
    max_size: "1MB"
    rotation: "daily"
    max_files: 3
EOF

# Start server
./target/release/stunserver --config /tmp/file-log-config.yaml

# Generate some log entries (send requests from client)
for i in {1..100}; do
  ./target/release/stunclient 127.0.0.1:33478 > /dev/null 2>&1
done

# Check log file
ls -lh /tmp/stunserver-test.log*
tail -20 /tmp/stunserver-test.log
```

**Expected Output**:

```
-rw-r--r-- 1 user user 15K Oct 29 14:35 /tmp/stunserver-test.log

[2026-10-29T14:35:00Z] [INFO] Server started on 127.0.0.1:33478
[2026-10-29T14:35:01Z] [INFO] Processed binding request from 127.0.0.1:54321
...
```

**Success Criteria**:
- ✅ Log file created at specified path
- ✅ Log entries written in correct format
- ✅ Log level filtering works (only info and above)

**Validation**:
- Verify log file format: `[timestamp] [level] message`
- Check log rotation works (create large log to trigger size rotation)
- Verify max_files limit enforced (old files deleted)

---

### Scenario 5: Performance Benchmark

**Objective**: Verify server handles 10,000 QPS as required.

**Steps**:

```bash
# Terminal 1: Start server
./target/release/stunserver --listen 127.0.0.1:43478

# Terminal 2: Run benchmark
./target/release/stunbench --qps 10000 --duration 60 127.0.0.1:43478
```

**Expected Output**:

```
STUN Benchmark v0.1.0
...
Starting benchmark: 10000 QPS for 60 seconds
Target: 127.0.0.1:43478

[00:10] Sent: 100000, Success: 99995, Failed: 5, QPS: 10000
[00:20] Sent: 200000, Success: 199990, Failed: 10, QPS: 10000
...

Benchmark Complete
==================
Duration: 60.0s
Target QPS: 10000
Actual QPS: 9998.45
Total Requests: 599907
Successful: 599877 (99.99%)
Failed: 30

Latency Percentiles:
  p50: 0.45ms
  p95: 1.12ms
  p99: 2.34ms
```

**Success Criteria**:
- ✅ Actual QPS >= 10,000 (within 5% tolerance)
- ✅ Success rate >= 99% (< 1% error rate)
- ✅ p95 latency < 100ms
- ✅ Benchmark completes without errors

**Validation**:
- Monitor server CPU and memory usage during benchmark
- Check server logs for any errors
- Verify no request drops or timeouts

---

### Scenario 6: Dual-Stack IPv4/IPv6 Support

**Objective**: Verify server handles both IPv4 and IPv6 addresses.

**Steps**:

```bash
# Terminal 1: Start server with IPv6
./target/release/stunserver --listen "[::]:53478"

# Terminal 2: Test IPv4 client
./target/release/stunclient 127.0.0.1:53478

# Terminal 3: Test IPv6 client
./target/release/stunclient "[::1]:53478"
```

**Expected Output**:

Both clients should receive successful responses:

```
Sending binding request to 127.0.0.1:53478...
Response received in 1.23ms
Mapped Address: 127.0.0.1:54321

Sending binding request to [::1]:53478...
Response received in 1.45ms
Mapped Address: [::1]:54322
```

**Success Criteria**:
- ✅ Server binds to IPv6 address
- ✅ IPv4 client can connect to dual-stack server
- ✅ IPv6 client can connect to server
- ✅ Both receive valid STUN responses

**Validation**:
- Verify server logs show both IPv4 and IPv6 connections
- Check network statistics for both protocols

---

### Scenario 7: Error Handling

**Objective**: Verify proper error handling for various failure scenarios.

**Steps**:

```bash
# Test 1: Invalid configuration file
./target/release/stunserver --config /nonexistent/config.yaml

# Test 2: Port already in use
./target/release/stunserver --listen 127.0.0.1:3478 &
./target/release/stunserver --listen 127.0.0.1:3478  # Should fail

# Test 3: Invalid server address (client)
./target/release/stunclient invalid-address

# Test 4: Server unreachable (client)
./target/release/stunclient 192.0.2.1:3478  # Non-routable address
```

**Expected Output**:

```
# Test 1
Error: Configuration file not found: /nonexistent/config.yaml

# Test 2
Error: Failed to bind to 127.0.0.1:3478: Address already in use

# Test 3
Error: Invalid server address: invalid IP address syntax

# Test 4
Error: Connection timeout after 5 seconds
```

**Success Criteria**:
- ✅ Clear error messages for each failure scenario
- ✅ Non-zero exit codes for errors
- ✅ No crashes or panics
- ✅ Actionable error messages (suggest fixes)

**Validation**:
- Verify exit codes are correct
- Check error messages are user-friendly
- Ensure no sensitive information leaked in errors

---

### Scenario 8: Test Coverage

**Objective**: Verify unit test coverage meets 95% requirement.

**Steps**:

```bash
# Run all tests
cargo test

# Generate coverage report
cargo tarpaulin --out Html

# Check coverage percentage
cargo tarpaulin --out Stdout | grep "Coverage"
```

**Expected Output**:

```
Running tests...
test result: ok. 150 passed; 0 failed; 0 ignored

Coverage: 96.5%
```

**Success Criteria**:
- ✅ All tests pass
- ✅ Coverage >= 95%
- ✅ No test failures or ignored tests

**Validation**:
- Review coverage report (tarpaulin-output.html)
- Identify uncovered lines and add tests if needed
- Verify critical paths have 100% coverage

---

### Scenario 9: Code Quality Gates

**Objective**: Verify code passes all quality checks.

**Steps**:

```bash
# Check formatting
cargo fmt --check

# Run clippy
cargo clippy -- -D warnings

# Build in release mode
cargo build --release
```

**Expected Output**:

All commands should complete successfully with no errors or warnings.

**Success Criteria**:
- ✅ `cargo fmt --check` passes (no formatting issues)
- ✅ `cargo clippy` passes with zero warnings
- ✅ Release build completes without errors

**Validation**:
- Fix any formatting issues: `cargo fmt`
- Fix any clippy warnings before proceeding
- Verify no unsafe blocks without justification

---

### Scenario 10: Systemd Service Integration

**Objective**: Verify server can run as systemd service (Linux only).

**Steps**:

```bash
# Copy service file (requires root)
sudo cp systemd/stunserver.service /etc/systemd/system/

# Copy binary
sudo cp target/release/stunserver /usr/local/bin/

# Create config directory
sudo mkdir -p /etc/stunserver
sudo cp config/example.yaml /etc/stunserver/config.yaml

# Reload systemd
sudo systemctl daemon-reload

# Start service
sudo systemctl start stunserver

# Check status
sudo systemctl status stunserver

# View logs
sudo journalctl -u stunserver -f

# Stop service
sudo systemctl stop stunserver
```

**Expected Output**:

```
● stunserver.service - STUN Server
   Loaded: loaded (/etc/systemd/system/stunserver.service; disabled)
   Active: active (running) since ...
   Main PID: 12345 (stunserver)
   ...
```

**Success Criteria**:
- ✅ Service starts successfully
- ✅ Service runs in background
- ✅ Logs visible in journalctl
- ✅ Service can be stopped/started/restarted

**Validation**:
- Verify service auto-restarts on failure
- Check service runs as non-root user
- Test service enablement: `sudo systemctl enable stunserver`

---

## Continuous Integration Validation

### GitHub Actions Workflow

The CI workflow automatically validates:

1. **Build**: All executables build on all platforms
2. **Test**: All unit and integration tests pass
3. **Format**: Code formatting matches rustfmt
4. **Lint**: Zero clippy warnings
5. **Coverage**: Test coverage >= 95%

**View CI Status**: Check GitHub Actions tab in repository

---

## Troubleshooting

### Issue: Port already in use

**Solution**: Use a different port or stop the process using the port.

```bash
# Find process using port
sudo lsof -i :3478

# Or use different port
./target/release/stunserver --listen 0.0.0.0:8080
```

### Issue: Permission denied binding to port < 1024

**Solution**: Use port >= 1024 or run with elevated privileges (not recommended).

```bash
# Use non-privileged port
./target/release/stunserver --listen 0.0.0.0:3478
```

### Issue: Configuration file not found

**Solution**: Provide correct path or create configuration file.

```bash
# Check file exists
ls -l /path/to/config.yaml

# Create minimal config
cat > config.yaml << 'EOF'
server:
  listen: "0.0.0.0:3478"
logging:
  level: "info"
  output: "console"
EOF
```

### Issue: Test coverage below 95%

**Solution**: Add tests for uncovered code paths.

```bash
# Generate detailed coverage report
cargo tarpaulin --out Html

# Review tarpaulin-output.html and add tests for uncovered lines
```

---

## Success Summary

After completing all validation scenarios, you should have:

- ✅ Three working executables (stunserver, stunclient, stunbench)
- ✅ RFC 5389 compliant STUN protocol implementation
- ✅ 10,000+ QPS performance achieved
- ✅ 95%+ test coverage
- ✅ Zero clippy warnings
- ✅ Comprehensive logging (console and file)
- ✅ YAML configuration support
- ✅ Dual-stack IPv4/IPv6 support
- ✅ Build metadata display
- ✅ Systemd service integration (Linux)

**Next Steps**: Proceed to `/speckit-tasks` for detailed implementation tasks.
