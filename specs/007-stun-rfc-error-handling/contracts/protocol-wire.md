# Contract: STUN Wire Behavior

**Feature**: `007-stun-rfc-error-handling` | **Date**: 2026-10-31
**Consumers**: `stunserver`, `stunclient`, third-party STUN clients, `stun-protocol` tests

This contract defines observable wire behavior. Field-level types are in [../data-model.md](../data-model.md).

## C1 — Attribute type registry

| Code | Name | Range |
|------|------|-------|
| `0x0001` | MAPPED-ADDRESS | required |
| `0x0003` | CHANGE-REQUEST | required |
| `0x0009` | ERROR-CODE | required |
| `0x0020` | XOR-MAPPED-ADDRESS | required |
| `0x0026` | PADDING | required |
| `0x0027` | RESPONSE-PORT | required |
| `0x8022` | SOFTWARE | optional |
| `0x8027` | CACHE-TIMEOUT | optional |
| `0x802B` | RESPONSE-ORIGIN | optional |
| `0x802C` | OTHER-ADDRESS | optional |

- `0x802D` MUST NOT be reported as OTHER-ADDRESS.
- `0x802B` MUST NOT be reported as SOURCE-ADDRESS and `0x802C` MUST NOT be reported as CHANGED-ADDRESS.
- Required range: `0x0000–0x7FFF`; optional range: `0x8000–0xFFFF`.

## C2 — Binding request processing

Given a datagram that decodes as a Binding Request (`0x0001`):

1. Collect every attribute with `raw_type < 0x8000` that the server does not implement. The server implements none, so this is every required-range attribute.
2. De-duplicate by type, preserving first-seen order.
3. If the collection is empty → respond with `0x0101` and XOR-MAPPED-ADDRESS (existing behavior).
4. If non-empty → respond with `0x0111` (Binding Error Response) containing:
   - ERROR-CODE: class `4`, number `20`, reason `Unknown Attribute`;
   - UNKNOWN-ATTRIBUTES: the collected type list;
   - the request's transaction ID and magic cookie;
   - no XOR-MAPPED-ADDRESS.
5. Attributes with `raw_type >= 0x8000` are ignored and never listed.

Non-request or undecodable datagrams produce no response (unchanged).

### C2.1 — Observable examples

| Request attributes | Reply |
|--------------------|-------|
| none, or SOFTWARE only | `0x0101`, XOR-MAPPED-ADDRESS |
| PRIORITY `0x0024` | `0x0111`, ERROR-CODE 420, UNKNOWN-ATTRIBUTES `[0x0024]` |
| CHANGE-REQUEST `0x0003` | `0x0111`, ERROR-CODE 420, UNKNOWN-ATTRIBUTES `[0x0003]` |
| PADDING `0x0026` | `0x0111`, ERROR-CODE 420, UNKNOWN-ATTRIBUTES `[0x0026]` |
| RESPONSE-PORT `0x0027` | `0x0111`, ERROR-CODE 420, UNKNOWN-ATTRIBUTES `[0x0027]` |
| `0x0024` twice + `0x0025` | `0x0111`, UNKNOWN-ATTRIBUTES `[0x0024, 0x0025]` |
| `0x80FF` only | `0x0101`, XOR-MAPPED-ADDRESS |
| `0x0024` + `0x80FF` | `0x0111`, UNKNOWN-ATTRIBUTES `[0x0024]` |

## C3 — ERROR-CODE value format

Value = `00 00 CC NN <reason UTF-8>` where `CC` holds the 3-bit class and `NN` the 8-bit number. For 420: `00 00 04 14` followed by `Unknown Attribute` (17 bytes), padded to 24 value bytes.

Attribute header length field excludes padding.

## C4 — UNKNOWN-ATTRIBUTES value format

Value = big-endian 16-bit types, padded to a 4-byte boundary. `[0x0003]` encodes as `00 03 00 00` (4 value bytes); `[0x0024, 0x0025]` encodes as `00 24 00 25`.

The attribute MUST be present in 420 responses and absent otherwise.

## C5 — XOR-MAPPED-ADDRESS

- IPv4 value is exactly 8 bytes: `00 01 <port^0x2112> <ipv4^0x2112A442>`.
- IPv6 value is exactly 20 bytes: `00 02 <port^0x2112> <ipv6^cookie||txid>`.
- Reserved first byte is ignored on receive.
- Any other family/length combination is a malformed message (client rejects; tests must assert).

## C6 — Attribute padding

- Padding length = `(4 - value_length % 4) % 4`.
- Padding bytes may be any value on the wire and MUST be preserved when a message is decoded and re-encoded.
- Newly built attributes emit zero padding.

## C7 — RFC 5769 vector expectations

| Section | Bytes | Type | Transaction ID | Attributes (order) | Decoded address |
|---------|-------|------|----------------|--------------------|-----------------|
| §2.1 | 108 | `0x0001` | `b7e7a701bc34d686fa87dfae` | SOFTWARE(16), PRIORITY(4), ICE-CONTROLLED(8), USERNAME(9, pad `20 20 20`), MESSAGE-INTEGRITY(20), FINGERPRINT(4) | — |
| §2.2 | 80 | `0x0101` | same | SOFTWARE(11), XOR-MAPPED-ADDRESS(8), MESSAGE-INTEGRITY(20), FINGERPRINT(4) | `192.0.2.1:32853` |
| §2.3 | 92 | `0x0101` | same | SOFTWARE(11), XOR-MAPPED-ADDRESS(20), MESSAGE-INTEGRITY(20), FINGERPRINT(4) | `[2001:db8:1234:5678:11:2233:4455:6677]:32853` |
| §2.4 | 116 | `0x0001` | `78ad3433c6ad72c029da412e` | USERNAME(18), NONCE(28), REALM(11), MESSAGE-INTEGRITY(20), FINGERPRINT(4) | — |

- Attribute lengths shown are value lengths, not including padding.
- MESSAGE-INTEGRITY and FINGERPRINT values are opaque and preserved exactly.
- Re-encoding each decoded vector MUST equal the input bytes exactly.

## C8 — No alternate address

- Success and error responses never include OTHER-ADDRESS.
- The server does not advertise `stun-behavior` or `stun-behaviors` service names.

## C9 — Logging summary (observable)

- `Binding Error Response txn=<hex> attrs=[ERROR-CODE=420 "Unknown Attribute", UNKNOWN-ATTRIBUTES=[0x0024]]`
- `Binding Success Response txn=<hex> attrs=[XOR-MAPPED-ADDRESS=192.0.2.1:32853]`
- New attribute names appear as RESPONSE-ORIGIN, OTHER-ADDRESS, CACHE-TIMEOUT, CHANGE-REQUEST, PADDING, RESPONSE-PORT.
