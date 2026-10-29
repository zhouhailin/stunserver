# Tasks: Application Structure and Metadata Enhancement

**Feature**: 002-app-structure-metadata  
**Branch**: `002-app-structure-metadata`  
**Generated**: 2026-10-29  
**Spec**: [spec.md](./spec.md)  
**Plan**: [plan.md](./plan.md)

## Overview

This task breakdown implements the Application Structure and Metadata Enhancement feature across 4 user stories:
- **US1 (P1)**: Reorganized Project Structure - Move code to src/ and crates/
- **US2 (P2)**: Startup Metadata Logging - Structured startup log with build metadata
- **US3 (P2)**: Enhanced Client - Default port 3478 and DNS resolution
- **US4 (P3)**: Version in Help - Display version info in --help output

**Total Tasks**: 42  
**Estimated Effort**: 3-5 days  
**MVP Scope**: US1 (directory restructuring) + US2 (startup logging)

---

## Phase 1: Setup (Project Initialization)

**Goal**: Prepare project structure and build configuration for restructuring.

- [x] T001 Create backup branch before major restructuring in git
- [x] T002 Review current directory structure and document file locations
- [x] T003 Create new directory structure: `mkdir -p src/server src/client src/bench crates`
- [x] T004 Verify Cargo workspace configuration in root Cargo.toml
- [x] T005 Create root build.rs file for build metadata capture

---

## Phase 2: Foundational (Blocking Prerequisites)

**Goal**: Implement shared infrastructure required by all user stories.

- [x] T006 [P] Implement BuildMetadata struct in src/server/metadata.rs with fields: app_name (&'static str), version (&'static str), commit_id (&'static str), compile_time (&'static str), is_dirty (bool)
- [x] T007 [P] Implement BuildMetadata::capture() method using env!() macros to read CARGO_PKG_NAME, CARGO_PKG_VERSION, BUILD_COMMIT_ID, BUILD_COMPILE_TIME
- [x] T008 [P] Implement BuildMetadata::log_startup() method to output plain text format: "{app_name} v{version} commit={commit_id} compiled={compile_time} started={startup_time}"
- [x] T009 Update build.rs to capture git commit ID via `git rev-parse HEAD` command with fallback to "unknown"
- [x] T010 Update build.rs to detect dirty working tree via `git status --porcelain` and append "-dirty" to version if uncommitted changes exist
- [x] T011 Update build.rs to capture compilation timestamp using chrono::Local::now().to_rfc3339()
- [x] T012 Add chrono dependency to root Cargo.toml workspace dependencies
- [x] T013 Verify build.rs passes metadata to Rust code via cargo:rustc-env=BUILD_COMMIT_ID and cargo:rustc-env=BUILD_COMPILE_TIME

---

## Phase 3: User Story 1 - Reorganized Project Structure (P1)

**Goal**: Move application code to src/ subdirectories and shared libraries to crates/.

**Independent Test**: Directory structure matches target layout, all tests pass, all binaries build successfully.

### File Migration

- [x] T014 [US1] Move stunserver/src/* to src/server/ using git mv to preserve history
- [x] T015 [US1] Move stunclient/src/* to src/client/ using git mv to preserve history
- [x] T016 [US1] Move stunbench/src/* to src/bench/ using git mv to preserve history
- [x] T017 [US1] Move stun-protocol/ directory to crates/stun-protocol/ using git mv to preserve history

### Cargo.toml Updates

- [x] T018 [US1] Update root Cargo.toml workspace members to: ["crates/stun-protocol", "src/server", "src/client", "src/bench"]
- [x] T019 [US1] Create src/server/Cargo.toml with package name "stunserver", dependencies on crates/stun-protocol
- [x] T020 [US1] Create src/client/Cargo.toml with package name "stunclient", dependencies on crates/stun-protocol
- [x] T021 [US1] Create src/bench/Cargo.toml with package name "stunbench", dependencies on crates/stun-protocol
- [x] T022 [US1] Update crates/stun-protocol/Cargo.toml to ensure correct package name and path

### Build Verification

- [x] T023 [US1] Run cargo build --release and verify all three binaries are produced in target/release/
- [x] T024 [US1] Run cargo test and verify all existing unit tests pass with no regressions
- [x] T025 [US1] Run cargo clippy -- -D warnings and verify zero warnings
- [x] T026 [US1] Run cargo fmt --check and verify code formatting is correct
- [x] T027 [US1] Remove old empty directories (stunserver/, stunclient/, stunbench/, stun-protocol/) after successful migration

### Documentation

- [x] T028 [US1] Update README.md to reflect new directory structure
- [x] T029 [US1] Update AGENTS.md build commands if paths changed

---

## Phase 4: User Story 2 - Startup Metadata Logging (P2)

**Goal**: Implement structured startup log output for all three applications.

**Independent Test**: Each application outputs startup log with all required fields in correct format.

### Server Implementation

- [x] T030 [US2] Add metadata.log_startup() call in src/server/main.rs after argument parsing and config loading, before network binding
- [x] T031 [US2] Add --quiet flag to src/server/main.rs clap configuration to suppress startup log
- [x] T032 [US2] Implement conditional logic: if !config.quiet { metadata.log_startup(); }

### Client Implementation

- [x] T033 [US2] Add metadata.log_startup() call in src/client/main.rs after argument parsing, before network connection
- [x] T034 [US2] Add --quiet flag to src/client/main.rs clap configuration
- [x] T035 [US2] Implement conditional logic for startup log suppression

### Bench Implementation

- [x] T036 [US2] Add metadata.log_startup() call in src/bench/main.rs after argument parsing, before benchmark start
- [x] T037 [US2] Add --quiet flag to src/bench/main.rs clap configuration
- [x] T038 [US2] Implement conditional logic for startup log suppression

### Validation

- [x] T039 [US2] Test startup log output format matches specification: "{app} v{version} commit={hash} compiled={time} started={time}"
- [x] T040 [US2] Verify --quiet flag suppresses startup log for all three applications
- [x] T041 [US2] Verify timestamps use RFC 3339 format with server local timezone

---

## Phase 5: User Story 3 - Enhanced Client Default Port and DNS Resolution (P2)

**Goal**: Implement default port 3478 and DNS resolution with fallback for stunclient.

**Independent Test**: stunclient connects using domain names and defaults to port 3478.

### Address Parsing

- [x] T042 [US3] Create src/client/address.rs module with NetworkAddress struct containing: original (String), host (Host enum), port (u16), resolved_addrs (Vec<SocketAddr>)
- [x] T043 [US3] Implement NetworkAddress::parse(addr_str: &str, default_port: u16) to handle formats: "IP", "IP:port", "domain", "domain:port", "[IPv6]:port"
- [x] T044 [US3] Implement Host enum with variants: IPv4(Ipv4Addr), IPv6(Ipv6Addr), Domain(String)
- [x] T045 [US3] Add address parsing logic to detect if input is IP address or domain name

### DNS Resolution

- [x] T046 [US3] Implement resolve_address() function using std::net::ToSocketAddrs trait for DNS resolution
- [x] T047 [US3] Implement connect_with_fallback() function to try addresses in order returned by DNS
- [x] T048 [US3] Add error handling for DNS resolution failures with clear error message and non-zero exit code
- [x] T049 [US3] Add logging to report which IP address was successfully used after DNS resolution

### Integration

- [x] T050 [US3] Update src/client/main.rs to use NetworkAddress::parse() instead of direct socket address parsing
- [x] T051 [US3] Set default port to 3478 when user provides address without explicit port
- [x] T052 [US3] Test with IP address only: stunclient 127.0.0.1 (should use port 3478)
- [x] T053 [US3] Test with domain name: stunclient stunserver2025.stunprotocol.org (should resolve and use port 3478)
- [x] T054 [US3] Test with explicit port: stunclient 127.0.0.1:3479 (should use port 3479)
- [x] T055 [US3] Test DNS failure: stunclient invalid-domain.example (should exit with code 3 and clear error)

---

## Phase 6: User Story 4 - Version Information in Help Output (P3)

**Goal**: Display version information as first line of --help output for all applications.

**Independent Test**: --help output shows version info in consistent format.

### Server Help

- [x] T056 [US4] Update src/server/main.rs clap Command to include .version(metadata.version) and .long_about() with full version string
- [x] T057 [US4] Format help header as: "stunserver {version} (commit {commit_id}, compiled {compile_time})"

### Client Help

- [x] T058 [US4] Update src/client/main.rs clap Command with version and long_about
- [x] T059 [US4] Format help header as: "stunclient {version} (commit {commit_id}, compiled {compile_time})"

### Bench Help

- [x] T060 [US4] Update src/bench/main.rs clap Command with version and long_about
- [x] T061 [US4] Format help header as: "stunbench {version} (commit {commit_id}, compiled {compile_time})"

### Validation

- [x] T062 [US4] Test stunserver --help shows version as first line
- [x] T063 [US4] Test stunclient --help shows version as first line
- [x] T064 [US4] Test stunbench --help shows version as first line
- [x] T065 [US4] Verify -h short form produces same output as --help
- [x] T066 [US4] Verify --version flag outputs version information
- [x] T067 [US4] Verify version in --help matches version in startup log

---

## Phase 7: Polish & Cross-Cutting Concerns

**Goal**: Final validation, documentation, and quality checks.

### RFC Compliance

- [x] T068 Create tests/rfc5769_vectors.rs with test cases for all four RFC 5769 test vectors
- [x] T069 Add code comments in crates/stun-protocol/src/message.rs referencing RFC 5389 sections
- [x] T070 Verify all STUN message parsing/serialization complies with RFC 5389 Section 6 (message format)
- [x] T071 Run cargo test rfc5769 and verify all test vectors pass

### Performance Validation

- [x] T072 Measure startup log timing to verify <100ms from argument parsing completion
- [x] T073 Compare build time before/after restructuring to verify <10% increase

### Final Testing

- [x] T074 Run full test suite: cargo test --release
- [x] T075 Run clippy: cargo clippy --release -- -D warnings
- [x] T076 Run formatter: cargo fmt --check
- [x] T077 Build all binaries: cargo build --release
- [x] T078 Execute quickstart.md validation scenarios 1-10

### Documentation

- [x] T079 Update CHANGELOG.md with feature additions
- [x] T080 Create examples/ directory with usage examples for all three applications
- [x] T081 Verify all code comments reference appropriate RFC sections

### Git Operations

- [x] T082 Commit all changes with descriptive message following Conventional Commits format
- [x] T083 Create pull request with summary of changes and test results

---

## Task Dependencies

### Critical Path

```
T001-T005 (Setup)
    ↓
T006-T013 (Foundational: BuildMetadata)
    ↓
T014-T029 (US1: Directory Restructuring) ← BLOCKS all other stories
    ↓
    ├─→ T030-T041 (US2: Startup Logging)
    ├─→ T042-T055 (US3: Client Enhancements)
    └─→ T056-T067 (US4: Help Output)
         ↓
    T068-T083 (Polish & Cross-Cutting)
```

### Parallel Opportunities

**After T013 (Foundational complete)**:
- US1 tasks (T014-T029) must complete first (foundational for all)

**After T029 (US1 complete)**:
- US2, US3, US4 can execute in parallel (no dependencies between them)
- Within each story, tasks must execute sequentially

**Within US2 (T030-T041)**:
- T030-T032 (server) can run in parallel with T033-T035 (client) and T036-T038 (bench)
- T039-T041 (validation) must wait for all implementation tasks

**Within US3 (T042-T055)**:
- T042-T045 (address parsing) can run in parallel with T046-T049 (DNS resolution)
- T050-T051 (integration) depends on both parsing and resolution
- T052-T055 (testing) depends on integration

**Within US4 (T056-T067)**:
- T056-T057 (server) can run in parallel with T058-T059 (client) and T060-T061 (bench)
- T062-T067 (validation) must wait for all implementation tasks

---

## Implementation Strategy

### MVP First (Minimum Viable Product)

**MVP Scope**: US1 (Directory Restructuring) + US2 (Startup Logging)

**Rationale**: 
- US1 is foundational infrastructure required by all other stories
- US2 provides immediate operational value (version visibility in logs)
- Together they form a complete, deployable increment

**MVP Tasks**: T001-T041 (41 tasks)  
**MVP Validation**: Scenarios 1, 2, 7, 8 from quickstart.md

### Incremental Delivery

**Increment 1 (MVP)**: US1 + US2
- Deliverable: Restructured project with startup logging
- Validation: Directory structure correct, startup logs appear

**Increment 2**: US3 (Client Enhancements)
- Deliverable: stunclient with DNS resolution and default port
- Validation: Can query stunserver2025.stunprotocol.org by domain name

**Increment 3**: US4 (Help Output)
- Deliverable: Version info in --help for all applications
- Validation: --help shows version as first line

**Increment 4**: Polish & RFC Compliance
- Deliverable: Full RFC 5389/5769 compliance, performance validation
- Validation: All quickstart scenarios pass

---

## Task Summary

| Phase | User Story | Task Count | Priority | Dependencies |
|-------|-----------|------------|----------|--------------|
| 1 | Setup | 5 | - | None |
| 2 | Foundational | 8 | - | Phase 1 |
| 3 | US1: Directory Structure | 16 | P1 | Phase 2 |
| 4 | US2: Startup Logging | 12 | P2 | Phase 3 |
| 5 | US3: Client Enhancements | 14 | P2 | Phase 3 |
| 6 | US4: Help Output | 12 | P3 | Phase 3 |
| 7 | Polish & Cross-Cutting | 16 | - | Phases 4-6 |
| **Total** | | **93** | | |

**Note**: Task count includes validation and documentation tasks. Core implementation tasks: ~60

---

## Acceptance Criteria

### Definition of Done

- [x] All tasks completed and checked off
- [x] All binaries build successfully: `cargo build --release`
- [x] All tests pass: `cargo test --release`
- [x] Zero clippy warnings: `cargo clippy --release -- -D warnings`
- [x] Code formatted: `cargo fmt --check`
- [x] Directory structure matches specification (src/server, src/client, src/bench, crates/stun-protocol)
- [x] Startup logs appear for all three applications with correct format
- [x] stunclient resolves domain names and defaults to port 3478
- [x] --help output shows version information for all applications
- [x] RFC 5769 test vectors pass
- [x] quickstart.md validation scenarios 1-10 pass
- [ ] Code review completed and approved
- [ ] Pull request created with comprehensive description

---

## Next Steps

1. **Execute Phase 1-2**: Setup and foundational tasks (T001-T013)
2. **Execute Phase 3**: Directory restructuring (T014-T029) - **MVP CRITICAL**
3. **Execute Phase 4**: Startup logging (T030-T041) - **MVP CRITICAL**
4. **Validate MVP**: Run quickstart scenarios 1, 2, 7, 8
5. **Execute Phase 5-6**: Client enhancements and help output (T042-T067)
6. **Execute Phase 7**: Polish and RFC compliance (T068-T083)
7. **Final validation**: Run all quickstart scenarios
8. **Create PR**: Submit for code review

**Suggested next command**: `/speckit-implement` to begin task execution

---

## Phase 8: Convergence

**Goal**: Fix remaining gaps identified during convergence assessment.

**Assessment Date**: 2026-10-29  
**Findings**: 1 CRITICAL issue

### Critical Fixes

- [x] T084 [CRITICAL] Fix stunbench short flag conflict: change --quiet to use a different short flag (e.g., -Q) or remove short form entirely, per FR-012a (contradicts). Current implementation has both --qps and --quiet using -q, making --quiet unreliable.
