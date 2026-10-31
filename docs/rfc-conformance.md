# RFC Conformance Matrix

- **Feature**: `008-rfc-compliance-audit`
- **Last reviewed**: 2026-10-31
- **Scope**: UDP Binding server, bundled diagnostic client, and `stun-protocol` library
- **Status vocabulary**: `conformant` (implemented and evidenced), `gap` (known missing behavior with a closing requirement), `out-of-scope` (not offered by this deployment; justification given)
- **Evidence conventions**: test names are given as `<file>::<test>`; manual evidence names the artifact inspected
- **Exclusion**: `docs/rfcs/` holds normative reference text and is excluded from advertising and compatibility scans

## RFC 5389 — STUN

| RFC | Section | Level | Requirement | Status | Evidence | Notes |
|-----|---------|-------|-------------|--------|----------|-------|
| 5389 | §6 | MUST | Message header is exactly 20 bytes: type, length, magic cookie, transaction ID | conformant | `crates/stun-protocol/src/message.rs`; `crates/stun-protocol/tests/protocol.rs::test_message_encode_decode_roundtrip` | |
| 5389 | §6 | MUST | The two most significant bits of the message type are zero | conformant | `message.rs::decode`; `src/server/tests/integration.rs::test_defective_headers_silently_discarded` | |
| 5389 | §6 | MUST | Magic cookie equals `0x2112A442` | conformant | `message.rs::decode`; `tests/protocol.rs::test_invalid_magic_cookie` | |
| 5389 | §6 | MUST | Message length is a multiple of four and never exceeds the datagram | conformant | `message.rs::decode`; `tests/protocol.rs::test_message_too_short`; `src/server/tests/integration.rs::test_defective_headers_silently_discarded` | |
| 5389 | §6 | MUST | Transaction ID is a 96-bit value echoed unchanged in responses | conformant | `message.rs`; `src/server/tests/integration.rs`; `crates/stun-protocol/tests/rfc5769_vectors.rs` | Server echo covered; client generation is row §6 random ID |
| 5389 | §6 | MUST | Client transaction IDs are uniformly random (SHOULD be cryptographically random) | conformant | `src/client/client.rs::generate_transaction_id`; `src/client/tests/client.rs::test_generated_transaction_ids_are_random` | 96 bits from `OsRng` (FR-034) |
| 5389 | §6 | MUST | Attributes are padded to four-byte boundaries; padding bits are ignored | conformant | `crates/stun-protocol/src/attributes.rs`; `tests/protocol.rs::test_attribute_padding` | |
| 5389 | §6 | MUST | Comprehension-required range `0x0000`–`0x7FFF`, optional range `0x8000`–`0xFFFF` | conformant | `attributes.rs::is_comprehension_required`; `attributes.rs::test_comprehension_required_boundary_is_exact` | |
| 5389 | §7.1 | MUST | Binding Request processing returns a Binding Success Response for valid requests | conformant | `src/server/server.rs::build_success_response`; `src/server/tests/integration.rs::test_stun_binding_request_response` | |
| 5389 | §7.1 | SHOULD | Client includes SOFTWARE in requests | conformant | `src/client/client.rs::build_request`; `src/client/tests/client.rs::test_binding_request_contains_software` | Value is `stunclient <version>` (FR-035) |
| 5389 | §7.3 | MUST | Server rejects unsupported comprehension-required attributes with 420 | conformant | `server.rs::unsupported_required_attributes`; `integration.rs::test_unknown_required_attribute_gets_420` | |
| 5389 | §7.3 | MUST | Server ignores unknown comprehension-optional attributes | conformant | `integration.rs::test_unknown_optional_attributes_ignored` | |
| 5389 | §7.3 | MUST | Known attributes the server does not act on are not treated as unknown | conformant | `integration.rs::test_known_required_attributes_are_not_listed` | USERNAME/MESSAGE-INTEGRITY/REALM/NONCE ignored when auth is disabled |
| 5389 | §7.3 | MUST | 420 responses contain ERROR-CODE and UNKNOWN-ATTRIBUTES; duplicate types listed once | conformant | `server.rs::build_unknown_attribute_response`; `integration.rs::test_duplicate_unknown_types_listed_once_in_first_seen_order` | |
| 5389 | §7.3 | MUST | Requests failing FINGERPRINT validation, or with a non-final/duplicate FINGERPRINT, are silently discarded | conformant | `server.rs::intake`; `integration.rs::test_bad_fingerprint_crc_silently_discarded`, `test_non_final_fingerprint_silently_discarded`, `test_duplicate_fingerprint_silently_discarded`; `tests/protocol.rs::test_verify_fingerprint_rejects_non_final_attribute`, `test_verify_fingerprint_rejects_duplicate_attributes` | |
| 5389 | §7.3 | MUST | Duplicate or short MESSAGE-INTEGRITY is silently discarded | conformant | `server.rs::intake`; `integration.rs::test_duplicate_integrity_silently_discarded`, `test_short_integrity_silently_discarded`; `protocol.rs::test_verify_integrity_rejects_duplicate_attributes` | |
| 5389 | §7.3 | MUST | Attributes after MESSAGE-INTEGRITY are ignored except FINGERPRINT | conformant | `server.rs::unsupported_required_attributes`; `src/server/auth.rs::text_attribute`; `crates/stun-protocol/tests/authentication.rs::test_attributes_after_message_integrity_are_excluded` | |
| 5389 | §7.3 | MUST | Malformed headers, unsupported methods, and non-Request classes receive no response | conformant | `message.rs::decode`; `server.rs::build_response`; `server.rs::test_build_response_rejects_non_request` | |
| 5389 | §7.3.1 | MUST | Error response class is 4 and ERROR-CODE number identifies the failure | conformant | `server.rs::build_unknown_attribute_response`, `build_auth_error_response` | |
| 5389 | §7.3.1 | SHOULD | Responses include SOFTWARE | conformant | `server.rs::SOFTWARE`, `add_software`; `integration.rs` response-shape tests | Value is `stunserver <version>` |
| 5389 | §10 | MUST | Short-term credentials: missing USERNAME or MESSAGE-INTEGRITY yields unsigned 400 | conformant | `src/server/auth.rs::authenticate_short_term`; `integration.rs::test_auth_short_term_missing_credentials_400` | |
| 5389 | §10 | MUST | Short-term credentials: unknown USERNAME or bad MESSAGE-INTEGRITY yields unsigned 401 | conformant | `auth.rs::authenticate_short_term`; `integration.rs::test_auth_short_term_unknown_user_401`, `test_auth_short_term_bad_mac_401` | |
| 5389 | §10 | MUST | Short-term success is signed with the same key and does not echo USERNAME | conformant | `integration.rs::test_auth_short_term_success_is_signed` | |
| 5389 | §10 | MUST | Long-term credentials: missing MESSAGE-INTEGRITY yields 401 with REALM and NONCE | conformant | `auth.rs::authenticate_long_term`; `integration.rs::test_auth_long_term_missing_integrity_401_with_challenge` | |
| 5389 | §10 | MUST | Long-term credentials: MESSAGE-INTEGRITY present but USERNAME/REALM/NONCE missing yields 400 | conformant | `integration.rs::test_auth_long_term_integrity_without_nonce_400` | |
| 5389 | §10 | MUST | Long-term credentials: stale or invalid NONCE yields 438 with a fresh NONCE and REALM | conformant | `auth.rs::verify_nonce_at`; `integration.rs::test_auth_long_term_stale_nonce_438_with_fresh_challenge` | Stateless HMAC nonce, 600 s default lifetime |
| 5389 | §10 | MUST | Long-term credentials: unknown USERNAME, REALM mismatch, or bad MAC yields 401 with REALM and NONCE | conformant | `integration.rs::test_auth_long_term_unknown_user_401_with_challenge`, `test_auth_long_term_realm_mismatch_401`, `test_auth_long_term_bad_mac_401` | |
| 5389 | §10 | MUST | Long-term success is signed and omits USERNAME, REALM, and NONCE | conformant | `integration.rs::test_auth_long_term_success_is_signed` | |
| 5389 | §10 | MUST | Enabled authentication never falls back to an unauthenticated success | conformant | `integration.rs::test_auth_enabled_modes_never_succeed_unauthenticated` | |
| 5389 | §15 | MUST | MESSAGE-INTEGRITY is HMAC-SHA1 over the message up to the attribute with the header length adjusted to end at the digest | conformant | `crates/stun-protocol/src/integrity.rs::hmac_input`; `tests/integrity.rs`; `tests/authentication.rs::test_manual_hmac_range_matches_encoding` | |
| 5389 | §15 | MUST | MESSAGE-INTEGRITY comparison is constant-time | conformant | `integrity.rs::verify_hmac_sha1` (`subtle`) | |
| 5389 | §15 | MUST | MESSAGE-INTEGRITY is the last authenticated attribute | conformant | `authentication.rs::test_attributes_after_message_integrity_are_excluded`; `server.rs::sign_response` | |
| 5389 | §15 | MUST | FINGERPRINT is CRC-32 of the message before it with the header length at the final length, XOR `0x5354554E` | conformant | `integrity.rs::fingerprint`; `tests/integrity.rs`; `tests/protocol.rs` FINGERPRINT tests | Computation conformant; intake enforcement tracked in §7.3 row |
| 5389 | §15 | MUST | FINGERPRINT is exactly four bytes and terminal | conformant | `attributes.rs::decode`; `tests/protocol.rs::test_encode_with_fingerprint_is_terminal`, `test_verify_fingerprint_rejects_non_final_attribute` | Intake enforcement tracked in §7.3 row |
| 5389 | §15.4 | MUST | Long-term key is `MD5(username ":" realm ":" SASLprep(password))` | conformant | `integrity.rs::long_term_key`, `saslprep`; `tests/integrity.rs`; `tests/authentication.rs::test_long_term_non_ascii_password_saslprep` | Full RFC 4013 profile |
| 5389 | §15.4 | MUST | Short-term key is the raw password bytes | conformant | `integrity.rs::short_term_key`; `tests/authentication.rs::test_short_term_hmac_verifies` | |
| 5389 | §15.7/§15.8 | MUST | REALM and NONCE are carried in long-term challenges and validated on retry | conformant | `auth.rs::issue_nonce`, `verify_nonce`; `integration.rs` long-term tests | NONCE is stateless: 8-byte seconds + 16-byte truncated HMAC, 48 hex |

## RFC 5769 — Test Vectors

| RFC | Section | Level | Requirement | Status | Evidence | Notes |
|-----|---------|-------|-------------|--------|----------|-------|
| 5769 | §2 | MUST | Published sample messages decode and re-encode byte-for-byte | conformant | `crates/stun-protocol/tests/rfc5769_vectors.rs::test_rfc5769_sample_request`, `test_rfc5769_sample_ipv4_response`, `test_rfc5769_sample_ipv6_response`, `test_rfc5769_sample_long_term_request` | |
| 5769 | §2.1 | MUST | Sample request MESSAGE-INTEGRITY matches the published digest | conformant | `rfc5769_vectors.rs::test_rfc5769_short_term_integrity_and_fingerprint` | Short-term key `VOkJxbRl1RmTxUk/WvJxBt` |
| 5769 | §2.1 | MUST | Sample request FINGERPRINT matches the published CRC value | conformant | `rfc5769_vectors.rs::test_rfc5769_short_term_integrity_and_fingerprint` | |
| 5769 | §2.2 | MUST | IPv4 response MESSAGE-INTEGRITY and FINGERPRINT match the published values | conformant | `rfc5769_vectors.rs::test_rfc5769_short_term_integrity_and_fingerprint` | |
| 5769 | §2.3 | MUST | IPv6 response MESSAGE-INTEGRITY and FINGERPRINT match the published values | conformant | `rfc5769_vectors.rs::test_rfc5769_short_term_integrity_and_fingerprint` | |
| 5769 | §2.4 | MUST | Long-term credential request key derived from the documented username/realm/password verifies MESSAGE-INTEGRITY | conformant | `rfc5769_vectors.rs::test_rfc5769_long_term_key_verifies_integrity` | Username `マトリックス`, realm `example.org`, password `TheMatrIX` |
| 5769 | §2 | MUST | Tampering any covered byte causes verification failure | conformant | `rfc5769_vectors.rs::test_rfc5769_tampering_is_detected`; `tests/integrity.rs::test_verify_hmac_sha1_rejects_tampering`; `tests/authentication.rs::test_tampered_request_fails_verification` | |

## RFC 5780 — NAT Behavior Discovery Attributes

| RFC | Section | Level | Requirement | Status | Evidence | Notes |
|-----|---------|-------|-------------|--------|----------|-------|
| 5780 | §6 | MUST | CHANGE-REQUEST, PADDING, and RESPONSE-PORT codepoints are recognized | conformant | `crates/stun-protocol/src/attributes.rs`; `attributes.rs::test_rfc5780_attribute_registry` | |
| 5780 | §6 | MUST | A single-address server rejects CHANGE-REQUEST with 420 | conformant | `src/server/server.rs::REJECTED_REQUIRED`; `integration.rs::test_rfc5780_unsupported_attributes_get_420` | |
| 5780 | §6 | MUST | PADDING and RESPONSE-PORT are rejected with 420 | conformant | `integration.rs::test_rfc5780_unsupported_attributes_get_420` | |
| 5780 | §7 | MUST | Responses never contain OTHER-ADDRESS or RESPONSE-ORIGIN when behavior discovery is not offered | conformant | `integration.rs::test_responses_never_contain_other_address` | |
| 5780 | §7 | MUST | Documentation and deployment configuration do not advertise `stun-behavior`/`stun-behaviors` | conformant | `src/server/tests/rfc5780_advertising.rs::test_no_rfc5780_service_advertising` | Scans READMEs, `config/`, `deploy/`, `docker/`, `systemd/`, and the Makefile; excludes `docs/rfcs/` |
| 5780 | §6–§7 | MAY | NAT behavior discovery (MAPPED-ADDRESS comparison, response origin discovery) | out-of-scope | Single-address server; documented in README | Not offered by design |
| 3489 | — | MAY | RFC 3489 backward-compatible Binding (no magic cookie) | out-of-scope | `message.rs` requires the RFC 5389 magic cookie | Client uses `send_binding_request_with_fallback` only for transport retry, not RFC 3489 framing |
| 5389 | §9 | MAY | TCP/TLS transport for STUN | out-of-scope | Only UDP is implemented; `[security]` TLS fields are reserved | RFC 5389 allows UDP-only deployments |

## Quickstart Validation (T055)

Scenarios 1-9 from `specs/008-rfc-compliance-audit/quickstart.md` were executed against release binaries.

- Deviation: scenarios 3-5 used `127.0.0.1:3499` instead of `3478` because an unrelated server instance already held 3478; port choice does not affect the assertions.
- Short-term: correct credentials produced a mapped address (exit 0); wrong password produced 401 (exit 5); no credentials produced 400 (exit 5); `STUNCLIENT_PASSWORD` worked; `--debug` warned about process-list visibility and logged only `auth=short-term user=alice`.
- Long-term: the debug trace showed an unsigned 401 challenge, one signed retry, a MESSAGE-INTEGRITY-validated success (exit 0); no credentials and wrong password produced 401 (exit 5) with no retry.
- Unauthenticated: mapped address returned (exit 0); credentials with `--auth-mode disabled` were rejected as a usage error (exit 1).
- Secrets: `grep -iE 'correct-horse|password|server_secret|nonce_secret'` over the server log and stdout found no matches.
- RFC 5769 and RFC 5780 suites passed, including the advertising scan.
- Docker (T057): `docker build -f docker/Dockerfile .` could not start because this environment cannot reach the Docker registry to pull `rust:1.96-alpine`; the Dockerfile is unchanged and the equivalent host `cargo build --release` passed.

## Performance (SC-008)

`stunbench --qps 1000 --duration 10` against a release server with authentication disabled:

| Run | Requests | Success | p99 |
|-----|----------|---------|-----|
| Pre-change baseline (recorded in T002) | ~10,000 | 100% | 0.50 ms |
| Post-change plain | 10,002 | 100% | 0.50 ms |
| Post-change `--fingerprint` | 10,001 | 100% | 0.50 ms |

- Plain vs baseline: +0.0% (SC-008 allows +5%).
- Fingerprint vs plain: +0.0%.
- Both runs sit on the benchmark's 0.5 ms latency floor, so FINGERPRINT validation adds no measurable p99 cost.

## Review Checklist

- Every row above carries an RFC, section, normative level, status, and evidence or out-of-scope justification.
- A `gap` row names the requirement that closes it (FR-001–FR-045); no gap rows remain after the audit.
- The matrix is updated in the same change as any future protocol behavior change.
