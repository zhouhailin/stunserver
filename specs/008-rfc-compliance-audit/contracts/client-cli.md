# Contract: `stunclient` CLI and Authentication Behavior

**Feature**: `008-rfc-compliance-audit` | **Binary**: `stunclient`

Existing flags (`server`, `--timeout`, `--quiet`, `--debug`, `--trace`, `--ipv4-only`, `--ipv6-only`) are unchanged.

## New flags

| Flag | Values | Default | Rule |
|------|--------|---------|------|
| `--auth-mode` | `disabled`, `short-term`, `long-term` | `disabled` | Selects the client credential flow |
| `--username` | string | none | Required when `--auth-mode` is not `disabled` |
| `--password` | string | none | Required when `--auth-mode` is not `disabled` unless `STUNCLIENT_PASSWORD` is set |
| `--realm` | string | none | Diagnostic override for the REALM proposed by a long-term challenge; requires `--auth-mode long-term` |
| Environment `STUNCLIENT_PASSWORD` | string | none | Used only when `--password` is omitted |

Rules:

- Supplying `--username` or `--password` with `--auth-mode disabled` is a usage error.
- Supplying `--realm` with any mode other than `long-term` is a usage error; an empty `--realm` is also a usage error.
- The client warns in `--debug` output that `--password` is visible in the process list and recommends `STUNCLIENT_PASSWORD`.
- Passwords and derived keys never appear in output, debug logs, or trace logs.

## Short-term flow

1. Build the Binding Request with SASLprep-prepared USERNAME and MESSAGE-INTEGRITY using the SASLprep-prepared password.
2. Send with RFC 5389 §7.2.1 retransmission: RTO starts at 500 ms, doubles per attempt, at most 7 transmissions, then wait Rm × RTO (8 s) for the final response. `--timeout` remains a hard cap on the whole exchange.
3. A 401 or 438 is a terminal `ServerError`; the client does not retry with credentials.

## Long-term flow

1. Send an initial request without authentication (same retransmission rules).
2. If the response is 401 or 438 and contains REALM and NONCE, derive `MD5(SASLprep(username) ":" SASLprep(realm) ":" SASLprep(password))` and retry with SASLprep-prepared USERNAME, REALM, NONCE, and MESSAGE-INTEGRITY.
   - Without `--realm`, `realm` is the challenge REALM. With `--realm`, the override replaces it in both the REALM attribute and key derivation; the challenge NONCE is still used.
3. If a retry receives 438 with a fresh NONCE, adopt it and retry once more.
4. At most two signed retries. A third 438 becomes a terminal `ServerError`, and a 401 after a signed request is terminal without retry (RFC 5389 §10.2.3).
5. A 401 without REALM or NONCE, or a challenge missing either attribute, becomes a `Protocol` error.

## Response validation

- Datagrams from a different source address, with a mismatched transaction ID, that fail to decode, or that fail response validation are silently discarded and the exchange keeps waiting (RFC 5389 §7.3.3, §10.1.3). If no valid response arrives, the result is a timeout.
- When the client sent MESSAGE-INTEGRITY, any response it accepts (success, 300/420, or other authenticated error) MUST contain MESSAGE-INTEGRITY that validates with the same key. A missing or invalid digest causes the response to be discarded.
- 400 is exempt from the integrity requirement: the server may be unable to compute a valid MESSAGE-INTEGRITY for it (RFC 5389 §15.6). 401 and 438 failure responses are also unsigned.
- When the mode is `disabled`, a 401 or 438 is reported as a structured `ServerError` with no retry.
- Applied length limits: USERNAME at most 512 bytes, REALM and NONCE fewer than 128 characters (RFC 5389 §15.3, §15.7, §15.8). Values that exceed a limit produce a `Protocol` error before sending.
- Existing checks remain: FINGERPRINT terminal and CRC-correct, unknown comprehension-required rejection, unknown optional ignored.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success; mapped address printed |
| 1 | Invalid address or invalid CLI usage |
| 2 | Connection failure |
| 3 | Timeout (no valid response after retransmission) |
| 4 | Protocol error (invalid local credentials or challenge values) |
| 5 | Server error (400, 401, 420, 438, or other ERROR-CODE) |

## Output

Success output keeps the current format:

```text
Response received in 12.3ms
Mapped Address: 203.0.113.5:54321
```

Authentication mode may be logged at INFO/DEBUG as `auth=short-term user=alice`; the password, derived key, REALM nonce MAC, and server secret must not be printed.
