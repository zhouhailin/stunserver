# Tasks: STUN RFC Error Handling & Attribute Interoperability

**Input**: Design documents from `/specs/007-stun-rfc-error-handling/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: REQUIRED. The project constitution (Principle II) mandates TDD; every behavior change below starts with a failing test that must be observed red before implementation.

**Organization**: Tasks are grouped by user story so each can be implemented and tested independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1–US4 map to spec.md priorities
- Every task names the exact file path

## Path Conventions

Workspace layout: `crates/stun-protocol/` (library), `src/server/` (server crate, tests under `src/server/tests/`), `src/client/` (client crate, tests under `src/client/tests/`).

---

## Phase 1: Setup

**Purpose**: Confirm a clean baseline before touching protocol code.

- [X] T001 Establish a green baseline by running `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all` across the workspace (`Cargo.toml` members); record any pre-existing failures before editing `crates/stun-protocol/`, `src/server/`, or `src/client/`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared codecs and comprehension-range helper used by both the server (US1) and client (US4).

**⚠️ CRITICAL**: T002–T005 must complete before US1 or US4 implementation.

- [X] T002 [P] Write failing codec tests in `crates/stun-protocol/tests/error_code.rs` for ERROR-CODE class/number/reason round trip, value shorter than 4 bytes rejected, UNKNOWN-ATTRIBUTES `[0x0003]` → `00 03 00 00`, `[0x0024, 0x0025]` → `00 24 00 25`, and odd-length UNKNOWN-ATTRIBUTES rejected
- [X] T003 Create `crates/stun-protocol/src/error_code.rs` implementing `ErrorCode { class: u8 /*0–7*/, number: u8, reason: String }` with value layout `00 00 CC NN <reason UTF-8>` and `UnknownAttributes { types: Vec<u16> }` (big-endian, unique first-seen order, even-length decode); export both from `crates/stun-protocol/src/lib.rs` (depends on T002)
- [X] T004 [P] Write failing tests in the `#[cfg(test)]` module of `crates/stun-protocol/src/attributes.rs` asserting `is_comprehension_required` returns true for `0x0000–0x7FFF` and false for `0x8000–0xFFFF` (boundary values `0x7FFF`/`0x8000`)
- [X] T005 Implement `pub fn is_comprehension_required(raw: u16) -> bool { raw < 0x8000 }` in `crates/stun-protocol/src/attributes.rs` and export it from `crates/stun-protocol/src/lib.rs` (depends on T004)

**Checkpoint**: Codecs and range helper exist; US1 and US4 can proceed. US2/US3 remain independent.

---

## Phase 3: User Story 1 - Server returns 420 for unknown comprehension-required attributes (Priority: P1) 🎯 MVP

**Goal**: A Binding Request carrying any comprehension-required attribute the server does not implement gets a `0x0111` error response with ERROR-CODE 420 and a de-duplicated UNKNOWN-ATTRIBUTES list; unknown optional attributes are ignored.

**Independent Test**: Send a UDP Binding Request with PRIORITY `0x0024`; the reply is `0x0111` with ERROR-CODE class 4/number 20 and UNKNOWN-ATTRIBUTES `[0x0024]`, while a request with only `0x80FF` still succeeds.

### Tests for User Story 1 ⚠️

> Write these FIRST and confirm they fail for the expected reason (missing rejection behavior).

- [X] T006 [P] [US1] Write failing unit tests in `src/server/server.rs` calling `build_response` and asserting: PRIORITY `0x0024` → `0x0111`, ERROR-CODE class 4 number 20 reason `"Unknown Attribute"`, UNKNOWN-ATTRIBUTES `[0x0024]`, no XOR-MAPPED-ADDRESS; duplicate `0x0024` listed once; `0x80FF`-only request → `0x0101` success; transaction ID and magic cookie echoed
- [X] T007 [P] [US1] Write failing real-UDP integration tests in `src/server/tests/integration.rs` (loopback socket + `server::build_response`) for the single-unknown, duplicate-unknown, and optional-only scenarios from `contracts/protocol-wire.md` C2.1

### Implementation for User Story 1

- [X] T008 [US1] Implement the rejection path in `src/server/server.rs`: collect `attr.raw_type < 0x8000` into a first-seen de-duplicated `Vec<u16>`; add a shared `response_for_request` helper returning success (unchanged) or `ErrorIndication` with ERROR-CODE followed by UNKNOWN-ATTRIBUTES; call it from both `build_response` and the `run_server` receive loop (depends on T003, T005, T006, T007)
- [X] T009 [US1] Write failing tests in the `#[cfg(test)]` module of `crates/stun-protocol/src/summary.rs`, then render `ERROR-CODE=<class><number> "<reason>"` and `UNKNOWN-ATTRIBUTES=[0x....]` (uppercase four-digit hex) in `describe_attribute`
- [X] T010 [US1] Run `cargo test -p stunserver -p stun-protocol` covering `src/server/server.rs` and `crates/stun-protocol/src/summary.rs`; verify the DEBUG summary matches `contracts/protocol-wire.md` C9, e.g. `Binding Error Response txn=... attrs=[ERROR-CODE=420 "Unknown Attribute", UNKNOWN-ATTRIBUTES=[0x0024]]`

**Checkpoint**: US1 is fully functional over real UDP and independently testable.

---

## Phase 4: User Story 2 - Attribute registry uses correct RFC 5780 codepoints (Priority: P1)

**Goal**: The registry reports RFC 5780 names for CHANGE-REQUEST, PADDING, RESPONSE-PORT, CACHE-TIMEOUT, RESPONSE-ORIGIN, and OTHER-ADDRESS; `0x802D` is no longer OTHER-ADDRESS; CHANGE-REQUEST/PADDING/RESPONSE-PORT are rejected with 420; no response contains OTHER-ADDRESS.

**Independent Test**: Decode each code and assert its name, then send CHANGE-REQUEST/PADDING/RESPONSE-PORT and confirm 420 with the exact type listed.

### Tests for User Story 2 ⚠️

- [X] T011 [P] [US2] Write failing registry tests in the `#[cfg(test)]` module of `crates/stun-protocol/src/attributes.rs`: `0x0003`→CHANGE-REQUEST, `0x0026`→PADDING, `0x0027`→RESPONSE-PORT, `0x8027`→CACHE-TIMEOUT, `0x802B`→RESPONSE-ORIGIN, `0x802C`→OTHER-ADDRESS, and `0x802D` must not resolve to OTHER-ADDRESS
- [X] T012 [US2] Update `StunAttributeType` and `from_raw` in `crates/stun-protocol/src/attributes.rs`: add `ChangeRequest = 0x0003`, `Padding = 0x0026`, `ResponsePort = 0x0027`, `CacheTimeout = 0x8027`, `ResponseOrigin = 0x802B`, `OtherAddress = 0x802C`; remove `SourceAddress` and `ChangedAddress`; remove the `0x802D` mapping (depends on T011)
- [X] T013 [P] [US2] Write failing name tests in the `#[cfg(test)]` module of `crates/stun-protocol/src/summary.rs` for the six new attribute names and for decoding RESPONSE-ORIGIN/OTHER-ADDRESS via `MappedAddress::decode`
- [X] T014 [US2] Update `attribute_name` and the address-decoding match arms in `crates/stun-protocol/src/summary.rs`: RESPONSE-ORIGIN/OTHER-ADDRESS use `MappedAddress::decode`; CHANGE-REQUEST/PADDING/RESPONSE-PORT/CACHE-TIMEOUT render `NAME(len=N)` (depends on T012, T013)
- [X] T015 [US2] Add regression tests in `src/server/tests/integration.rs` for CHANGE-REQUEST `0x0003`, PADDING `0x0026`, and RESPONSE-PORT `0x0027` → 420 with the exact type in UNKNOWN-ATTRIBUTES, and assert no success or error response contains OTHER-ADDRESS (expected to pass once T008 lands; guards regressions)
- [X] T016 [US2] Run `cargo test -p stunserver -p stun-protocol` against `src/server/tests/integration.rs` and `crates/stun-protocol/src/attributes.rs`; confirm US2 acceptance scenarios 3–5

**Checkpoint**: Registry names are RFC-correct and unsupported RFC 5780 attributes are explicitly rejected.

---

## Phase 5: User Story 3 - Wire format validated against real RFC 5769 vectors (Priority: P2)

**Goal**: The exact §2.1–§2.4 byte sequences decode to documented values and re-encode byte-for-byte.

**Independent Test**: `cargo test -p stun-protocol --test rfc5769_vectors` passes with all four published vectors and byte-identical re-encoding.

### Tests for User Story 3 ⚠️

- [X] T017 [P] [US3] Write failing padding tests in `crates/stun-protocol/tests/rfc5769_vectors.rs`: a USERNAME value of 9 bytes decoded from padding `20 20 20` re-encodes with `20 20 20`; newly built 9-byte values still emit zero padding; a message whose last attribute pad is truncated is rejected
- [X] T018 [US3] Add `padding: Vec<u8>` to `StunAttribute` in `crates/stun-protocol/src/attributes.rs`: decode captures exactly `(4 - attribute_length % 4) % 4` bytes and errors when they are missing; encode replays stored padding or zeros; add a post-loop `offset == end` check in `crates/stun-protocol/src/message.rs` (depends on T017)
- [X] T019 [US3] Replace the "inspired-by" tests in `crates/stun-protocol/tests/rfc5769_vectors.rs` with the exact vectors from `contracts/protocol-wire.md` C7: §2.1 (108 bytes), §2.2 (80 bytes), §2.3 (92 bytes), §2.4 (116 bytes); assert message type, transaction ID, attribute order/value lengths, decoded mapped addresses (`192.0.2.1:32853`, `[2001:db8:1234:5678:11:2233:4455:6677]:32853`), opaque MESSAGE-INTEGRITY/FINGERPRINT, and byte-identical re-encoding; retain the existing structural tests (header, cookie, padding formula, round trip) (depends on T018)
- [X] T020 [US3] Run `cargo test -p stun-protocol --test rfc5769_vectors` against `crates/stun-protocol/tests/rfc5769_vectors.rs` and confirm all vectors and retained tests pass

**Checkpoint**: Interoperability is proven against published bytes, not self-round-trips.

---

## Phase 6: User Story 4 - Client consumes RFC-defined responses (Priority: P2)

**Goal**: The client reports structured server errors (420 with UNKNOWN-ATTRIBUTES), ignores unknown optional attributes, rejects unknown required attributes, and enforces address-family lengths.

**Independent Test**: `cargo test -p stunclient` plus `cargo test -p stun-protocol` pass with scenarios CR-01–CR-10 from `contracts/client-response.md`.

### Tests for User Story 4 ⚠️

- [X] T021 [P] [US4] Write failing tests in `crates/stun-protocol/tests/protocol.rs` for `MappedAddress::decode_xor`: IPv4 value of exactly 8 bytes succeeds, IPv6 exactly 20 succeeds, IPv4 family with 20 bytes fails, IPv6 family with 8 bytes fails, and wrong family byte fails
- [X] T022 [US4] Enforce exact value lengths in `MappedAddress::decode_xor` in `crates/stun-protocol/src/address.rs` (8 bytes IPv4, 20 bytes IPv6; mismatch → `MalformedMessage`), leaving non-XOR `decode` unchanged (depends on T021)
- [X] T023 [P] [US4] Write failing unit tests in the `#[cfg(test)]` module of `src/client/client.rs` for `parse_response`: `0x0111` with ERROR-CODE 420 + UNKNOWN-ATTRIBUTES → `ClientError::ServerError { class: 4, number: 20, unknown_attributes: [0x0024] }`; 420 without the list → empty list; error without ERROR-CODE → `Protocol`; success with unknown optional `0x80FF` → `Ok`; success with unknown required `0x0024` → `Protocol`; txid mismatch and malformed data → `Protocol`
- [X] T024 [US4] Add `ClientError::ServerError { class: u8, number: u8, reason: String, unknown_attributes: Vec<u16> }` and implement error-response parsing, unknown-attribute checks, and empty-list handling in `src/client/client.rs` (depends on T003, T005, T023)
- [X] T025 [US4] Extend the mock-server harness in `src/client/tests/client.rs` with scenarios CR-01–CR-10 (IPv4/IPv6 success, 420 with/without list, unknown optional/required, txid mismatch, malformed, family/length mismatch, timeout) (depends on T024)
- [X] T026 [US4] Run `cargo test -p stunclient -p stun-protocol` covering `src/client/tests/client.rs` and `crates/stun-protocol/tests/protocol.rs`; confirm every CR scenario passes and no previous client test regressed

**Checkpoint**: The bundled client can diagnose 420 responses and distrust malformed successes.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final validation and documentation consistency.

- [X] T027 [P] Execute `specs/007-stun-rfc-error-handling/quickstart.md` sections 1–4 (unit/vector tests, server UDP tests, client tests, manual probe) and confirm each expected outcome
- [X] T028 [P] Confirm both documented deviations (MESSAGE-INTEGRITY → 420, PADDING + RESPONSE-PORT → 420) remain recorded in `specs/007-stun-rfc-error-handling/research.md` and `spec.md` Assumptions; adjust wording only if implementation diverged
- [X] T029 Run the full CI-parity gate for the workspace (`Cargo.toml` members) and fix any findings: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all`
- [X] T030 Re-run `cargo test --all` after formatting fixes to confirm no test changed behavior, then update the completed task checkboxes in `specs/007-stun-rfc-error-handling/tasks.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies
- **Foundational (Phase 2)**: Depends on Setup; blocks US1 and US4. US2 and US3 do not depend on it.
- **US1 (Phase 3)**: Depends on T003 (codecs) and T005 (range helper)
- **US2 (Phase 4)**: Depends only on Setup; T014 depends on T012/T013
- **US3 (Phase 5)**: Depends only on Setup; internal chain T017→T018→T019→T020
- **US4 (Phase 6)**: Depends on T003/T005 (codecs/helper) and T022 (strict lengths) for the full matrix; T023/T024 can start after Foundational
- **Polish (Phase 7)**: Depends on all desired stories

### User Story Dependencies

- **US1 (P1)**: Independent MVP after Foundational
- **US2 (P1)**: Independent of US1/US4; shares only `attributes.rs`/`summary.rs` edits (execute sequentially to avoid file conflicts)
- **US3 (P2)**: Independent; owns padding changes in `attributes.rs`/`message.rs` (execute before or after US2 edits to the same files)
- **US4 (P2)**: Independent after Foundational + T022

### File-conflict notes

- `crates/stun-protocol/src/attributes.rs`: touched by T005, T012, T018 — run these sequentially
- `crates/stun-protocol/src/summary.rs`: T009 then T014 sequentially
- `src/server/tests/integration.rs`: T007 then T015 sequentially
- `src/client/client.rs`: T023 then T024 sequentially

### Parallel Opportunities

- T002 and T004 (different files) can run in parallel; their implementations T003/T005 follow their own tests
- Once Foundational completes, US1 chain (T006/T007 → T008) and US4 chain (T021 → T022, T023 → T024 → T025) can run in parallel
- T009 (summary error rendering) can run alongside T008 (server path) after codecs exist, but must precede T014
- T011/T013 (different test modules) can run in parallel; T014 must wait for T012 and T013
- T027 and T028 are independent final checks

## Parallel Example: US1 + US4 after Foundational

```text
Chain A (US1): T006 → T007 → T008 → T010 → T009
Chain B (US4): T021 → T022 → T023 → T024 → T025 → T026
```

## Implementation Strategy

### MVP First

1. Phase 1 baseline
2. Phase 2 codecs + range helper
3. Phase 3 (US1) → validate with real UDP 420 tests
4. Stop and demo: server now rejects unknown required attributes

### Incremental Delivery

1. US1 (MVP) — 420 behavior
2. US2 — correct RFC 5780 names and explicit rejects
3. US3 — real vector interoperability proof
4. US4 — client diagnostics
5. Polish — quickstart + CI gate

## Notes

- Write tests first and confirm the expected red state before each implementation task
- Do not modify checklist markers in `specs/007-stun-rfc-error-handling/checklists/`
- Keep `build_response`'s public signature unchanged so existing tests and integrations compile
- Verify every comprehension-required request attribute is rejected, including CHANGE-REQUEST, PADDING, RESPONSE-PORT, PRIORITY, USERNAME, and MESSAGE-INTEGRITY

---

## Phase 8: Convergence

- [X] T031 Enforce the RFC 5389 §15.4/§15.5 fixed value lengths in `StunAttribute::decode` (`crates/stun-protocol/src/attributes.rs`): MESSAGE-INTEGRITY (`0x0008`) MUST be exactly 20 value bytes and FINGERPRINT (`0x8028`) exactly 4, otherwise `MalformedMessage`; begin with failing tests in `crates/stun-protocol/tests/protocol.rs` (valid lengths accepted, every other length rejected via `StunAttribute::decode` and `StunMessage::decode`) and confirm `crates/stun-protocol/tests/rfc5769_vectors.rs` still decodes and re-encodes all four vectors byte-for-byte per FR-012 (partial)

