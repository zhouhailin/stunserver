# Implementation Plan: STUN RFC 5389/5769/5780 Conformance Audit and Remediation

**Branch**: `008-rfc-compliance-audit` | **Date**: 2026-10-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/008-rfc-compliance-audit/spec.md`

## Summary

Produce a traceable RFC 5389/5769/5780 conformance matrix and close the gaps the audit found: complete the attribute registry, distinguish known from unknown attributes, enforce FINGERPRINT and MESSAGE-INTEGRITY ordering, add SOFTWARE to responses, make client transaction IDs unpredictable, verify the RFC 5769 integrity vectors, and add RFC 5389 §10 authentication to both server and client using short-term and long-term credentials selected by configuration. All work follows TDD and preserves the existing UDP-only, single-address design.

## Technical Context

**Language/Version**: Rust 2021, stable toolchain (CI builder is Rust 1.96)

**Primary Dependencies**: Existing workspace crates `tokio 1.35`, `serde 1.0`/`toml 0.8`, `clap 4.4`, `log 0.4`/`env_logger 0.10`, `chrono 0.4`, `thiserror 1.0`, `byteorder 1.5`, `flate2 1.1`. New: `hmac 0.12`, `sha1 0.10`, `md-5 0.10`, `subtle 2.5`, `crc32fast 1.4`, `stringprep 0.1` (SASLprep profile), `rand 0.8` (OsRng).

**Storage**: TOML configuration file (`config/stunserver.conf`); credentials and precomputed keys are loaded once into memory. No database.

**Testing**: `cargo test --all` (workspace unit and integration tests); protocol vectors in `crates/stun-protocol/tests/`; server/client integration tests; benchmark via `stunbench` for the latency gate.

**Target Platform**: Linux server (UDP); `stunclient` and `stunbench` are portable CLI binaries.

**Project Type**: Rust workspace — protocol library, server binary, client binary, benchmark tool.

**Performance Goals**: Preserve existing request throughput; FINGERPRINT and MESSAGE-INTEGRITY processing must not increase p99 response latency by more than 5% (SC-008). Hash only when the attribute is present, precompute long-term keys at startup, and avoid per-request key derivation.

**Constraints**: UDP only, single public address/port, no RFC 5780 NAT behavior discovery, no RFC 3489 backward compatibility, no TCP/TLS in this feature. Credentials live in a plaintext TOML file (documented file-permission requirement). MESSAGE-INTEGRITY provides integrity, not confidentiality.

**Scale/Scope**: Thousands of concurrent clients; stateless per-request handling; stateless nonce validation so no per-client server state is required.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Gate | Status |
|-----------|------|--------|
| I. RFC 5389 Compliance | Exact codepoints, §7.3 processing order, §10 short- and long-term flows, §15.4/§15.5 integrity rules; conformance matrix maintained | PASS — enforced by FR-001–FR-045 and the matrix deliverable |
| II. Test-Driven Development | Every behavior change lands with a failing test first; no implementation without a test | PASS — task breakdown will order test tasks before implementation tasks |
| III. Performance-First Design | Async I/O unchanged; hashes only when attributes are present; precomputed keys; benchmark gate | PASS — SC-008 and the benchmark task cover this |
| IV. Code Quality Gates | `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all` | PASS — mandatory verification phase |
| V. Simplicity and YAGNI | One credential map, one auth mode, stateless HMAC nonce, no credential database or plugin abstraction | PASS — design keeps new modules small |
| VI. CI Parity | CI-equivalent commands plus `cargo build --release`; Docker check when toolchain/lockfile changes | PASS — verification phase mirrors CI |

No constitutional violations; Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/008-rfc-compliance-audit/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   ├── stun-wire.md     # Wire behavior for requests/responses and auth flows
│   ├── server-config.md # [auth] TOML schema and validation
│   └── client-cli.md    # stunclient credential flags, retry, exit codes
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/stun-protocol/
├── Cargo.toml           # add hmac, sha1, md-5, subtle, crc32fast, stringprep
├── src/
│   ├── attributes.rs    # add USERNAME, MESSAGE-INTEGRITY, REALM, NONCE,
│   │                    # ALTERNATE-SERVER, FINGERPRINT
│   ├── integrity.rs     # NEW: HMAC-SHA1, CRC-32 FINGERPRINT, key derivation,
│   │                    # SASLprep, constant-time comparison
│   ├── message.rs       # MI/FINGERPRINT location and integrity-aware encoding
│   ├── summary.rs       # new names; MESSAGE-INTEGRITY shown as length only
│   ├── error_code.rs    # unchanged codecs, reused for 400/401/438
│   └── lib.rs           # export integrity helpers
└── tests/
    ├── rfc5769_vectors.rs  # add HMAC-SHA1 and CRC-32 assertions
    ├── authentication.rs   # NEW: short-term and long-term protocol tests
    └── protocol.rs

src/server/
├── Cargo.toml           # add rand for nonce secret
├── config.rs            # add [auth] schema and validation
├── auth.rs              # NEW: mode selection, credential lookup, nonce issue/verify
├── server.rs            # intake pipeline, SOFTWARE, response signing
├── main.rs              # pass auth configuration into the server
└── tests/
    ├── integration.rs   # auth success/failure and attribute scenarios
    └── logging_levels.rs

src/client/
├── Cargo.toml           # add rand, hmac/sha1 via stun-protocol
├── main.rs              # credential flags and exit codes
├── client.rs            # request signing, response MI validation, challenge retry
└── tests/client.rs      # auth flows with a local test server

src/bench/
├── main.rs              # add --fingerprint to measure the FINGERPRINT validation path
└── bench.rs             # build requests with or without FINGERPRINT

config/stunserver.conf   # documented, commented [auth] example (disabled by default)
docs/rfc-conformance.md  # NEW: conformance matrix (FR-045)
```

**Structure Decision**: Keep the existing workspace layout. Wire-format and cryptographic helpers belong in `stun-protocol`; server policy (mode, credentials, nonce lifecycle) belongs in `src/server/auth.rs`; client policy stays in `src/client/client.rs`.

## Complexity Tracking

> No constitutional violations. No entries required.

## Phase 0: Outline & Research

All Technical Context items are resolved. See [research.md](./research.md) for the decisions on crypto crates, SASLprep, stateless nonces, credential storage, integrity-aware encoding, and the auth decision order.

## Phase 1: Design & Contracts

- [data-model.md](./data-model.md) — configuration, credential, nonce, auth-outcome, and conformance-row entities.
- [contracts/stun-wire.md](./contracts/stun-wire.md) — request intake, auth outcomes, response attribute sets, integrity algorithms.
- [contracts/server-config.md](./contracts/server-config.md) — `[auth]` TOML schema and validation.
- [contracts/client-cli.md](./contracts/client-cli.md) — client credential flags, challenge retry, exit codes.
- [quickstart.md](./quickstart.md) — runnable validation scenarios.

## Post-Design Constitution Re-check

| Principle | Result |
|-----------|--------|
| I. RFC 5389 Compliance | PASS — contracts encode §7.3, §10.1, §10.2, §15.4, §15.5 exactly |
| II. Test-Driven Development | PASS — every contract rule has a named test target |
| III. Performance-First Design | PASS — precomputed keys, conditional hashing, benchmark gate |
| IV. Code Quality Gates | PASS — unchanged toolchain, all targets linted |
| V. Simplicity and YAGNI | PASS — one auth module, one credential map, no new abstractions |
| VI. CI Parity | PASS — release build and CI-equivalent commands in the verification phase |

## Verification Commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```

Run the Docker build check only if `docker/Dockerfile`, the toolchain, or `Cargo.lock` changes during implementation.
