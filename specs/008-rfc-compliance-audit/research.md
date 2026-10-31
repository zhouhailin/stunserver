# Phase 0 Research: RFC 5389/5769/5780 Conformance

**Feature**: `008-rfc-compliance-audit` | **Date**: 2026-10-31

All decisions below resolve the Technical Context; no `NEEDS CLARIFICATION` items remain.

## D1 — Cryptographic crate selection

**Decision**: Use RustCrypto crates `hmac 0.12`, `sha1 0.10`, `md-5 0.10`, and `subtle 2.5` for MESSAGE-INTEGRITY and long-term key derivation; `crc32fast 1.4` for FINGERPRINT.

**Rationale**: RFC 5389 §15.4 fixes MESSAGE-INTEGRITY as HMAC-SHA1 and long-term keys as MD5 over the credential triple. These crates are widely used, pure Rust, and small. `subtle` supplies constant-time comparison so MESSAGE-INTEGRITY checks do not leak timing information. `crc32fast` implements the IEEE CRC-32 used by RFC 5389 §15.5 and is SIMD-accelerated.

**Alternatives considered**: `ring` (larger surface, SHA-1/MD5 availability varies), `sha2` (wrong digest family), hand-written HMAC (unnecessary risk). Rejected.

## D2 — SASLprep implementation

**Decision**: Use the `stringprep 0.1` crate's SASLprep profile (RFC 4013, Unicode 3.2) for long-term passwords.

**Rationale**: The clarified requirement is full SASLprep: mapping, normalization, prohibited-output, and bidi checks. Implementing the Unicode 3.2 tables by hand is error-prone and large. The `stringprep` crate exposes the SASLprep profile and keeps the implementation auditable.

**Alternatives considered**: ASCII-only passwords (rejected by clarification), raw UTF-8 (non-conformant), vendoring tables (maintenance burden). Implementation must confirm the crate's `saslprep` API at the first test; if the profile is unavailable, use the dedicated `saslprep` crate rather than weakening the requirement.

## D3 — Stateless NONCE construction

**Decision**: Issue a NONCE as a 24-byte value: 8-byte big-endian UNIX seconds followed by 16 bytes of HMAC-SHA1(server_secret, timestamp)[0..16], rendered as 48 lowercase hex characters. Validate by recomputing the MAC and checking `now - issued_at <= nonce_lifetime_sec`.

**Rationale**: UDP request handling stays stateless and allocation-free; no per-client nonce table or cleanup task is needed. A forged or expired nonce fails validation and produces 438. The server secret is 32 random bytes generated at startup with `OsRng`; restarting the server invalidates outstanding nonces, which clients recover from via the normal 438 retry.

**Alternatives considered**: stateful random-nonce map (memory growth, expiry sweeps), plain timestamps (forgeable), encrypted nonce (unnecessary complexity). Rejected.

**Accepted limitation**: Nonces are not portable across server restarts or across instances behind a load balancer. The deployment is a single stateless process, so this is acceptable and documented.

## D4 — Credential storage and configuration model

**Decision**: Add an `[auth]` TOML section with `mode` (`disabled` default, `short-term`, `long-term`), `realm`, `nonce_lifetime_sec` (default 600), and an array of `[[auth.users]]` entries with `username` and `password`. Precompute each user's key once at startup: the raw password for short-term, MD5(SASLprep(password)) over the credential triple for long-term.

**Rationale**: One credential list serves both mechanisms; the mode selects how the key is derived and which error flows apply. Storing passwords rather than derived keys keeps configuration understandable and lets the same file switch modes. Precomputing keys removes MD5/SASLprep from the request path.

**Alternatives considered**: separate sections per mechanism (duplication), environment variables (harder to test and document), hashed-at-rest secrets (impossible: the server needs the key material to verify HMAC). Rejected.

**Accepted limitation**: Secrets are plaintext in the TOML file; the sample configuration and README must instruct `chmod 600` and discourage committing real secrets.

## D5 — Attribute registry and wire positions

**Decision**: Extend `StunAttributeType` with USERNAME `0x0006`, MESSAGE-INTEGRITY `0x0008`, REALM `0x0014`, NONCE `0x0015`, ALTERNATE-SERVER `0x8023`, and FINGERPRINT `0x8028`. Locate MESSAGE-INTEGRITY and FINGERPRINT by iterating decoded attributes and summing `encoded_size()` from offset 20; do not add offset fields to `StunAttribute`.

**Rationale**: `encoded_size()` already accounts for wire padding, and decoded messages preserve the padding length, so the scan reproduces exact offsets without changing the public struct. Attributes after MESSAGE-INTEGRITY remain decoded but are ignored for classification and authentication.

**Alternatives considered**: adding `offset` to `StunAttribute` (public struct churn, no benefit), re-parsing raw bytes in every module (duplication). Rejected.

## D6 — Integrity-aware encoding

**Decision**: Add protocol helpers that build the byte range for HMAC/CRC computation by writing a header whose length field covers the attribute being authenticated, then concatenating encoded attributes up to that attribute. Response signing appends MESSAGE-INTEGRITY as the last attribute; no FINGERPRINT is emitted.

**Rationale**: RFC 5389 §15.4 and §15.5 compute the digest over the message with the length field set to include the digest attribute but with the digest bytes omitted. A single helper avoids duplicating this subtle rule between verification and response signing. MESSAGE-INTEGRITY must precede FINGERPRINT if FINGERPRINT is ever added; the server does not add FINGERPRINT, so MESSAGE-INTEGRITY stays last.

**Alternatives considered**: mutating the encoded buffer in place (easy to get the length wrong, harder to test), relying on `StunMessage::encode()` (cannot omit trailing attributes). Rejected.

## D7 — Authentication decision order

**Decision**: Process in this order: (1) header decode and Binding Request check, (2) FINGERPRINT structure and CRC, (3) MESSAGE-INTEGRITY length and position, (4) authentication per configured mode, (5) unknown-comprehension-required classification over attributes before MESSAGE-INTEGRITY, (6) success or 420 response signing.

**Rationale**: RFC 5389 §7.3 gives authentication priority over attribute handling, and §15.4 says attributes after MESSAGE-INTEGRITY are ignored. Short-circuiting on authentication failure prevents 420 from leaking information about a request that was never authenticated. FINGERPRINT is validated first because a damaged datagram must be discarded silently.

**Alternatives considered**: classifying unknown attributes before auth (wrong error precedence), validating auth after building a success response (wasted work and wrong response). Rejected.

## D8 — Short-term and long-term response rules

**Decision**: Short-term: missing USERNAME or MESSAGE-INTEGRITY → 400; unknown user or bad MAC → 401. Long-term: missing MESSAGE-INTEGRITY → 401 with REALM + NONCE; MESSAGE-INTEGRITY present but USERNAME/REALM/NONCE missing → 400; stale nonce → 438 with fresh NONCE + REALM; unknown user, realm mismatch, or bad MAC → 401 with REALM + NONCE. Authenticated success and 420 responses carry MESSAGE-INTEGRITY; all 400/401/438 responses omit MESSAGE-INTEGRITY and USERNAME.

**Rationale**: These are the exact RFC 5389 §10.1.2 and §10.2.2 branches. Responses to authenticated requests must be integrity-protected so clients can trust them (FR-028, FR-030).

**Alternatives considered**: challenge-first long-term for every request (adds a round trip beyond the RFC flow), 401 for missing attributes (contradicts §10). Rejected.

## D9 — Client authentication state machine

**Decision**: Add explicit `--auth-mode` (`disabled` default, `short-term`, `long-term`), `--username`, and `--password`. Short-term signs the first request. Long-term sends an unauthenticated first request, consumes REALM/NONCE from 401 or 438, retries at most twice, and validates MESSAGE-INTEGRITY on the response. Without credentials, 401/438 becomes a structured `ServerError` with no retry.

**Rationale**: The clarification selected full client authentication. An explicit mode mirrors the server and avoids guessing from attribute presence. The retry bound prevents loops against a misconfigured server.

**Alternatives considered**: infer mode from flags (ambiguous), unlimited retry (loop risk), no client auth (rejected by clarification). Rejected.

## D10 — Randomness source

**Decision**: Use `rand::rngs::OsRng` to fill each 96-bit client transaction ID and the 32-byte server nonce secret.

**Rationale**: RFC 5389 §6 requires uniform randomness and recommends cryptographic randomness. `OsRng` satisfies both without introducing a seedable RNG in production. The 10,000-ID uniqueness test exercises the generator.

**Alternatives considered**: clock-derived IDs (current defect), UUID crate (overkill), `getrandom` directly (works, but `rand` also covers future test helpers). Rejected.

## D11 — SOFTWARE value

**Decision**: Emit `SOFTWARE = "stunserver <CARGO_PKG_VERSION>"` (for example `stunserver 0.1.1`) from the server crate. Order attributes as XOR-MAPPED-ADDRESS, SOFTWARE, MESSAGE-INTEGRITY for success and ERROR-CODE, UNKNOWN-ATTRIBUTES, SOFTWARE, MESSAGE-INTEGRITY for 420.

**Rationale**: The value is stable, valid UTF-8, shorter than 128 characters, and identifies the implementation and version. Placing SOFTWARE before MESSAGE-INTEGRITY keeps the signature over the full response body.

**Alternatives considered**: include FINGERPRINT (optional and adds cost), omit version (weaker diagnostics). Rejected.

## D12 — Conformance matrix format

**Decision**: Add `docs/rfc-conformance.md` with one row per in-scope clause: RFC, section, normative level, requirement summary, status (`conformant`, `gap`, `out-of-scope`), evidence (test name and path), and notes. Link it from the README.

**Rationale**: FR-045 and SC-002 require 100% classification with evidence. A Markdown table is reviewable in pull requests and has no runtime cost.

**Alternatives considered**: inline comments in source (hard to audit), generated report (extra tooling, deferred). Rejected.

## D13 — Performance approach

**Decision**: Hash only when the corresponding attribute is present; in enabled auth modes, verify MESSAGE-INTEGRITY with precomputed keys; use stack buffers for nonce construction; keep the existing per-request task model. Extend `stunbench` with a `--fingerprint` flag so the benchmark can exercise the FINGERPRINT validation path, then record p99 before and after (SC-008).

**Rationale**: HMAC-SHA1 and CRC-32 are cheap relative to UDP I/O, but per-request MD5/SASLprep would not be. The benchmark must send FINGERPRINT-bearing traffic to measure the validation cost; the existing plain-request mode cannot demonstrate SC-008.

**Alternatives considered**: caching authenticated client addresses (stateful, security risk), skipping FINGERPRINT validation (RFC violation). Rejected.
