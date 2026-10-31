# Phase 1 Data Model: RFC Conformance and Authentication

**Feature**: `008-rfc-compliance-audit` | **Date**: 2026-10-31

## Configuration entities

### AuthConfig

Loaded once from the `[auth]` TOML section.

| Field | Type | Default | Validation |
|-------|------|---------|------------|
| `mode` | `disabled` \| `short-term` \| `long-term` | `disabled` | Unknown values are a configuration error |
| `realm` | string | `stunserver` | Required non-empty when mode is `long-term`; ignored otherwise |
| `nonce_lifetime_sec` | integer | `600` | 1–86400 when mode is `long-term`; ignored otherwise |
| `users` | list of `UserCredential` | empty | At least one entry required when mode is not `disabled`; usernames unique and non-empty; passwords non-empty |

Relationships: one `AuthConfig` owns zero or more `UserCredential` entries; mode selects how their keys are derived.

### UserCredential

| Field | Type | Validation |
|-------|------|------------|
| `username` | string | Non-empty UTF-8, unique within the list |
| `password` | string | Non-empty; SASLprep-normalized before long-term key derivation |
| `key` | bytes (computed) | Raw password bytes for short-term; 16-byte MD5 digest for long-term; never serialized or logged |

Lifecycle: created when configuration loads; immutable for the server lifetime; dropped on shutdown.

### AuthMode

State values: `Disabled`, `ShortTerm`, `LongTerm`. Exactly one is active per process (server) or per client invocation.

## Protocol entities

### AuthenticatedAttributeSet

The decoded attributes that participate in authentication.

| Attribute | Type code | Required for | Handling |
|-----------|-----------|--------------|----------|
| USERNAME | `0x0006` | short-term and long-term | Value bytes are compared to the credential username |
| MESSAGE-INTEGRITY | `0x0008` | both | Exactly 20 bytes; at most one; last authenticated attribute |
| REALM | `0x0014` | long-term | Compared to configured realm |
| NONCE | `0x0015` | long-term | Validated by MAC and lifetime |
| FINGERPRINT | `0x8028` | optional | Final attribute only; validated before auth |

Invariant: attributes after the first MESSAGE-INTEGRITY are retained in the decoded list but are invisible to authentication and unknown-attribute classification.

### Nonce

Wire value: 48 lowercase hex characters encoding 24 bytes.

| Component | Size | Meaning |
|-----------|------|---------|
| issued_at | 8 bytes, big-endian seconds | Issue time used for expiry |
| mac | 16 bytes | HMAC-SHA1(server_secret, issued_at)[0..16] |

Lifecycle: `Issued` (created on 401/438 challenge) → `Valid` (MAC matches and age ≤ lifetime) → `Stale` (age > lifetime or MAC mismatch). Stale nonces are never stored; the server reissues one in the 438 response.

### AuthDecision

Result of the authentication stage for one request.

| Variant | Trigger | Response |
|---------|---------|----------|
| `Disabled` | mode is disabled | Continue as a public Binding request; auth attributes ignored |
| `Authenticated { username, mechanism }` | credentials valid | Continue; response signed with the same key |
| `Reject { status, reason, challenge }` | missing/invalid credentials | 400, 401, or 438; `challenge` carries REALM and NONCE for long-term 401/438 |

Decision order (fixed):

1. Short-term missing USERNAME or MESSAGE-INTEGRITY → `Reject 400`.
2. Short-term unknown username or bad MAC → `Reject 401`.
3. Long-term missing MESSAGE-INTEGRITY → `Reject 401` with REALM + NONCE.
4. Long-term MESSAGE-INTEGRITY present but USERNAME, REALM, or NONCE missing → `Reject 400`.
5. Long-term nonce stale or invalid → `Reject 438` with fresh NONCE + REALM.
6. Long-term unknown username, realm mismatch, or bad MAC → `Reject 401` with REALM + NONCE.
7. Otherwise → `Authenticated`.

### Response shape

| Response | Attribute order | MESSAGE-INTEGRITY |
|----------|-----------------|-------------------|
| Success | XOR-MAPPED-ADDRESS, SOFTWARE, MESSAGE-INTEGRITY | Present only when the request authenticated |
| 420 | ERROR-CODE, UNKNOWN-ATTRIBUTES, SOFTWARE, MESSAGE-INTEGRITY | Present only when the request authenticated |
| 400 | ERROR-CODE, SOFTWARE | Absent |
| 401 | ERROR-CODE, REALM, NONCE, SOFTWARE (long-term only for REALM/NONCE) | Absent |
| 438 | ERROR-CODE, REALM, NONCE, SOFTWARE | Absent |

### ConformanceRow

One row in `docs/rfc-conformance.md`.

| Field | Type | Rule |
|-------|------|------|
| RFC | string | `5389`, `5769`, or `5780` |
| Section | string | Section identifier such as `§10.2.2` |
| Level | `MUST` \| `SHOULD` \| `MAY` | Normative level as written |
| Requirement | string | Short paraphrase |
| Status | `conformant` \| `gap` \| `out-of-scope` | Every row must be classified |
| Evidence | string | Test name/path or manual review note |
| Notes | string | Gap owner (FR id) or out-of-scope justification |

## State transition: request processing

```text
Received
  → HeaderValid? ── no → SilentlyDiscard
  → BindingRequest? ── no → SilentlyDiscard
  → FingerprintValid? ── no → SilentlyDiscard
  → AuthDecision
       Reject → SendError (400/401/438, unsigned)
       Disabled/Authenticated → ClassifyUnknownRequired
            none → SendSuccess (signed if Authenticated)
            some → Send420 (signed if Authenticated)
```
