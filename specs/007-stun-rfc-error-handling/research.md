# Research: STUN RFC Error Handling & Attribute Interoperability

**Feature**: `007-stun-rfc-error-handling` | **Date**: 2026-10-31

## Decision 1 — Preserve attribute padding bytes

**Decision**: `StunAttribute` stores the padding bytes read from the wire and replays them on encode; newly constructed attributes emit zero padding.

**Rationale**: RFC 5389 §15 states padding bits "are ignored, and may be any value." RFC 5769 §2.1 pads the 9-byte USERNAME with three `0x20` bytes, so a decoder that regenerates zero padding cannot reproduce the published vector byte-for-byte. Preserving padding satisfies FR-011 without weakening parsing.

**Alternatives considered**:
- Compare vectors after normalizing padding: rejected because FR-011 requires byte-identical re-encoding and normalized comparisons hide encoder regressions.
- Force zero padding and adjust the spec: rejected because it would make the project unable to round-trip valid third-party messages.

**Implementation note**: `decoded_size` stays derived from `attribute_length`; decode must also verify the pad bytes are present, and message decoding must finish exactly at the declared length.

## Decision 2 — Supported set vs. recognized set

**Decision**: Recognition (naming in logs) and support (acceptance) are separate. The registry recognizes the RFC 5780 codes; the server supports no comprehension-required request attributes, so every `raw_type < 0x8000` attribute triggers 420.

**Rationale**: A Binding Request from a plain client carries no comprehension-required attributes. Attributes such as CHANGE-REQUEST, PADDING, RESPONSE-PORT, PRIORITY, USERNAME, or MESSAGE-INTEGRITY are not implemented here, and silently returning success would claim support. FR-007/FR-008 require explicit rejects.

**Alternatives considered**:
- Maintain a whitelist of "implemented" required attributes: identical behavior today (empty set) with extra code; rejected under YAGNI.
- Special-case only RFC 5780 attributes and ignore others: rejected because unknown required attributes must produce 420 per RFC 5389 §7.3.1.

**Documented deviation**: MESSAGE-INTEGRITY (`0x0008`) is comprehension-required. RFC 5389 §10.1.2 implies a server that cannot validate it should discard the request silently; this server returns 420 instead, because silent discard gives clients no diagnostic signal and credential support is out of scope. Interoperability impact: clients configured for authenticated STUN learn the server has no auth support immediately.

## Decision 3 — ERROR-CODE encoding

**Decision**: New `ErrorCode` type stores `class` (`u8`, 3 bits), `number` (`u8`), and `reason` (`String`). Value layout: bytes 0–1 zero, byte 2 = class, byte 3 = number, bytes 4.. = UTF-8 reason (no NUL terminator), padded by the generic attribute encoder.

**Rationale**: Directly follows RFC 5389 §15.6. The 420 response uses class 4, number 20, reason "Unknown Attribute". Decoding masks class with `0x07` and trims nothing from the reason (padding is handled at attribute level).

**Alternatives considered**: storing a combined 3-digit code — rejected because the wire splits class and number, and masking is explicit.

## Decision 4 — UNKNOWN-ATTRIBUTES encoding

**Decision**: New `UnknownAttributes` value type wraps `Vec<u16>`; encode writes big-endian types in first-seen order after de-duplication, and relies on the attribute encoder for zero padding; decode returns 16-bit types and rejects a trailing odd byte.

**Rationale**: RFC 5389 §15.9 defines a list of 16-bit values. De-duplication matches spec edge-case expectations and keeps the response minimal.

**Alternatives considered**: storing a fixed array — rejected because the count is unbounded up to the datagram size; `Vec` is simplest.

## Decision 5 — 420 response composition and ordering

**Decision**: Error responses contain exactly ERROR-CODE followed by UNKNOWN-ATTRIBUTES, echo the request transaction ID and magic cookie, and never include XOR-MAPPED-ADDRESS.

**Rationale**: RFC 5389 §7.3.1 requires the code and list; §15.9 requires the list only for 420. Ordering mirrors the RFC's description and makes summaries deterministic.

**Alternatives considered**: adding SOFTWARE to error responses — rejected as unnecessary scope.

## Decision 6 — PADDING + RESPONSE-PORT conflict

**Decision**: Because neither attribute is supported, the server returns 420 for both individually and for the combination, rather than 400.

**Rationale**: RFC 5780 §6.1 mandates 400 only for servers that implement PADDING and RESPONSE-PORT. This deployment implements neither, so the general unknown-attribute rule applies and is documented as an accepted deviation.

**Alternatives considered**: implement PADDING echo and RESPONSE-PORT to satisfy 400 — rejected as out of P0 scope.

## Decision 7 — Client error surface

**Decision**: Add `ClientError::ServerError { class: u8, number: u8, reason: String, unknown_attributes: Vec<u16> }` and parse `StunMessageType::ErrorIndication` replies inside `parse_response`. A 420 response yields `ServerError`; an error response without ERROR-CODE yields `Protocol`.

**Rationale**: Operators need the numeric code and attribute list; a generic "unexpected message type" hides actionable data. Keeping `Protocol` for malformed error responses preserves existing rejection semantics.

**Alternatives considered**: returning a new top-level result enum — rejected because it would churn every caller for no gain.

## Decision 8 — Comprehension checks in the client

**Decision**: In success responses, ignore attributes with `raw_type >= 0x8000` that the client does not recognize, and fail with `Protocol` when `raw_type < 0x8000` is unrecognized. The check uses `raw_type`, not the enum, so newly added registry entries cannot accidentally bypass it.

**Rationale**: Mirrors RFC 5389 §7.3.3/§7.3.4 and fixes the current client, which ignores unknown attributes entirely.

## Decision 9 — Strict XOR-MAPPED-ADDRESS length

**Decision**: `MappedAddress::decode_xor` accepts exactly 8 bytes for IPv4 and 20 bytes for IPv6; any other combination is a protocol/malformed error.

**Rationale**: RFC 5389 §15.2 fixes the value length by family. Current code accepts trailing bytes, which FR-015 forbids. The non-XOR `MappedAddress::decode` keeps its existing behavior to limit blast radius (RFC 5780 address attributes are not consumed by this feature).

## Decision 10 — RFC 5769 vector fixtures

**Decision**: Embed the exact byte arrays from RFC 5769 §2.1 (108 bytes), §2.2 (80 bytes), §2.3 (92 bytes), and §2.4 (116 bytes) as test constants, and assert both decoded fields and byte-identical re-encoding.

**Rationale**: Self-generated round trips cannot detect symmetric encoder/decoder bugs. Published vectors are the interoperability ground truth. Expected decoded values are documented in [contracts/protocol-wire.md](contracts/protocol-wire.md).

## Decision 11 — No new dependencies

**Decision**: Implement ERROR-CODE/UNKNOWN-ATTRIBUTES codecs with existing `byteorder`; do not add crypto/CRC crates.

**Rationale**: MESSAGE-INTEGRITY/FINGERPRINT are preserved as opaque bytes (FR-012). Adding crypto would expand scope beyond P0.

## Decision 12 — Logging format for new attributes

**Decision**: Summaries render `ERROR-CODE=<class><number> "<reason>"` and `UNKNOWN-ATTRIBUTES=[0x0003, 0x0024]`; RFC 5780 names appear verbatim. `Binding Error Response` remains the message-type label.

**Rationale**: FR-016 makes rejections diagnosable; deterministic uppercase hex matches the existing `0xABCD(len=4)` unknown rendering.
