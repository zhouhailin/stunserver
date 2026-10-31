# Contract: STUN Wire Behavior

**Feature**: `008-rfc-compliance-audit` | **Scope**: UDP Binding requests and responses

This contract is the normative behavior the implementation must satisfy. RFC 5389 section references prevail if this document and the RFC disagree.

## 1. Message header

| Field | Requirement |
|-------|-------------|
| Leading two bits | MUST be zero; otherwise silently discard |
| Message type | MUST be Binding Request `0x0001` for requests; other classes/methods are silently discarded |
| Message length | MUST be a multiple of four and MUST NOT exceed the received datagram; otherwise silently discard |
| Magic cookie | MUST equal `0x2112A442`; otherwise silently discard |
| Transaction ID | Echoed unchanged in every response |

## 2. Request intake order

1. Decode header and attributes. Any truncation, bad padding, bad attribute length, or malformed value causes silent discard.
2. Require a Binding Request. Indications, success responses, error responses, and unsupported methods are silently discarded.
3. Validate FINGERPRINT when present:
   - It MUST be the final attribute and appear exactly once.
   - Its value MUST be exactly four bytes.
   - Its value MUST equal the CRC-32 of the message up to the FINGERPRINT attribute, with the header length field set to the final message length, XOR `0x5354554E`.
   - Any violation causes silent discard with no response.
4. Locate MESSAGE-INTEGRITY:
   - Its value MUST be exactly 20 bytes (already enforced by the attribute decoder).
   - It MUST appear at most once; a duplicate is silently discarded.
   - Attributes after the first MESSAGE-INTEGRITY, except FINGERPRINT, are ignored for authentication and classification.
5. Run the authentication stage for the configured mode.
6. Classify unknown comprehension-required attributes from the attributes before MESSAGE-INTEGRITY.
7. Build a success or 420 response; sign it with MESSAGE-INTEGRITY when the request authenticated.

## 3. Attribute classification

| Attribute class | Examples | Behavior |
|-----------------|----------|----------|
| Known and processed | USERNAME, MESSAGE-INTEGRITY, REALM, NONCE, FINGERPRINT | Handled by intake/auth; not listed as unknown |
| Known, not acted upon | MAPPED-ADDRESS, ERROR-CODE, UNKNOWN-ATTRIBUTES, ALTERNATE-SERVER | Ignored per RFC 5389 §7.3; does not cause 420 |
| RFC 5780 rejected by this server | CHANGE-REQUEST `0x0003`, PADDING `0x0026`, RESPONSE-PORT `0x0027` | 420, listed in UNKNOWN-ATTRIBUTES |
| Unrecognized comprehension-required | `0x0000`–`0x7FFF` not otherwise known | 420, listed once in first-seen order |
| Unrecognized comprehension-optional | `0x8000`–`0xFFFF` | Ignored; never listed |

When authentication is disabled, USERNAME, MESSAGE-INTEGRITY, REALM, and NONCE are ignored and do not cause 420.

## 4. Authentication flows

### 4.1 Short-term credentials

| Request state | Response | Body | MESSAGE-INTEGRITY in response |
|---------------|----------|------|-------------------------------|
| USERNAME + valid MESSAGE-INTEGRITY | Success or 420 | Normal body | Present, same key |
| USERNAME or MESSAGE-INTEGRITY missing | 400 | ERROR-CODE, SOFTWARE | Absent |
| Unknown USERNAME | 401 | ERROR-CODE, SOFTWARE | Absent |
| MESSAGE-INTEGRITY mismatch | 401 | ERROR-CODE, SOFTWARE | Absent |

Key: the configured password bytes for that username. USERNAME is not echoed.

### 4.2 Long-term credentials

| Request state | Response | Additional attributes | MESSAGE-INTEGRITY in response |
|---------------|----------|-----------------------|-------------------------------|
| MESSAGE-INTEGRITY missing | 401 | REALM, NONCE | Absent |
| MESSAGE-INTEGRITY present, USERNAME/REALM/NONCE missing | 400 | none | Absent |
| NONCE stale or invalid | 438 | fresh NONCE, REALM | Absent |
| Unknown USERNAME, REALM mismatch, or MAC mismatch | 401 | REALM, NONCE | Absent |
| Valid credentials | Success or 420 | Normal body | Present, same derived key |

Key derivation: `MD5(username ":" realm ":" SASLprep(password))`, where SASLprep is the full RFC 4013 profile (Unicode 3.2). The success response carries MESSAGE-INTEGRITY and omits USERNAME, REALM, and NONCE. A 401 or 438 carries REALM and NONCE and must not carry MESSAGE-INTEGRITY or USERNAME.

### 4.3 Precedence

Authentication runs before unknown-attribute classification. A request that fails authentication receives its 400/401/438 response even if it also contains unknown comprehension-required attributes.

## 5. Response attribute sets

| Response | Attributes in order |
|----------|---------------------|
| Success, unauthenticated | XOR-MAPPED-ADDRESS, SOFTWARE |
| Success, authenticated | XOR-MAPPED-ADDRESS, SOFTWARE, MESSAGE-INTEGRITY |
| 420, unauthenticated | ERROR-CODE (4/20), UNKNOWN-ATTRIBUTES, SOFTWARE |
| 420, authenticated | ERROR-CODE (4/20), UNKNOWN-ATTRIBUTES, SOFTWARE, MESSAGE-INTEGRITY |
| 400 | ERROR-CODE (4/00), SOFTWARE |
| 401 short-term | ERROR-CODE (4/01), SOFTWARE |
| 401 long-term | ERROR-CODE (4/01), REALM, NONCE, SOFTWARE |
| 438 long-term | ERROR-CODE (4/38), REALM, NONCE, SOFTWARE |

Rules:

- SOFTWARE is `stunserver <version>`, valid UTF-8, shorter than 128 characters, and identical across responses.
- XOR-MAPPED-ADDRESS reflects the UDP source address and port.
- OTHER-ADDRESS and RESPONSE-ORIGIN are never emitted.
- No FINGERPRINT is emitted by the server.
- MESSAGE-INTEGRITY is always the final attribute when present.
- Responses are sent from the local address and port that received the request, to the request's source address.

## 6. Integrity algorithms

### 6.1 MESSAGE-INTEGRITY

- Key: configured credential key (short-term) or MD5-derived key (long-term).
- Digest: HMAC-SHA1 over the message bytes preceding the MESSAGE-INTEGRITY attribute, with the header length field set to include the MESSAGE-INTEGRITY attribute (4-byte header + 20-byte value).
- Verification uses a constant-time comparison.
- Responses to authenticated requests are signed with the same key used to verify the request.

### 6.2 FINGERPRINT

- CRC-32 (IEEE, as used by zlib/Ethernet) over the message bytes preceding the FINGERPRINT attribute, with the header length field set to the final message length including FINGERPRINT.
- The 32-bit result is XORed with `0x5354554E`.
- A mismatch, duplicate, non-final position, or length other than four bytes causes silent discard on the server and a protocol error on the client.

## 7. RFC 5780 behavior

This is a single-address server without NAT behavior discovery. CHANGE-REQUEST, PADDING, and RESPONSE-PORT produce 420. Documentation and deployment configuration must not advertise `stun-behavior` or `stun-behaviors`.

## 8. Logging contract

- Message summaries show every recognized attribute by RFC name and unknown types as `0xNNNN`.
- MESSAGE-INTEGRITY and FINGERPRINT are summarized by length only.
- Passwords, derived keys, and the server nonce secret never appear in logs, summaries, or error text.
- USERNAME may appear; it is not a secret.
