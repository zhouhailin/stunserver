# Data Model: STUN RFC Error Handling & Attribute Interoperability

**Feature**: `007-stun-rfc-error-handling` | **Date**: 2026-10-31

## Attribute Registry

`StunAttributeType` (protocol crate) maps wire type codes to canonical names. Comprehension class is derived from the raw code: `0x0000–0x7FFF` = required, `0x8000–0xFFFF` = optional.

| Raw code | Variant | Canonical name | Class | Server support in requests |
|----------|---------|----------------|-------|----------------------------|
| `0x0001` | `MappedAddress` | MAPPED-ADDRESS | required | no |
| `0x0003` | `ChangeRequest` | CHANGE-REQUEST | required | no |
| `0x0009` | `ErrorCode` | ERROR-CODE | required | no (response only) |
| `0x0020` | `XorMappedAddress` | XOR-MAPPED-ADDRESS | required | no (response only) |
| `0x0026` | `Padding` | PADDING | required | no |
| `0x0027` | `ResponsePort` | RESPONSE-PORT | required | no |
| `0x8022` | `Software` | SOFTWARE | optional | ignored |
| `0x8027` | `CacheTimeout` | CACHE-TIMEOUT | optional | ignored |
| `0x802B` | `ResponseOrigin` | RESPONSE-ORIGIN | optional | ignored |
| `0x802C` | `OtherAddress` | OTHER-ADDRESS | optional | ignored |
| other required | `Unknown` | `0x{code}` | required | no → 420 |
| other optional | `Unknown` | `0x{code}` | optional | ignored |

**Validation rules**

- `from_raw` never fails; unrecognized codes become `Unknown` with `raw_type` preserved.
- No variant may map to `0x802D`; `SourceAddress`/`ChangedAddress` variants are removed.
- Legacy RFC 3489 SOURCE-ADDRESS (`0x0004`) and CHANGED-ADDRESS (`0x0005`) are deliberately unrecognized (they fall into "other required" → 420).
- `is_comprehension_required(raw) == raw < 0x8000`.

## StunAttribute

| Field | Type | Invariants |
|-------|------|-----------|
| `attribute_type` | `StunAttributeType` | Derived from `raw_type` at decode time |
| `raw_type` | `u16` | Exact wire value; drives comprehension checks |
| `attribute_length` | `u16` | Exact value length, excluding padding |
| `attribute_value` | `Vec<u8>` | Length equals `attribute_length` |
| `padding` | `Vec<u8>` | Length `(4 - length % 4) % 4`; preserved from decode, empty for new attributes |

**Validation rules**

- Decode requires the 4-byte header, `attribute_length` value bytes, and the full pad; a truncated pad is `MalformedMessage`.
- Encode writes stored padding when its length matches the required pad length; otherwise zero padding.
- Message decode must land exactly on the declared end offset after the last attribute; overshoot is `MalformedMessage`.

## ErrorCode

| Field | Type | Constraints |
|-------|------|-------------|
| `class` | `u8` | 0–7 (wire uses 3 bits) |
| `number` | `u8` | 0–255 |
| `reason` | `String` | UTF-8, may be empty; padded by attribute encoding |

**Encoding layout** (value bytes): `[0x00, 0x00, class, number, reason…]`, total length `4 + reason.len()`.

**Decoding rules**

- Value shorter than 4 bytes → `MalformedMessage`.
- `class = value[2] & 0x07`, `number = value[3]`.
- Reason = `value[4..]` as lossy UTF-8 (padding is not part of the value, so no trimming needed).

**Canonical 420 instance**: class `4`, number `20`, reason `"Unknown Attribute"`.

## UnknownAttributes

| Field | Type | Constraints |
|-------|------|-------------|
| `types` | `Vec<u16>` | Unique values, first-seen order |

**Encoding**: big-endian `u16` sequence; the generic attribute encoder pads to 4 bytes.

**Decoding**: value length must be even → otherwise `MalformedMessage`; returns the types in wire order (duplicates allowed on input, de-duplicated only when the server builds the list).

## Request Classification (server)

```text
Decoded request
  ├─ not a Binding Request          → no response (existing behavior)
  ├─ collect raw_type < 0x8000      → unsupported list (dedup, first-seen)
  │    ├─ empty                     → Binding Success Response (XOR-MAPPED-ADDRESS)
  │    └─ non-empty                 → Binding Error Response
  │           ERROR-CODE(4, 20, "Unknown Attribute")
  │           UNKNOWN-ATTRIBUTES(list)
  └─ raw_type >= 0x8000             → ignored, does not affect outcome
```

**Invariants**

- Error response message type is `0x0111`; success is `0x0101`.
- Transaction ID and magic cookie are copied from the request.
- Error responses contain no XOR-MAPPED-ADDRESS.
- Success responses contain no OTHER-ADDRESS (no alternate address exists).

## Client Outcome

`parse_response` produces one of:

| Outcome | Trigger | Payload |
|---------|---------|---------|
| Success | `0x0101` with a valid XOR-MAPPED-ADDRESS and no unrecognized required attributes | `BindingResponse { mapped_address, latency }` |
| `ClientError::ServerError` | `0x0111` with parseable ERROR-CODE | class, number, reason, unknown attribute list (empty if absent) |
| `ClientError::Protocol` | wrong message type, txid mismatch, malformed/truncated data, missing XOR-MAPPED-ADDRESS, unrecognized required attribute in success, malformed address family/length | message |
| `ClientError::Timeout` / `Connection` | unchanged transport failures | — |

**Validation rules**

- Error responses must echo the expected transaction ID before the error is surfaced.
- UNKNOWN-ATTRIBUTES is only meaningful for number 420; it is parsed when present and otherwise empty.
- Unknown optional attributes are ignored.
