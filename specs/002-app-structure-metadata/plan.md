# Implementation Plan: Application Structure and Metadata Enhancement

**Branch**: `002-app-structure-metadata` | **Date**: 2026-10-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/002-app-structure-metadata/spec.md`

## Summary

Reorganize the STUN server project structure to follow Rust workspace best practices (moving application code to `src/` and shared libraries to `crates/`), implement structured startup logging with build metadata, enhance stunclient with default port 3478 and DNS resolution support, and add version information to --help output. All implementations must comply with RFC 5389 and RFC 5769 (mandatory) and reference RFC 5780 (optional extension).

## Technical Context

**Language/Version**: Rust 1.75+ (edition 2021)

**Primary Dependencies**: 
- tokio 1.35 (async runtime for network I/O)
- clap 4.4 (command-line argument parsing)
- chrono 0.4 (timezone-aware datetime formatting)
- log 0.4 + env_logger 0.10 (logging facade)
- anyhow 1.0 + thiserror 1.0 (error handling)
- serde 1.0 + serde_yaml 0.9 (configuration serialization)
- byteorder 1.5 (binary protocol handling)

**Storage**: N/A (stateless network applications)

**Testing**: cargo test (unit tests) + integration tests in `tests/` directory

**Target Platform**: Linux server (x86_64), with cross-platform compatibility for macOS and Windows

**Project Type**: CLI tools (3 binaries: stunserver, stunclient, stunbench) + shared library (stun-protocol)

**Performance Goals**: 
- Startup log output within 100ms of argument parsing completion
- Build time increase <10% after restructuring
- Zero performance regression in STUN protocol handling

**Constraints**: 
- RFC 5389 and RFC 5769 strict compliance (mandatory)
- RFC 5780 compliance (optional extension)
- Zero clippy warnings
- 100% test coverage maintenance
- Backward-compatible CLI interfaces

**Scale/Scope**: 
- 3 binaries (stunserver, stunclient, stunbench)
- 1 shared library (stun-protocol → crates/stun-protocol)
- 4 application directories (src/server, src/client, src/bench, plus shared utilities if needed)
- 26 functional requirements
- 12 success criteria

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Principle I: RFC 5389 Compliance ✅ PASS

- All STUN protocol implementations will strictly conform to RFC 5389
- RFC 5769 test vectors will be used for validation
- RFC 5780 support is optional and clearly marked
- RFC 3489 is obsoleted and will not conflict with RFC 5389

**Verification**: Code review will confirm protocol-related code references specific RFC sections. Integration tests will validate against RFC 5769 test vectors.

### Principle II: Test-Driven Development (NON-NEGOTIABLE) ✅ PASS

- All new features will have failing tests written first
- Directory restructuring will maintain 100% test pass rate
- New functionality (startup logging, DNS resolution, --help output) will have corresponding tests
- Test coverage will not decrease

**Verification**: `cargo test` will pass with all existing and new tests. New tests will be added for:
- Startup log format and timing
- --quiet flag behavior
- DNS resolution with multiple IPs
- Default port 3478 behavior
- --help output format

### Principle III: Performance-First Design ✅ PASS

- Startup logging uses async/non-blocking I/O patterns
- DNS resolution uses system resolver (no additional overhead)
- Directory restructuring has no runtime performance impact
- Build time impact monitored and kept <10%

**Verification**: Startup log timing measured to be <100ms. Build time compared before/after restructuring.

### Principle IV: Code Quality Gates ✅ PASS

- All code will pass `cargo fmt` (rustfmt defaults)
- All code will pass `cargo clippy -- -D warnings` (zero warnings)
- All tests will pass (`cargo test`)
- No `unsafe` blocks without explicit justification

**Verification**: CI pipeline will enforce all quality gates. Pre-commit hooks recommended.

### Principle V: Simplicity and YAGNI ✅ PASS

- Directory structure follows standard Rust workspace conventions
- Startup logging uses existing log/env_logger infrastructure
- DNS resolution uses standard library ToSocketAddrs
- No premature abstractions or over-engineering

**Verification**: Code review will confirm minimal, focused implementations.

### Gate Result: ✅ ALL GATES PASS

Proceeding to Phase 0 research.

## Project Structure

### Documentation (this feature)

```text
specs/002-app-structure-metadata/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── cli-interface.md # CLI argument schemas
│   └── log-format.md    # Startup log format specification
└── tasks.md             # Phase 2 output (NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
# Target structure after restructuring

crates/
└── stun-protocol/       # Shared STUN protocol library (moved from stun-protocol/)
    ├── Cargo.toml
    ├── src/
    │   ├── lib.rs
    │   ├── message.rs   # STUN message parsing/serialization
    │   ├── attribute.rs # STUN attributes
    │   └── error.rs     # Protocol error types
    └── tests/
        ├── rfc5769_vectors.rs  # RFC 5769 test vector validation
        └── protocol_tests.rs

src/
├── server/              # stunserver application (moved from stunserver/)
│   ├── main.rs          # Entry point
│   ├── config.rs        # Server configuration
│   ├── metadata.rs      # Build metadata (version, commit, timestamps)
│   └── server.rs        # Server logic
│
├── client/              # stunclient application (moved from stunclient/)
│   ├── main.rs          # Entry point
│   ├── config.rs        # Client configuration
│   ├── metadata.rs      # Build metadata
│   ├── address.rs       # Address parsing and DNS resolution
│   └── client.rs        # Client logic
│
└── bench/               # stunbench application (moved from stunbench/)
    ├── main.rs          # Entry point
    ├── config.rs        # Benchmark configuration
    ├── metadata.rs      # Build metadata
    └── bench.rs         # Benchmark logic

# Build scripts for metadata capture
build.rs               # Root build script (captures git info, timestamps)

# Configuration
config/
└── default.yaml       # Default configuration files

# Integration tests
tests/
├── integration_test.rs
└── cli_tests.rs

# Documentation
docs/
└── rfcs/              # RFC reference documents
    ├── rfc3489.txt
    ├── rfc5389.txt
    ├── rfc5769.txt
    └── rfc5780.txt
```

**Structure Decision**: Adopt Rust workspace best practices with shared libraries in `crates/` and application code in `src/`. Each application (server, client, bench) has its own subdirectory under `src/` with a clear separation of concerns. The stun-protocol library moves to `crates/stun-protocol/` to indicate it's a shared dependency.

## Complexity Tracking

> **No violations detected. All constitution gates pass.**

## Phase 0: Research Findings

See [research.md](./research.md) for detailed research on:
- Build metadata capture strategies
- Startup log format and timing
- DNS resolution patterns
- CLI version display conventions
- RFC compliance verification approaches

## Phase 1: Design Artifacts

See the following artifacts for detailed design:
- [data-model.md](./data-model.md) - Entity definitions and relationships
- [contracts/cli-interface.md](./contracts/cli-interface.md) - CLI argument schemas
- [contracts/log-format.md](./contracts/log-format.md) - Startup log format specification
- [quickstart.md](./quickstart.md) - Validation guide for end-to-end testing

## Next Steps

After this plan is approved:
1. Run `/speckit-tasks` to generate detailed task breakdown
2. Run `/speckit-implement` to execute the implementation
3. Run `/verify` before marking tasks complete
