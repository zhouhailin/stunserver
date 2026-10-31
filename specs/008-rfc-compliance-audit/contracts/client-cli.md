# Contract: `stunclient` CLI and Authentication Behavior

**Feature**: `008-rfc-compliance-audit` | **Binary**: `stunclient`

Existing flags (`server`, `--timeout`, `--quiet`, `--debug`, `--trace`, `--ipv4-only`, `--ipv6-only`) are unchanged.

## New flags

| Flag | Values | Default | Rule |
|------|--------|---------|------|
| `--auth-mode` | `disabled`, `short-term`, `long-term` | `disabled` | Selects the client credential flow |
| `--username` | string | none | Required when `--auth-mode` is not `disabled` |
| `--password` | string | none | Required when `--auth-mode` is not `disabled` unless `STUNCLIENT_PASSWORD` is set |
| Environment `STUNCLIENT_PASSWORD` | string | none | Used only when `--password` is omitted |

Rules:

- Supplying `--username` or `--password` with `--auth-mode disabled` is a usage error.
- The client warns in `--debug` output that `--password` is visible in the process list and recommends `STUNCLIENT_PASSWORD`.
- Passwords and derived keys never appear in output, debug logs, or trace logs.

## Short-term flow

1. Build the Binding Request with USERNAME and MESSAGE-INTEGRITY using the supplied password.
2. Send once.
3. A 401 or 438 is a terminal `ServerError`; the client does not retry.

## Long-term flow

1. Send an initial request without authentication.
2. If the response is 401 or 438 and contains REALM and NONCE, derive `MD5(username ":" realm ":" SASLprep(password))` and retry with USERNAME, REALM, NONCE, and MESSAGE-INTEGRITY.
3. If a retry receives 438 with a fresh NONCE, adopt it and retry once more.
4. At most two signed retries. A third challenge becomes a terminal `ServerError`.
5. A 401 without REALM or NONCE, or a challenge missing either attribute, becomes a `Protocol` error.

## Response validation

- When the client sent MESSAGE-INTEGRITY, any response it accepts (success, 420, or other authenticated error) MUST contain MESSAGE-INTEGRITY that validates with the same key. A missing or invalid digest is a `Protocol` error.
- When the mode is `disabled`, a 401 or 438 is reported as a structured `ServerError` with no retry.
- Existing checks remain: transaction ID match, source address match, FINGERPRINT terminal and CRC-correct, unknown comprehension-required rejection, unknown optional ignored.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success; mapped address printed |
| 1 | Invalid address or invalid CLI usage |
| 2 | Connection failure |
| 3 | Timeout |
| 4 | Protocol error (including invalid/missing response MESSAGE-INTEGRITY) |
| 5 | Server error (400, 401, 420, 438, or other ERROR-CODE) |

## Output

Success output keeps the current format:

```text
Response received in 12.3ms
Mapped Address: 203.0.113.5:54321
```

Authentication mode may be logged at INFO/DEBUG as `auth=short-term user=alice`; the password, derived key, REALM nonce MAC, and server secret must not be printed.
