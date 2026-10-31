<!--
Sync Impact Report
==================
Version change: 1.0.0 → 1.1.0 (MINOR: materially expanded quality-gate
guidance and added a CI parity principle)
Modified principles:
  - IV. Code Quality Gates: commands now match GitHub Actions exactly
    (all targets/features, formatting check, workspace-wide tests)
Added principles:
  - VI. CI Parity and Pre-Push Verification
Modified sections:
  - Technology Stack: linting scope clarified
  - Development Workflow: PR verification commands aligned with CI
Removed sections: N/A
Follow-up TODOs: None
-->

# STUN Server Constitution

## Core Principles

### I. RFC 5389 Compliance

All STUN protocol implementations MUST strictly conform to RFC 5389
(Session Traversal Utilities for NAT). Any deviation from the RFC
MUST be documented with justification and reviewed for interoperability
impact. Protocol attributes, message types, and error codes MUST match
the RFC specification exactly.

**Rationale**: STUN is a standardized protocol. Non-compliant
implementations will fail to interoperate with other STUN clients
and servers in the ecosystem.

### II. Test-Driven Development (NON-NEGOTIABLE)

All features and bug fixes MUST follow TDD discipline:

1. Write failing tests first that define expected behavior
2. Verify tests fail for the right reason
3. Implement minimum code to make tests pass
4. Refactor while keeping tests green

No implementation code SHALL be written without a corresponding
failing test. Test coverage MUST NOT decrease.

**Rationale**: TDD ensures correctness, prevents regressions, and
serves as living documentation. For a protocol implementation,
tests are the primary defense against subtle interoperability bugs.

### III. Performance-First Design

As a high-performance STUN server, all design decisions MUST consider
performance impact:

- Network I/O MUST use async/non-blocking patterns
- Memory allocations in hot paths MUST be minimized
- Connection handling MUST scale to thousands of concurrent clients
- Benchmark critical paths and set performance regression thresholds

**Rationale**: STUN servers operate on the critical path of real-time
communication. Latency and throughput directly affect user experience.

### IV. Code Quality Gates

All code MUST pass the following quality gates before merge:

- `cargo fmt --all -- --check` — formatting MUST match rustfmt defaults
- `cargo clippy --all-targets --all-features -- -D warnings` — zero
  clippy warnings allowed, including in test, benchmark, and example
  targets
- `cargo test --all` — all tests in the workspace MUST pass
- No `unsafe` blocks without explicit justification and review

**Rationale**: Consistent code quality reduces review burden and
prevents entire categories of bugs. Rust's type system is our
primary safety net; `unsafe` bypasses that guarantee. Linting test
targets matters because CI checks them even when local builds do not.

### V. Simplicity and YAGNI

Start with the simplest implementation that satisfies the requirement.
Do not add abstraction, features, or optimization until a concrete
need is demonstrated. Every line of code is a liability — minimize it.

**Rationale**: Premature optimization and over-engineering are the
primary sources of complexity. A simple, correct implementation
is always preferred over a clever, complex one.

### VI. CI Parity and Pre-Push Verification

Local verification MUST mirror the GitHub Actions CI before a commit
is pushed to a shared branch. The minimum local gate is:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo build --release`

A successful `cargo build` or `cargo test` alone is NOT sufficient
evidence: clippy warnings in `#[cfg(test)]` code still fail CI.
When a change touches `docker/Dockerfile`, the Rust toolchain version,
or `Cargo.lock`, a Docker build MUST be verified before pushing, and
the builder toolchain MUST be able to parse the committed
`Cargo.lock` format. A commit with known failing CI-equivalent checks
MUST NOT be pushed.

**Rationale**: Past CI failures came from clippy lints in test-only
code and from a Docker builder whose Cargo version could not read the
committed lockfile format. Running the exact CI commands locally
prevents avoidable red builds and keeps `main` verifiable.

## Technology Stack

- **Language**: Rust (stable toolchain, latest edition)
- **Build**: Cargo (standard build, test, fmt, clippy commands)
- **Async Runtime**: Tokio (for network I/O)
- **Testing**: cargo test + integration tests in `tests/`
- **Linting**: clippy across all targets and features, warnings as errors
- **Formatting**: rustfmt with default configuration

## Development Workflow

- **Commits**: Conventional Commits format (`feat:`, `fix:`, `docs:`,
  `test:`, `refactor:`, `chore:`)
- **Branches**: `type/description` (e.g., `feat/stun-binding`,
  `fix/attribute-parsing`)
- **Pull Requests**: All changes require review. Verify:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all`
  - Docker build when Dockerfile, toolchain, or lockfile changes
    (Principle VI)
  - RFC compliance verified for protocol changes
- **Verification**: Run `/verify` skill before marking tasks complete

## Governance

This constitution supersedes all other development practices and
conventions. Amendments require:

1. Documentation of the proposed change via pull request
2. Rationale explaining why the change is necessary
3. Migration plan if the change affects existing code
4. Approval from project maintainers

All contributors MUST verify compliance with this constitution
during code review. Violations MUST be fixed before merge.

**Version**: 1.1.0 | **Ratified**: 2026-10-29 | **Last Amended**: 2026-10-31
