# Implementation Plan: STUN RFC Error Handling & Attribute Interoperability

**Branch**: `007-stun-rfc-error-handling` | **Date**: 2026-10-31 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/007-stun-rfc-error-handling/spec.md`

## Summary

Bring the STUN implementation in line with RFC 5389, RFC 5780, and RFC 5769 for the P0 scope: (1) return 420 Unknown Attribute error responses with ERROR-CODE and UNKNOWN-ATTRIBUTES, (2) correct the RFC 5780 attribute registry codepoints, (5) validate wire format against the exact RFC 5769 vectors, and (4) make the bundled client interpret RFC-defined error/success responses instead of treating non-success messages as opaque failures.

The work stays inside the existing workspace. `stun-protocol` gains error-code/unknown-attributes codecs, comprehension-range helpers, padding-preserving attributes, strict XOR address lengths, and regenerated vector tests. `stunserver` classifies request attributes and emits error responses from the existing `build_response` path. `stunclient` gains structured server-error reporting and RFC-driven response tests. No new dependencies.

## Technical Context

**Language/Version**: Rust stable (CI toolchain 1.96, edition 2021)

**Primary Dependencies**: tokio 1.35 (UDP async I/O), byteorder 1.5, thiserror 1.0, log/env_logger; no new crates

**Storage**: N/A (stateless UDP protocol; config/log files unrelated to this feature)

**Testing**: `cargo test --all` with unit tests in `stun-protocol`, `stunserver` (`src/server/tests/integration.rs`), and `stunclient` (`src/client/tests/client.rs`); exact RFC 5769 byte-vector tests in `crates/stun-protocol/tests/rfc5769_vectors.rs`

**Target Platform**: Linux server, Tokio multi-thread runtime

**Project Type**: Rust workspace — protocol library + server/client binaries

**Performance Goals**: No regression on the success path; per-request attribute scan is O(attribute count) with early exit; error construction allocates only on the rejection path

**Constraints**: RFC 5389 §7.3.1, §15.6, §15.9; RFC 5780 §6, §6.1, §7.2–7.4; RFC 5769 §2.1–2.4; single public address/port (no alternate address); 64 KB datagram buffer

**Scale/Scope**: Server must continue scaling to thousands of concurrent clients (constitution III); changes touch 2 crates plus protocol library and test suites

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. RFC 5389 Compliance**: PASS. The feature implements mandatory §7.3.1 behavior and fixes codepoints that currently contradict RFC 5780. Two documented deviations are recorded in [research.md](research.md): treating MESSAGE-INTEGRITY as an unsupported comprehension-required attribute (420 instead of silent discard, because no credential support exists) and returning 420 instead of 400 for PADDING + RESPONSE-PORT.
- **II. TDD (NON-NEGOTIABLE)**: PASS by plan. Every behavior change is paired with a failing test first (see task ordering in tasks.md).
- **III. Performance-First**: PASS. Rejection scanning is a single pass over a small attribute vector and returns early; success path adds one pass without allocation. No new locks, no blocking I/O.
- **IV. Code Quality Gates**: PASS by plan. Final gate: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all`.
- **V. Simplicity/YAGNI**: PASS. No credential/crypto support, no PADDING/RESPONSE-PORT implementation, no alternate-address plumbing; only the codecs needed for 420 and diagnostics.
- **VI. CI Parity**: PASS by plan. Verification runs the exact CI commands locally before completion.

No violations; Complexity Tracking remains empty.

## Project Structure

### Documentation (this feature)

```text
specs/007-stun-rfc-error-handling/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   ├── protocol-wire.md     # Attribute registry, error response, XOR rules
│   └── client-response.md   # Client parsing outcome contract
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/stun-protocol/
├── src/
│   ├── attributes.rs        # registry codepoints, padding preservation, range helpers
│   ├── error_code.rs        # NEW: ErrorCode + UnknownAttributes encode/decode
│   ├── address.rs           # strict XOR-MAPPED-ADDRESS lengths
│   ├── message.rs           # post-decode attribute alignment check
│   ├── summary.rs           # ERROR-CODE / UNKNOWN-ATTRIBUTES / RFC 5780 names
│   └── lib.rs               # re-exports
└── tests/
    └── rfc5769_vectors.rs   # exact §2.1–2.4 vectors + existing structural tests

src/server/
├── server.rs                # unsupported-attribute scan + error response builder
└── tests/
    └── integration.rs       # real-UDP 420 scenarios

src/client/
├── client.rs                # parse error responses, unknown-attr checks
└── tests/
    └── client.rs            # RFC-driven success/error response scenarios
```

**Structure Decision**: Reuse the existing three-crate workspace layout. Protocol codecs belong in `stun-protocol` so both server and client share one implementation; server policy (which attributes are supported) stays in `stunserver`; client reporting stays in `stunclient`.

## Phase 0: Research & Decisions

See [research.md](research.md). All unknowns are resolved; key decisions:

1. Preserve attribute padding bytes so RFC 5769 vectors re-encode byte-for-byte.
2. Classify all comprehension-required request attributes as unsupported (supported set is empty today), yielding 420 — including CHANGE-REQUEST, PADDING, RESPONSE-PORT, and MESSAGE-INTEGRITY.
3. Add `ErrorCode` and `UnknownAttributes` codecs in a new `error_code.rs` module.
4. Add `ClientError::ServerError` carrying class/number/reason/unknown types.
5. Enforce exact XOR-MAPPED-ADDRESS value lengths (8 for IPv4, 20 for IPv6).

## Phase 1: Design & Contracts

- [data-model.md](data-model.md): attribute registry table, `ErrorCode`, `UnknownAttributes`, unsupported-attribute scan result, client outcome.
- [contracts/protocol-wire.md](contracts/protocol-wire.md): wire-level contract for the registry, 420 response layout, comprehension rules, XOR lengths.
- [contracts/client-response.md](contracts/client-response.md): client outcome contract and error surfaces.
- [quickstart.md](quickstart.md): runnable validation scenarios and exact CI commands.

### Implementation outline (ordered for TDD)

1. **Protocol registry**: update `StunAttributeType` (add `ChangeRequest=0x0003`, `Padding=0x0026`, `ResponsePort=0x0027`, `CacheTimeout=0x8027`, `ResponseOrigin=0x802B`, `OtherAddress=0x802C`; drop `SourceAddress`, `ChangedAddress`; remove `0x802D` mapping) and add `is_comprehension_required(raw: u16) -> bool`. Update tests that assert old names.
2. **Error codecs**: new `error_code.rs` with `ErrorCode::encode_value`/`decode_value` (class/number/reason, 4-byte header, UTF-8 reason padded) and `UnknownAttributes::encode_value`/`decode_value` (BE u16 list, de-duplicated on encode, padded). Export via `lib.rs`.
3. **Padding preservation**: add `padding: Vec<u8>` to `StunAttribute`; `decode` captures the exact pad bytes and rejects a truncated pad; `encode` replays them (zeros for newly built attributes). Add message-level `offset == end` check.
4. **XOR strictness**: `MappedAddress::decode_xor` requires value length exactly 8 (IPv4) or 20 (IPv6); mismatched family/length is `MalformedMessage`.
5. **Server 420 path**: scan decoded request attributes; collect `raw_type < 0x8000` in first-seen order into a de-duplicated `Vec<u16>`; introduce a shared `response_for_request` helper that returns either the existing success message or an `ErrorIndication` with ERROR-CODE 420 "Unknown Attribute" and UNKNOWN-ATTRIBUTES. Both `build_response` (library/tests) and the `run_server` receive loop MUST call this helper; otherwise the live path would bypass rejection. Keep the public `build_response` signature unchanged.
6. **Summaries**: render `ERROR-CODE=<class><number> "<reason>"` and `UNKNOWN-ATTRIBUTES=[0x....]`; add RFC 5780 names; keep existing unknown rendering.
7. **Client**: add `ClientError::ServerError { class, number, reason, unknown_attributes }`; branch on `StunMessageType::ErrorIndication`; ignore unknown optional attributes; reject unknown required attributes in success responses. Keep txid/source checks.
8. **RFC 5769 vectors**: replace "inspired-by" vectors with the exact §2.1, §2.2, §2.3, §2.4 byte arrays; assert type/txid/attribute order and lengths, decoded addresses, preserved MI/FINGERPRINT bytes, and byte-identical re-encode; retain the existing structural tests (header, padding, cookie, round-trip).
9. **End-to-end tests**: extend `src/server/tests/integration.rs` with real UDP loopback cases for 420 (single, duplicate, optional-only success) and `src/client/tests/client.rs` with mock-server scenarios for success v4/v6, 420, unknown optional/required, txid mismatch, malformed.

### Verification gate

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

## Complexity Tracking

No constitution violations; no complexity exceptions required.
