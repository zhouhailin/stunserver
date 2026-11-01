# RFC Conformance Matrix

- **Feature**: `008-rfc-compliance-audit`
- **Last reviewed**: 2026-11-01
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
| 5389 | §6 | MUST | Message length is a multiple of four and delimits exactly one message; trailing bytes are rejected | conformant | `message.rs::decode`; `tests/protocol.rs::test_message_too_short`, `test_trailing_bytes_rejected`; `src/server/tests/integration.rs::test_defective_headers_silently_discarded` | |
| 5389 | §6 | MUST | Transaction ID is a 96-bit value echoed unchanged in responses | conformant | `message.rs`; `src/server/tests/integration.rs`; `crates/stun-protocol/tests/rfc5769_vectors.rs` | Server echo covered; client generation is row §6 random ID |
| 5389 | §6 | MUST | Client transaction IDs are uniformly random (SHOULD be cryptographically random) | conformant | `src/client/client.rs::generate_transaction_id`; `src/client/tests/client.rs::test_generated_transaction_ids_are_random` | 96 bits from `OsRng` (FR-034) |
| 5389 | §6 | MUST | Attributes are padded to four-byte boundaries; padding bits are ignored | conformant | `crates/stun-protocol/src/attributes.rs`; `tests/protocol.rs::test_attribute_padding` | |
| 5389 | §6 | MUST | Comprehension-required range `0x0000`–`0x7FFF`, optional range `0x8000`–`0xFFFF` | conformant | `attributes.rs::is_comprehension_required`; `attributes.rs::test_comprehension_required_boundary_is_exact` | |
| 5389 | §7.1 | MUST | Binding Request processing returns a Binding Success Response for valid requests | conformant | `src/server/server.rs::build_success_response`; `src/server/tests/integration.rs::test_stun_binding_request_response` | |
| 5389 | §7.1 | SHOULD | Client includes SOFTWARE in requests | conformant | `src/client/client.rs::build_request`; `src/client/tests/client.rs::test_binding_request_contains_software` | Value is `stunclient <version>` (FR-035) |
| 5389 | §7.2.1 | SHOULD | Client retransmits UDP requests with a doubling RTO, at most Rc requests, then waits Rm × RTO for the final response | conformant | `src/client/client.rs::exchange`, `retransmission_delays`, `INITIAL_RTO`, `MAX_REQUESTS`, `FINAL_WAIT_MULTIPLIER`; `src/client/tests/client.rs::test_binding_request_retransmits_lost_datagram`, `test_binding_request_sends_rc_requests_and_waits_rm_rto` | RTO starts at 500 ms and doubles; 7 sends at 0/0.5/1.5/3.5/7.5/15.5/31.5 s, then an 8 s final wait (39.5 s total). `--timeout` is a hard cap, so smaller values truncate the schedule |
| 5389 | §7.2.1 | SHOULD | Client caches the RTO per server for the next transaction | out-of-scope | `stunclient` performs one discovery exchange per invocation (one-shot CLI), so there is no subsequent transaction to reuse the value | The 96-bit transaction ID and address fallback already prevent cross-run reuse |
| 5389 | §7.3 | MUST | Server rejects unsupported comprehension-required attributes with 420 | conformant | `server.rs::unsupported_required_attributes`; `integration.rs::test_unknown_required_attribute_gets_420` | |
| 5389 | §7.3 | MUST | Server ignores unknown comprehension-optional attributes | conformant | `integration.rs::test_unknown_optional_attributes_ignored` | |
| 5389 | §7.3 | MUST | Known attributes the server does not act on are not treated as unknown | conformant | `integration.rs::test_known_required_attributes_are_not_listed` | USERNAME/MESSAGE-INTEGRITY/REALM/NONCE ignored when auth is disabled |
| 5389 | §7.3 | MUST | 420 responses contain ERROR-CODE and UNKNOWN-ATTRIBUTES; duplicate types listed once | conformant | `server.rs::build_unknown_attribute_response`; `integration.rs::test_duplicate_unknown_types_listed_once_in_first_seen_order` | |
| 5389 | §7.3 | MUST | Requests failing FINGERPRINT validation, or with a non-final/duplicate FINGERPRINT, are silently discarded | conformant | `server.rs::intake`; `integration.rs::test_bad_fingerprint_crc_silently_discarded`, `test_non_final_fingerprint_silently_discarded`, `test_duplicate_fingerprint_silently_discarded`; `tests/protocol.rs::test_verify_fingerprint_rejects_non_final_attribute`, `test_verify_fingerprint_rejects_duplicate_attributes` | §15.5 requires FINGERPRINT to be the single final attribute; a duplicate makes the first one non-final, and §8 discards messages with FINGERPRINT errors |
| 5389 | §7.3 | MUST | A short first MESSAGE-INTEGRITY is silently discarded; duplicate MESSAGE-INTEGRITY attributes after the first are ignored | conformant | `server.rs::intake`; `crates/stun-protocol/src/message.rs::verify_message_integrity`; `integration.rs::test_short_integrity_silently_discarded`, `test_duplicate_integrity_ignored_after_first`, `test_duplicate_integrity_authenticates_with_first_digest`; `tests/authentication.rs::test_duplicate_message_integrity_is_ignored_after_the_first` | §15.4 ignores every attribute after MESSAGE-INTEGRITY except FINGERPRINT, including a second MESSAGE-INTEGRITY |
| 5389 | §7.3 | MUST | Attributes after MESSAGE-INTEGRITY are ignored except FINGERPRINT | conformant | `server.rs::unsupported_required_attributes`; `src/server/auth.rs::raw_attribute`; `crates/stun-protocol/tests/authentication.rs::test_attributes_after_message_integrity_are_excluded`, `test_duplicate_message_integrity_is_ignored_after_the_first` | |
| 5389 | §7.3 | MUST | Malformed headers, unsupported methods, and non-Request classes receive no response | conformant | `message.rs::decode`; `server.rs::build_response`; `server.rs::test_build_response_rejects_non_request` | |
| 5389 | §7.3.3/§10.1.3 | MUST | Client silently discards responses from another source, with a mismatched transaction ID, undecodable, or failing integrity/fingerprint/attribute validation, and keeps waiting | conformant | `src/client/client.rs::exchange`; `src/client/tests/client.rs::test_binding_request_discards_wrong_transaction_id_then_accepts`, `test_response_from_unexpected_source_is_discarded`, `test_binding_request_malformed_response_is_discarded`, `test_short_term_response_without_integrity_is_discarded` | Discarded datagrams do not abort the UDP exchange |
| 5389 | §10.2.1.2 | SHOULD | Client caches REALM and NONCE across requests | out-of-scope | `stunclient` performs one discovery exchange per invocation (one-shot CLI), so there is no later request to reuse a cache | Documented in README |
| 5389 | §7.3.1 | MUST | Error response class is 4 and ERROR-CODE number identifies the failure | conformant | `server.rs::build_unknown_attribute_response`, `build_auth_error_response` | |
| 5389 | §7.3.1 | SHOULD | Responses include SOFTWARE | conformant | `server.rs::SOFTWARE`, `add_software`; `integration.rs` response-shape tests | Value is `stunserver <version>` |
| 5389 | §10 | MUST | Short-term credentials: missing USERNAME or MESSAGE-INTEGRITY yields unsigned 400 | conformant | `src/server/auth.rs::authenticate_short_term`; `integration.rs::test_auth_short_term_missing_credentials_400` | |
| 5389 | §10 | MUST | Short-term credentials: unknown USERNAME, unusable USERNAME value, or bad MESSAGE-INTEGRITY yields unsigned 401 | conformant | `auth.rs::authenticate_short_term`; `integration.rs::test_auth_short_term_unknown_user_401`, `test_auth_short_term_bad_mac_401`, `test_auth_short_term_empty_username_401`, `test_auth_short_term_non_utf8_username_401` | A missing USERNAME stays 400; a present but empty/non-UTF-8/SASLprep-invalid value is 401 |
| 5389 | §10 | MUST | Short-term success is signed with the same key and does not echo USERNAME | conformant | `integration.rs::test_auth_short_term_success_is_signed` | |
| 5389 | §10 | MUST | Long-term credentials: missing MESSAGE-INTEGRITY yields 401 with REALM and NONCE | conformant | `auth.rs::authenticate_long_term`; `integration.rs::test_auth_long_term_missing_integrity_401_with_challenge` | |
| 5389 | §10 | MUST | Long-term credentials: MESSAGE-INTEGRITY present but USERNAME/REALM/NONCE missing yields 400 | conformant | `integration.rs::test_auth_long_term_integrity_without_nonce_400`, `test_auth_long_term_missing_username_400` | |
| 5389 | §10 | MUST | Long-term credentials: present but invalid USERNAME or REALM yields 401 with REALM and NONCE; a present but invalid NONCE yields 438 | conformant | `auth.rs::authenticate_long_term`; `integration.rs::test_auth_long_term_empty_username_401_with_challenge`, `test_auth_long_term_non_utf8_username_401_with_challenge`, `test_auth_long_term_invalid_realm_401_with_challenge`, `test_auth_long_term_present_invalid_nonce_438` | Validity is checked after NONCE freshness, per §10.2.2 order |
| 5389 | §10 | MUST | Long-term credentials: stale or invalid NONCE yields 438 with a fresh NONCE and REALM | conformant | `auth.rs::verify_nonce_at`; `integration.rs::test_auth_long_term_stale_nonce_438_with_fresh_challenge` | Stateless HMAC nonce, 600 s default lifetime |
| 5389 | §10 | MUST | Long-term credentials: unknown USERNAME, REALM mismatch, or bad MAC yields 401 with REALM and NONCE | conformant | `integration.rs::test_auth_long_term_unknown_user_401_with_challenge`, `test_auth_long_term_realm_mismatch_401`, `test_auth_long_term_bad_mac_401` | |
| 5389 | §10 | MUST | Long-term success is signed and omits USERNAME, REALM, and NONCE | conformant | `integration.rs::test_auth_long_term_success_is_signed` | |
| 5389 | §10 | MUST | Enabled authentication never falls back to an unauthenticated success | conformant | `integration.rs::test_auth_enabled_modes_never_succeed_unauthenticated` | |
| 5389 | §10.2.3 | MUST | Client does not retry a 401 after a signed request without changing credentials | conformant | `src/client/client.rs::long_term_flow`; `src/client/tests/client.rs::test_long_term_401_after_signed_request_is_terminal`, `test_long_term_401_challenge_retries_once`, `test_long_term_438_adopts_fresh_nonce_within_retry_bound` | 438 still retries with the fresh NONCE within two signed retries |
| 5389 | §15 | MUST | MESSAGE-INTEGRITY is HMAC-SHA1 over the message up to the attribute with the header length adjusted to end at the digest | conformant | `crates/stun-protocol/src/integrity.rs::hmac_input`; `tests/integrity.rs`; `tests/authentication.rs::test_manual_hmac_range_matches_encoding` | |
| 5389 | §15 | MUST | MESSAGE-INTEGRITY comparison is constant-time | conformant | `integrity.rs::verify_hmac_sha1` (`subtle`) | |
| 5389 | §15 | MUST | MESSAGE-INTEGRITY is the last authenticated attribute | conformant | `authentication.rs::test_attributes_after_message_integrity_are_excluded`; `server.rs::sign_response` | |
| 5389 | §15 | MUST | FINGERPRINT is CRC-32 of the message before it with the header length at the final length, XOR `0x5354554E` | conformant | `integrity.rs::fingerprint`; `tests/integrity.rs`; `tests/protocol.rs` FINGERPRINT tests | Computation conformant; intake enforcement tracked in §7.3 row |
| 5389 | §15 | MUST | FINGERPRINT is exactly four bytes and terminal | conformant | `attributes.rs::decode`; `tests/protocol.rs::test_encode_with_fingerprint_is_terminal`, `test_verify_fingerprint_rejects_non_final_attribute` | Intake enforcement tracked in §7.3 row |
| 5389 | §15.1/§15.2 | MUST | MAPPED-ADDRESS and XOR-MAPPED-ADDRESS values are exactly 8 bytes (IPv4) or 20 bytes (IPv6); anything shorter or longer is rejected | conformant | `crates/stun-protocol/src/address.rs::decode`, `decode_xor`; `tests/protocol.rs::test_decode_mapped_address_rejects_ipv4_trailing_bytes`, `test_decode_mapped_address_rejects_ipv6_trailing_bytes`, `test_decode_xor_exact_ipv4_length`, `test_decode_xor_exact_ipv6_length` | |
| 5389 | §15.3 | MUST | USERNAME is SASLprep-prepared and at most 512 bytes; an unusable USERNAME is rejected with 401 and an over-long one with 400 | conformant | `integrity.rs::saslprep`; `src/server/auth.rs::prepare_text`; `src/server/config.rs::precompute_keys`; `src/client/client.rs::prepared_username`; `crates/stun-protocol/tests/authentication.rs::test_long_term_username_and_realm_saslprep`; `integration.rs::test_auth_long_term_non_ascii_username_and_realm_saslprep_end_to_end`, `test_auth_long_term_empty_username_401_with_challenge`, `test_auth_short_term_non_utf8_username_401` | Client, server, and config all prepare; config rejects 513-byte names |
| 5389 | §15.4 | MUST | Long-term key is `MD5(SASLprep(username) ":" SASLprep(realm) ":" SASLprep(password))` | conformant | `integrity.rs::long_term_key`, `strip_credential`, `saslprep`; `tests/integrity.rs`; `tests/authentication.rs::test_long_term_non_ascii_password_saslprep`, `test_long_term_username_and_realm_saslprep` | Full RFC 4013 profile; USERNAME/REALM lose surrounding quotes and trailing nulls, password loses trailing nulls, per §15.4 (`tests/integrity.rs::test_long_term_key_strips_username_realm_quotes_and_trailing_nulls`, `test_long_term_key_password_only_strips_trailing_nulls`) |
| 5389 | §15.4 | MUST | Short-term key is `SASLprep(password)` | conformant | `integrity.rs::short_term_key`, `saslprep`; `tests/integrity.rs::test_short_term_key_uses_saslprep`; `tests/authentication.rs::test_short_term_non_ascii_password_saslprep`; `integration.rs::test_auth_short_term_non_ascii_password_saslprep_end_to_end` | Full RFC 4013 profile; ASCII passwords are unchanged |
| 5389 | §15.6 | MUST | ERROR-CODE class is 3–6, number is 0–99, and the reason phrase is valid UTF-8 under 128 characters, enforced on both encode and decode | conformant | `crates/stun-protocol/src/error_code.rs::validate_code`, `validate_reason`, `ErrorCode::new`, `decode_value`; `tests/error_code.rs::test_error_code_new_validates_class_and_number`, `test_error_code_new_rejects_overlong_reason`, `test_error_code_decode_rejects_invalid_class_and_number`, `test_error_code_decode_rejects_invalid_utf8_reason`, `test_error_code_decode_rejects_overlong_reason` | Invalid codes in a response are discarded by the client |
| 5389 | §15.6 | MUST | Client accepts an unsigned 400 error, for which the server may be unable to compute a valid MESSAGE-INTEGRITY | conformant | `src/client/client.rs::parse_error_response`; `src/client/tests/client.rs::test_short_term_400_without_integrity_is_accepted` | Other non-challenge errors still require integrity |
| 5389 | §15.7/§15.8 | MUST | REALM is SASLprep-prepared and shorter than 128 characters; NONCE is shorter than 128 characters; challenges carry both and retries are validated | conformant | `auth.rs::issue_nonce`, `verify_nonce`, `authenticate_long_term`; `config.rs::AuthConfig::MAX_REALM_CHARS`, `precompute_keys`; `client.rs::MAX_REALM_CHARS`, `build_request`; `src/server/config.rs::test_auth_long_term_realm_uses_saslprep`, `test_auth_rejects_oversized_username_and_realm`; `src/client/tests/client.rs::test_build_request_rejects_oversized_challenge_realm` | NONCE is stateless: 8-byte seconds + 16-byte truncated HMAC, 48 hex characters |

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
| 5780 | §6 | MUST | A single-address server rejects CHANGE-REQUEST with 420 | conformant | `src/server/server.rs::INTERCEPTED_RFC5780`, `classify_required_attribute`; `integration.rs::test_rfc5780_change_request_with_flags_rejected`, `test_rfc5780_and_unknown_types_listed_once_in_first_seen_order`, `test_rfc5780_authenticated_420_is_signed` | Interception runs after authentication and stops at MESSAGE-INTEGRITY; `test_rfc5780_interception_loses_to_authentication_failure` pins the ordering |
| 5780 | §6 | MUST | PADDING and RESPONSE-PORT are rejected with 420 | conformant | `integration.rs::test_rfc5780_padding_of_any_length_rejected`, `test_rfc5780_response_port_rejected`, `test_rfc5780_padding_with_response_port_rejected_listing_both`, `test_rfc5780_attributes_after_integrity_ignored`, `test_rfc5780_optional_cache_timeout_ignored` | Combined PADDING+RESPONSE-PORT answered with 420 listing both (deviation from §6.1's 400, recorded in `specs/007-stun-rfc-error-handling/spec.md`); optional CACHE-TIMEOUT stays ignored; rationale in "RFC 5780 attribute rejection policy" below |
| 5780 | §6.1 | MUST | Behavior-discovery responses contain both MAPPED-ADDRESS and XOR-MAPPED-ADDRESS | out-of-scope | `src/server/server.rs::build_success_response` (XOR-MAPPED-ADDRESS only); `specs/008-rfc-compliance-audit/spec.md` records the decision | §6.1 applies to the NAT-behavior-discovery Usage, which this server does not advertise. Plain RFC 5389 Binding responses require XOR-MAPPED-ADDRESS; MAPPED-ADDRESS is optional under §15.1 |
| 5780 | §7 | MUST | Responses never contain OTHER-ADDRESS or RESPONSE-ORIGIN when behavior discovery is not offered | conformant | `integration.rs::test_responses_never_contain_other_address` | |
| 5780 | §7 | MUST | Documentation and deployment configuration do not advertise `stun-behavior`/`stun-behaviors` | conformant | `src/server/tests/rfc5780_advertising.rs::test_no_rfc5780_service_advertising` | Scans READMEs, `config/`, `deploy/`, `docker/`, `systemd/`, and the Makefile; excludes `docs/rfcs/` |
| 5780 | §6–§7 | MAY | NAT behavior discovery (MAPPED-ADDRESS comparison, response origin discovery) | out-of-scope | Single-address server; documented in README | Not offered by design; success responses omit MAPPED-ADDRESS, which RFC 5389 §15.1 makes optional |
| 3489 | — | MAY | RFC 3489 backward-compatible Binding (no magic cookie) | out-of-scope | `message.rs` requires the RFC 5389 magic cookie | Client uses `send_binding_request_with_fallback` only for transport retry, not RFC 3489 framing |
| 5389 | §9 | MAY | TCP/TLS transport for STUN | out-of-scope | Only UDP is implemented; `[security]` TLS fields are reserved | RFC 5389 allows UDP-only deployments |

### RFC 5780 attribute rejection policy

This deployment is a plain RFC 5389 Binding server on a single address and port. It does not implement the RFC 5780 NAT-behavior-discovery Usage and never advertises `stun-behavior`/`stun-behaviors`. Requests carrying CHANGE-REQUEST, PADDING, or RESPONSE-PORT are answered with ERROR-CODE 420 and an UNKNOWN-ATTRIBUTES list of the offending types in first-seen order (`src/server/server.rs::unsupported_required_attributes`).

Rationale per attribute:

- **CHANGE-REQUEST** is the case RFC 5780 §6 makes mandatory: a server that cannot allocate the same ports on two addresses "MUST respond with a 420 ... to any Request with a CHANGE-REQUEST attribute".
- **RESPONSE-PORT** asks the server to send the response to a different port. Redirection is not implemented, so a success response to the source port would contradict the request; RFC 5780 §5 explicitly tells clients to expect 420 for RESPONSE-PORT because support is optional.
- **PADDING** has defined response semantics only inside the behavior-discovery Usage, where RFC 5780 §6.1 requires the server to echo it. Ignoring PADDING would return an ordinary success response, letting a probing client mistake a response that was never padded for behavior-discovery output; 420 states the refusal unambiguously.

RFC 5389 §7.3 says known-but-unexpected comprehension-required attributes SHOULD be ignored, so answering 420 for PADDING and RESPONSE-PORT is a deliberate policy choice rather than a literal reading of that paragraph. The policy is chosen for consistency with the CHANGE-REQUEST requirement and because RFC 5780 clients are already required to tolerate 420 for these attributes. It is recorded as FR-008 in `specs/007-stun-rfc-error-handling/spec.md`.

Consequences:

- A request carrying both PADDING and RESPONSE-PORT receives one 420 listing both types; RFC 5780 §6.1's 400 applies only to servers that implement the Usage.
- Rejection runs after FINGERPRINT validation and authentication, stops at MESSAGE-INTEGRITY, and an authenticated request receives a signed 420 (`test_rfc5780_authenticated_420_is_signed`).
- Comprehension-optional RFC 5780 attributes such as CACHE-TIMEOUT remain ignored per RFC 5389 §7.3 (`test_rfc5780_optional_cache_timeout_ignored`).
- The server never emits OTHER-ADDRESS, RESPONSE-ORIGIN, or a PADDING echo, so no response can be mistaken for behavior-discovery output.

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
