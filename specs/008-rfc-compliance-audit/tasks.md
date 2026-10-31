# Tasks: STUN RFC 5389/5769/5780 Conformance Audit and Remediation

**Input**: Design documents from `/specs/008-rfc-compliance-audit/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Required. The project constitution makes TDD non-negotiable, so every behavior change below has a test task that MUST be written and observed failing before its implementation task.

**Organization**: Tasks are grouped by user story so each story can be implemented, tested, and delivered independently. Story phases follow the priority order in spec.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel with other [P] tasks in the same phase (different files, no dependency on an incomplete task)
- **[Story]**: Which user story the task belongs to (US1-US7); setup, foundational, and polish tasks have no story label
- Every task names the exact file path it changes

## Path Conventions

- Protocol library: `crates/stun-protocol/`
- Server binary: `src/server/`
- Client binary: `src/client/`
- Benchmark binary: `src/bench/`
- Documentation: `docs/`, `README.md`, `README.zh-CN.md`
- Configuration: `config/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add the dependency surface required by every story.

- [X] T001 Add `hmac 0.12`, `sha1 0.10`, `md-5 0.10`, `subtle 2.5`, `crc32fast 1.4`, and `stringprep 0.1` to the workspace dependencies in `Cargo.toml` and to `crates/stun-protocol/Cargo.toml`.
- [X] T002 [P] Add `rand 0.8` to `src/server/Cargo.toml` and `src/client/Cargo.toml` for `rand::rngs::OsRng`, run `cargo check --all --all-targets`, and (before any behavior change) start the unmodified server locally and record the SC-008 p99 baseline with `cargo run -p stunbench --release -- 127.0.0.1:3478 --qps 1000 --duration 10` for T054.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Attribute registry, summary naming, and integrity primitives that every story depends on. No user story work begins until this phase is complete.

**CRITICAL**: Complete all tests first, observe them fail, then implement.

### Tests (write first, observe failure)

- [X] T003 [P] Add failing registry tests in `crates/stun-protocol/src/attributes.rs` asserting `from_raw` recognizes USERNAME `0x0006`, MESSAGE-INTEGRITY `0x0008`, REALM `0x0014`, NONCE `0x0015`, SOFTWARE `0x8022`, ALTERNATE-SERVER `0x8023`, FINGERPRINT `0x8028`, plus the six existing RFC 5780 types, and that comprehension-required is exactly `0x0000`-`0x7FFF`.
- [X] T004 [P] Add failing summary tests in `crates/stun-protocol/src/summary.rs` asserting each recognized type renders its RFC name and unrecognized types render as a four-digit `0xNNNN` code.
- [X] T005 [P] Add failing integrity tests in `crates/stun-protocol/tests/integrity.rs` covering HMAC-SHA1 (20-byte output), CRC-32 XOR `0x5354554E`, `MD5(username ":" realm ":" SASLprep(password))`, constant-time comparison, and the full SASLprep RFC 4013 behavior: mapping, normalization, prohibited output, and bidirectional-text rejection. Include a non-ASCII password case and confirm the `stringprep` SASLprep API; if that profile is unavailable, switch to the dedicated `saslprep` crate rather than weakening the algorithm (SC-012).
- [X] T006 [P] Add failing encoding tests in `crates/stun-protocol/tests/protocol.rs` asserting attributes are located by summing `encoded_size()` from offset 20, that the HMAC range uses a header length covering the MESSAGE-INTEGRITY attribute with digest bytes omitted, that the FINGERPRINT range uses the final message length, and that a duplicate MESSAGE-INTEGRITY or FINGERPRINT is detectable.

### Implementation

- [X] T007 [P] Add the six new variants and `from_raw` mappings to `StunAttributeType` in `crates/stun-protocol/src/attributes.rs` (FR-001, FR-003).
- [X] T008 [P] Update attribute and message name rendering in `crates/stun-protocol/src/summary.rs` for all new types, with `0xNNNN` fallback (FR-002).
- [X] T009 [P] Implement `crates/stun-protocol/src/integrity.rs`: HMAC-SHA1 compute/verify, CRC-32 FINGERPRINT compute/verify, short-term key handling, long-term key derivation with full SASLprep, and constant-time digest comparison via `subtle` (FR-027, FR-032 support).
- [X] T010 Export the integrity module from `crates/stun-protocol/src/lib.rs`.
- [X] T011 Implement integrity-aware encoding helpers in `crates/stun-protocol/src/message.rs`: locate a digest attribute, build the HMAC byte range, build the CRC byte range, and append MESSAGE-INTEGRITY to a response (FR-010, FR-011, FR-028).

**Checkpoint**: Registry, summaries, and integrity helpers are green; user stories can start.

---

## Phase 3: User Story 1 - Traceable conformance audit matrix (Priority: P1) - MVP

**Goal**: Publish the clause-by-clause conformance matrix that defines audit scope, records gaps, and gives every later fix a traceable home.

**Independent Test**: Review `docs/rfc-conformance.md` and spot-check at least ten entries against the RFC text and named test evidence; every in-scope clause has a status and every gap names its closing FR.

- [X] T012 [US1] Create `docs/rfc-conformance.md` with the table schema `RFC | Section | Level | Requirement | Status | Evidence | Notes` and populate RFC 5389 §6, §7.1-§7.3, and §15 rows using MUST/SHOULD/MAY levels (FR-045).
- [X] T013 [US1] Add RFC 5769 §2 and RFC 5780 §6-§7 rows to `docs/rfc-conformance.md`, mark every row `conformant`, `gap`, or `out-of-scope`, and map each `gap` row to its closing FR-001-FR-045 identifier.
- [X] T014 [P] [US1] Link `docs/rfc-conformance.md` from `README.md` and `README.zh-CN.md`.
- [X] T015 [US1] Review `docs/rfc-conformance.md` for 100% classification: every row has a normative level, status, and evidence or an out-of-scope justification (RFC 3489, TCP/TLS, NAT behavior discovery), and record the review date in the file header.

**Checkpoint**: MVP deliverable complete - the audit is traceable before remediation starts.

---

## Phase 4: User Story 2 - Known versus unknown attributes are distinguished correctly (Priority: P1)

**Goal**: Stop rejecting standard RFC 5389 attributes with 420, ignore known-but-unexpected attributes as RFC 5389 §7.3 requires, and reject only genuinely unsupported comprehension-required types.

**Independent Test**: Send crafted requests with known-only, unknown-required, mixed, and unknown-optional attributes; assert success versus 420 and exact UNKNOWN-ATTRIBUTES contents.

### Tests for User Story 2 (write first, observe failure)

- [X] T016 [P] [US2] Add failing integration tests in `src/server/tests/integration.rs`: known-only attributes (MAPPED-ADDRESS, ERROR-CODE, UNKNOWN-ATTRIBUTES, ALTERNATE-SERVER) return success; PRIORITY `0x0024` alone returns 420 listing exactly `0x0024`; mixed requests list only unknown required types; unknown optional `0x8000+` is ignored; CHANGE-REQUEST, PADDING, and RESPONSE-PORT still return 420; duplicate unknown types are listed once in first-seen order; with authentication disabled, requests carrying USERNAME, MESSAGE-INTEGRITY, REALM, or NONCE return a Binding Success Response and no 420 (SC-003, SC-005).
- [X] T017 [P] [US2] Add failing DEBUG-summary tests in `src/server/tests/logging_levels.rs` asserting RFC 5389 and RFC 5780 attributes appear by RFC name and unrecognized types appear as `0xNNNN`.

### Implementation for User Story 2

- [X] T018 [US2] Replace `unsupported_required_attributes` in `src/server/server.rs` with classification that rejects only unrecognized comprehension-required types plus CHANGE-REQUEST/PADDING/RESPONSE-PORT, ignores unknown optional types, ignores known non-authentication attributes, de-duplicates in first-seen order, and ignores authentication attributes while the mode is disabled (FR-004-FR-007).

**Checkpoint**: US1 and US2 pass independently; standard attributes no longer trigger 420.

---

## Phase 5: User Story 3 - Authenticated Binding requests (Priority: P1)

**Goal**: Validate USERNAME and MESSAGE-INTEGRITY for short-term and long-term credentials, issue challenges, and sign authenticated responses.

**Independent Test**: Send requests with valid, unknown, missing, stale, and tampered credentials under both mechanisms; assert response type, error code, and presence or absence of MESSAGE-INTEGRITY, USERNAME, REALM, and NONCE.

### Tests for User Story 3 (write first, observe failure)

- [X] T019 [P] [US3] Add failing protocol authentication tests in `crates/stun-protocol/tests/authentication.rs`: short-term HMAC verification, long-term key derivation with ASCII and non-ASCII SASLprep passwords, tampered byte rejection, and exclusion of attributes after MESSAGE-INTEGRITY from the digest range (SC-012).
- [X] T020 [P] [US3] Add failing `[auth]` configuration tests in `src/server/config.rs`: an absent section defaults to disabled; unknown mode is rejected; enabled modes require at least one user; usernames are non-empty and unique; passwords are non-empty; long-term requires a non-empty realm; `nonce_lifetime_sec` must be 1-86400 in long-term mode; short-term keys are raw password bytes and long-term keys are 16-byte MD5 digests.
- [X] T021 [P] [US3] Add failing server authentication tests in `src/server/tests/integration.rs` covering all 11 US3 scenarios: short-term success signed with no USERNAME; short-term missing attributes 400 unsigned; short-term unknown user or bad MAC 401 unsigned; long-term missing MESSAGE-INTEGRITY 401 with REALM+NONCE; long-term MESSAGE-INTEGRITY present but USERNAME/REALM/NONCE missing 400; stale NONCE 438 with fresh NONCE+REALM; long-term unknown user, realm mismatch, or bad MAC 401 with REALM+NONCE; long-term success signed with no USERNAME/REALM/NONCE; authenticated 420 signed; enabled mode with no credential attributes never succeeds; failure responses omit MESSAGE-INTEGRITY and USERNAME. Add an end-to-end non-ASCII SASLprep case: configure a credential whose password requires normalization and authenticate over the wire with a client password whose normalized form matches it (SC-009, SC-012).
- [X] T022 [P] [US3] Add failing secret-leak tests in `src/server/tests/logging_levels.rs` asserting passwords, derived keys, and the nonce secret never appear in DEBUG/TRACE output and that MESSAGE-INTEGRITY is summarized by length only (FR-033, SC-010).

### Implementation for User Story 3

- [X] T023 [US3] Implement `AuthMode`, `AuthConfig`, and `UserCredential` parsing, validation, and startup key precomputation in `src/server/config.rs`, including full SASLprep before long-term MD5 derivation (FR-019, FR-027, FR-031, FR-032, SC-012).
- [X] T024 [US3] Create `src/server/auth.rs` with credential lookup, stateless NONCE issue/verify (8-byte big-endian seconds plus 16-byte truncated HMAC-SHA1 rendered as 48 lowercase hex characters), the fixed `AuthDecision` order, and challenge construction; ensure no `Debug`/`Display` output exposes secrets (FR-020-FR-026, FR-029, FR-032).
- [X] T025 [US3] Add `pub mod auth;` to `src/server/lib.rs`.
- [X] T026 [US3] Integrate the authentication stage into `src/server/server.rs` after header and FINGERPRINT intake and before classification: emit unsigned 400/401/438 responses, sign success and 420 responses with the request's key, and omit USERNAME/REALM/NONCE from success responses (FR-020, FR-028-FR-030).
- [X] T027 [US3] Pass the loaded `AuthConfig` from `src/server/main.rs` into `run_server` and the server state used by `src/server/server.rs`.
- [X] T028 [US3] Add a commented `[auth]` example (mode `disabled` by default) to `config/stunserver.conf` and the plaintext-file `chmod 600` warning to `config/README.md`.

**Checkpoint**: Both authentication mechanisms pass independently, including 400/401/438 flows and signed 420 responses.

---

## Phase 6: User Story 4 - Message intake enforces FINGERPRINT and MESSAGE-INTEGRITY rules (Priority: P1)

**Goal**: Validate FINGERPRINT position and CRC, enforce MESSAGE-INTEGRITY uniqueness, ignore attributes after MESSAGE-INTEGRITY, and silently discard defective messages.

**Independent Test**: Send valid, invalid, duplicated, and misordered FINGERPRINT/MESSAGE-INTEGRITY combinations and confirm response versus silent discard.

### Tests for User Story 4 (write first, observe failure)

- [X] T029 [P] [US4] Add failing integration tests in `src/server/tests/integration.rs`: valid terminal FINGERPRINT returns success; bad CRC causes no response; non-final or duplicate FINGERPRINT causes no response; a required attribute after MESSAGE-INTEGRITY is ignored and does not cause 420; MESSAGE-INTEGRITY followed by valid FINGERPRINT succeeds when authentication passes or is disabled; a valid FINGERPRINT combined with an unknown comprehension-required attribute still returns 420 after FINGERPRINT validation; defective headers (non-zero leading bits, wrong magic cookie, length not a multiple of four, length beyond the datagram), non-Binding-Request classes, unsupported methods, and truncated attributes produce no response (SC-004).
- [X] T030 [P] [US4] Add failing protocol tests in `crates/stun-protocol/tests/protocol.rs` for CRC computation over the final-length header and duplicate MESSAGE-INTEGRITY/FINGERPRINT detection; add regression tests for the already-enforced 20-byte MESSAGE-INTEGRITY and 4-byte FINGERPRINT lengths and note that those assertions are expected to pass immediately.

### Implementation for User Story 4

- [X] T031 [US4] Implement the intake pipeline in `src/server/server.rs`: decode/header validation, Binding Request requirement, FINGERPRINT final/unique/4-byte/CRC validation, MESSAGE-INTEGRITY 20-byte/unique enforcement, and exclusion of attributes after MESSAGE-INTEGRITY from auth and classification (FR-008-FR-013).
- [X] T032 [US4] Add DEBUG/TRACE discard logging in `src/server/server.rs` that records the reason and attribute length/type only - never payload bytes or secrets.
- [X] T033 [US4] Enforce the fixed pipeline order in `src/server/server.rs`: header -> Binding Request -> FINGERPRINT -> MESSAGE-INTEGRITY location -> authentication -> unknown-attribute classification -> response; add a precedence test in `src/server/tests/integration.rs` proving authentication failures win over 420 (FR-020, edge cases).

**Checkpoint**: Malformed traffic is silently discarded and authenticated processing has the correct precedence.

---

## Phase 7: User Story 5 - Responses are complete and do not overclaim RFC 5780 support (Priority: P2)

**Goal**: Add SOFTWARE to every response, keep 420 contents exact, and guarantee no OTHER-ADDRESS, RESPONSE-ORIGIN, or `stun-behavior` advertising.

**Independent Test**: Capture success, 420, and authentication-error responses, inspect attribute sets and SOFTWARE stability, and scan documentation/configuration for RFC 5780 service claims.

### Tests for User Story 5 (write first, observe failure)

- [X] T034 [P] [US5] Add failing response tests in `src/server/tests/integration.rs`: success, 420, 400, 401, and 438 each carry SOFTWARE; SOFTWARE is valid UTF-8, shorter than 128 characters, and identical across requests; no response contains OTHER-ADDRESS or RESPONSE-ORIGIN; responses arrive from the bound local address and port (FR-014-FR-018, SC-005).
- [X] T035 [P] [US5] Add a failing workspace test in `tests/rfc5780_advertising.rs` that scans `README.md`, `README.zh-CN.md`, `config/`, and deployment files for affirmative `stun-behavior`/`stun-behaviors` service advertising; scan `docs/rfc-conformance.md` only outside rows marked `out-of-scope`, and explicitly exclude `docs/rfcs/` (FR-017, SC-007).

### Implementation for User Story 5

- [X] T036 [US5] Add a stable `stunserver <CARGO_PKG_VERSION>` SOFTWARE constant in `src/server/server.rs` and emit SOFTWARE before MESSAGE-INTEGRITY in success, 420, 400, 401, and 438 responses (FR-016, FR-028-FR-030).
- [X] T037 [US5] Update `README.md` and `README.zh-CN.md` to state that the server offers no NAT behavior discovery and never emits OTHER-ADDRESS or RESPONSE-ORIGIN.
- [X] T038 [US5] Verify UDP responses are sent from the same local socket in `src/server/server.rs` and fix the send path if T034 fails (FR-018).

**Checkpoint**: Every response is self-identifying and RFC 5780 claims are absent.

---

## Phase 8: User Story 6 - RFC 5769 vectors prove integrity correctness (Priority: P2)

**Goal**: Prove MESSAGE-INTEGRITY and FINGERPRINT against the published RFC 5769 vectors, not just structural re-encoding.

**Independent Test**: `cargo test -p stun-protocol rfc5769 -- --nocapture` reproduces each documented HMAC-SHA1 and CRC-32 value and re-encodes byte-for-byte.

### Tests for User Story 6 (write first, observe failure)

- [X] T039 [P] [US6] Extend `crates/stun-protocol/tests/rfc5769_vectors.rs` to verify MESSAGE-INTEGRITY and FINGERPRINT for §2.1-§2.3, derive the §2.4 long-term key from the documented username/realm/password, assert tampering any covered byte fails, and retain the byte-identical re-encode assertion (FR-043, SC-001).

### Implementation for User Story 6

- [X] T040 [US6] Correct any digest-range, length-field, or key-derivation mismatch in `crates/stun-protocol/src/integrity.rs` and `crates/stun-protocol/src/message.rs` until all four vectors pass.
- [X] T041 [US6] Record the passing vector test names as evidence in the RFC 5769 rows of `docs/rfc-conformance.md`.

**Checkpoint**: Wire-format integrity is proven by published vectors.

---

## Phase 9: User Story 7 - Client follows the same protocol rules (Priority: P3)

**Goal**: Make the diagnostic client generate unpredictable IDs, identify itself, validate FINGERPRINT and response integrity, and support short-term and long-term authentication.

**Independent Test**: Exercise ID generation, request contents, response validation, and both authenticated flows with crafted valid and invalid responses.

### Tests for User Story 7 (write first, observe failure)

- [X] T042 [P] [US7] Add failing client tests in `src/client/tests/client.rs`: 10,000 generated IDs are 12 bytes and all distinct with no clock-derived pattern; requests contain SOFTWARE; valid terminal FINGERPRINT accepted while invalid, duplicate, or non-terminal FINGERPRINT is a protocol error; unknown required attributes rejected and unknown optional ignored; ERROR-CODE class outside 3-6 or number above 99 is malformed; transaction-ID and source-address mismatches rejected; short-term requests carry USERNAME+MESSAGE-INTEGRITY; long-term 401 challenge produces one signed retry and 438 adopts a fresh NONCE within a two-retry bound; response MESSAGE-INTEGRITY validates; unconfigured client reports 401/438 without retry; no secret appears in errors (FR-034-FR-042, SC-006, SC-011); add DEBUG/TRACE redaction assertions that capture client output across `src/client/main.rs` and `src/client/client.rs`, confirming `auth=<mode> user=<name>` is logged while the password, derived key, REALM nonce MAC, and server secret never appear (SC-010); and add CLI parsing/validation tests in `src/client/main.rs` for each `--auth-mode`, missing username/password, credentials combined with `disabled`, and `STUNCLIENT_PASSWORD` precedence, observed failing before T047.

### Implementation for User Story 7

- [X] T043 [US7] Replace the clock-derived generator in `src/client/client.rs` with `OsRng`-filled 96-bit transaction IDs (FR-034).
- [X] T044 [US7] Add a SOFTWARE attribute to every request in `src/client/client.rs` (FR-035).
- [X] T045 [US7] Add response FINGERPRINT validation (final attribute, exactly 4 bytes, CRC-correct) to `src/client/client.rs` (FR-036).
- [X] T046 [US7] Extend `client_recognizes` in `src/client/client.rs` for the RFC 5389 response attributes and validate ERROR-CODE class 3-6 and number 0-99 (FR-037, FR-038).
- [X] T047 [P] [US7] Add `--auth-mode`, `--username`, `--password`, and `STUNCLIENT_PASSWORD` handling with usage validation to `src/client/main.rs`; reject credentials with `disabled` mode (FR-039).
- [X] T048 [US7] Implement short-term request signing and the long-term state machine (initial unauthenticated request, 401/438 challenge consumption, at most two signed retries) in `src/client/client.rs` (FR-040, FR-041, SC-011).
- [X] T049 [US7] Validate response MESSAGE-INTEGRITY with the same key in `src/client/client.rs` and map missing/invalid digests to protocol exit code 4 (FR-042, SC-011, client-cli contract).
- [X] T050 [US7] Review client output in `src/client/main.rs` and `src/client/client.rs`: log `auth=<mode> user=<name>` only, never passwords or keys, and warn in `--debug` that `--password` is visible in process listings.

**Checkpoint**: The client can exercise all server paths and cannot be fooled by unauthenticated responses.

---

## Phase 10: Polish & Cross-Cutting Concerns

**Purpose**: Performance evidence, documentation consolidation, and full CI-parity verification.

- [X] T051 [P] Add a `--fingerprint` flag to `src/bench/main.rs` that makes `src/bench/bench.rs` append a valid terminal FINGERPRINT to every request, with parsing and request-shape tests in `src/bench/tests/bench.rs` (SC-008).
- [X] T052 [P] Update `docs/rfc-conformance.md` so every former `gap` row is `conformant` with the implementing test path as evidence; keep out-of-scope rows justified (FR-045, SC-002).
- [X] T053 [P] Document authentication setup in `README.md` and `README.zh-CN.md`: `[auth]` modes, `chmod 600`, long-term REALM/NONCE behavior, and `STUNCLIENT_PASSWORD` over `--password`.
- [X] T054 Compare the pre-change p99 baseline recorded in T002 against the post-change plain and `--fingerprint` runs of `stunbench --qps 1000 --duration 10` against a release server, and record all three p99 values plus the plain-vs-baseline and fingerprint-vs-plain deltas in `docs/rfc-conformance.md`; the plain run must stay within 5% of the baseline (SC-008).
- [X] T055 Execute the scenarios in `specs/008-rfc-compliance-audit/quickstart.md` 1-9 end to end (including wrong/missing credentials, unauthenticated mode, secret grep, and the matrix review) and record any deviations.
- [X] T056 Run the full quality gate across `Cargo.toml`, `crates/stun-protocol/`, `src/`, and `tests/`: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all`, and `cargo build --release`.
- [X] T057 Run the build check for `docker/Dockerfile` if the Dockerfile, the toolchain, or `Cargo.lock` changed during implementation.
- [X] T058 Perform the security review of `src/server/`, `src/client/`, `config/README.md`, and `docs/rfc-conformance.md`: grep logs, summaries, error text, and documentation for passwords/keys/nonce secrets, confirm the `chmod 600` guidance exists, and confirm `--password` documentation also names the environment-variable alternative (SC-010).
- [X] T059 Perform the final conformance-matrix review in `docs/rfc-conformance.md`, confirming 100% of in-scope MUST/SHOULD clauses are classified and no row is left unclassified (SC-002); also audit FR-001-FR-045 coverage end to end, verifying every functional requirement has at least one named test or conformance-row evidence entry, and record any missing mapping as a defect before closing the feature (FR-044).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup; BLOCKS all user stories.
- **US1 (Phase 3)**: Depends on Foundational; no dependency on other stories.
- **US2 (Phase 4)**: Depends on Foundational; touches `src/server/server.rs`.
- **US3 (Phase 5)**: Depends on Foundational; touches `src/server/server.rs` and must follow US2 if both are in progress, because both rewrite the same request path.
- **US4 (Phase 6)**: Depends on Foundational; also touches `src/server/server.rs`; run after US3 so authentication exists before the pipeline order is finalized.
- **US5 (Phase 7)**: Depends on Foundational and US3 for signed-response ordering; SOFTWARE must be appended before MESSAGE-INTEGRITY.
- **US6 (Phase 8)**: Depends on Foundational; independent of server/client stories.
- **US7 (Phase 9)**: Depends on Foundational and US4 semantics (FINGERPRINT validation); independent of the server stories at compile time.
- **Polish (Phase 10)**: Depends on all selected stories.

### Within Each User Story

- Test tasks MUST be written and observed failing before implementation.
- Configuration and protocol primitives precede server pipeline integration.
- Core implementation precedes cross-story integration (SOFTWARE before signing order changes, pipeline before precedence tests).
- A story is complete only when its tests pass and the checkpoint is validated.

### Parallel Opportunities

- T001 and T002 are independent dependency edits.
- T003-T006 (foundational tests) are independent files and can run together.
- T007-T009 are independent files; T010 and T011 depend on T009.
- T016 and T017 are independent test files.
- T019-T022 are independent test files (protocol, config, integration, logging).
- T029 and T030 are independent test files.
- T034 and T035 are independent test files.
- T043-T046 all edit `src/client/client.rs` and must be sequential; T047 edits `src/client/main.rs` and can run alongside them.
- T051-T053 are independent polish files.

---

## Parallel Example: Foundational Tests

```bash
# Write these failing tests together, then implement:
Task: "Add failing registry tests in crates/stun-protocol/src/attributes.rs"
Task: "Add failing summary tests in crates/stun-protocol/src/summary.rs"
Task: "Add failing integrity tests in crates/stun-protocol/tests/integrity.rs"
Task: "Add failing encoding tests in crates/stun-protocol/tests/protocol.rs"
```

## Parallel Example: User Story 3 Tests

```bash
Task: "Add failing protocol authentication tests in crates/stun-protocol/tests/authentication.rs"
Task: "Add failing [auth] configuration tests in src/server/config.rs"
Task: "Add failing server authentication tests in src/server/tests/integration.rs"
Task: "Add failing secret-leak tests in src/server/tests/logging_levels.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational (registry, summaries, integrity primitives).
3. Complete Phase 3: US1 conformance matrix.
4. **STOP and VALIDATE**: review the matrix, confirm 100% classification and gap-to-FR traceability.
5. Deliver the audit as the MVP; subsequent stories close the recorded gaps.

### Incremental Delivery

1. Setup + Foundational -> protocol groundwork ready.
2. US1 -> traceable audit delivered.
3. US2 -> standard attribute interoperability fixed.
4. US3 -> authentication functional.
5. US4 -> intake hardening and precedence correct.
6. US5 -> response completeness and RFC 5780 non-claims.
7. US6 -> integrity proven by published vectors.
8. US7 -> client trustworthy and able to exercise authentication.
9. Polish -> performance evidence, documentation, and CI parity.

### Parallel Team Strategy

After Foundational:

- Developer A: US1 (documentation) and US6 (vectors), both low-conflict.
- Developer B: US2 then US3 then US4 (all `src/server/server.rs`; must be sequential).
- Developer C: US7 (`src/client/`), then US5 README portions.
- US5 server.response changes must wait for US3 signing to land.

---

## Notes

- [P] means different files and no dependency on an incomplete task; tasks editing `src/server/server.rs` or `src/client/client.rs` are never parallel with each other.
- Every behavior-changing task has a preceding failing test task, per the constitution.
- Commit after each task or logical group using Conventional Commits.
- Stop at each checkpoint to validate the story independently.
- Do not weaken SASLprep to ASCII-only; switch to the dedicated `saslprep` crate if `stringprep` lacks the profile.
- Do not log passwords, derived keys, or the nonce secret at any level.

## Phase 11: Convergence

- [ ] T060 Verify the Docker image build required by Constitution VI now that `Cargo.lock` changed: run `docker build -f docker/Dockerfile .` in an environment that can pull `rust:1.96-alpine`, confirm Rust 1.96 parses the committed lockfile, and replace the "could not start" note in `docs/rfc-conformance.md` with the recorded result per Constitution VI / T057 (missing)
- [X] T061 Correct the conformance-matrix evidence in `docs/rfc-conformance.md`: replace the seven citations that name non-existent tests (`test_encode_decode_roundtrip`, `test_rejects_nonzero_leading_bits`, `test_rejects_bad_magic_cookie`, `test_rejects_unaligned_length`, `test_rejects_truncated_message`, `test_fingerprint_must_be_terminal`, `test_tampered_message_fails_verification`) with the actual test names in `crates/stun-protocol/tests/` and `src/server/tests/`, and relabel the REALM/NONCE row from §15.2 to §15.7/§15.8 per FR-045 / US1/AC4 / T052 / T059 (partial)
- [X] T062 Give the two umbrella `out-of-scope` rows (RFC 5780 §6–§7, RFC 3489) in `docs/rfc-conformance.md` an explicit MUST/SHOULD/MAY level or restructure them as section-level notes so every ConformanceRow carries a normative level per T015 / plan: ConformanceRow schema (partial)

## Phase 12: Convergence

- [X] T063 Give the remaining umbrella `out-of-scope` row in `docs/rfc-conformance.md` (the TCP/TLS transport row, currently `| — | — | — | TCP/TLS transport for STUN | out-of-scope | ...`) an explicit RFC, section, and MUST/SHOULD/MAY level — e.g. `| 5389 | §9 | MAY |` — so every ConformanceRow carries the normative metadata the plan's schema requires and the document's own Review Checklist asserts per T015 / plan: ConformanceRow schema (partial)
