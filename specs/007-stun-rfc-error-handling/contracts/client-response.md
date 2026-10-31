# Contract: Client Response Handling

**Feature**: `007-stun-rfc-error-handling` | **Date**: 2026-10-31
**Consumer**: operators using `stunclient`; tests in `src/client/tests/client.rs`

## C1 — Inputs

`send_binding_request(server_addr, timeout)` sends a Binding Request with a random transaction ID and accepts one datagram only from `server_addr`.

## C2 — Outcome contract

| Server reply | Preconditions | Client outcome |
|--------------|---------------|----------------|
| `0x0101` success | txid matches; exactly one valid XOR-MAPPED-ADDRESS; no unrecognized required attributes | `Ok(BindingResponse { mapped_address, latency })` |
| `0x0101` success | unrecognized attribute with `raw_type < 0x8000` | `Err(ClientError::Protocol(..))` naming the attribute |
| `0x0101` success | unrecognized attribute with `raw_type >= 0x8000` | ignored → `Ok(...)` |
| `0x0101` success | missing XOR-MAPPED-ADDRESS | `Err(Protocol(..))` |
| `0x0111` error | txid matches; ERROR-CODE present | `Err(ClientError::ServerError { class, number, reason, unknown_attributes })` |
| `0x0111` error | ERROR-CODE absent or value shorter than 4 bytes | `Err(Protocol(..))` |
| `0x0111` error | number 420 with UNKNOWN-ATTRIBUTES | `ServerError` with the parsed type list |
| `0x0111` error | number 420 without UNKNOWN-ATTRIBUTES | `ServerError` with an empty list |
| `0x0111` error | txid mismatch | `Err(Protocol("transaction ID mismatch"))` |
| other message type | — | `Err(Protocol(..))` |
| undecodable / truncated | — | `Err(Protocol(..))` |
| no reply within timeout | — | `Err(Timeout(secs))` |

Address family handling: IPv4 XOR value must be 8 bytes and IPv6 20 bytes; other combinations are `Protocol` errors. Source-address mismatch remains a `Protocol` error (existing behavior).

## C3 — Error payload

```text
ClientError::ServerError {
    class: u8,              // e.g. 4
    number: u8,             // e.g. 20
    reason: String,         // "Unknown Attribute"
    unknown_attributes: Vec<u16>, // e.g. [0x0024]
}
```

Display for 420: `server returned error 420 (Unknown Attribute); unknown attributes: 0x0024`.

- `unknown_attributes` is empty when the reply is not 420 or omits the attribute.
- Values are formatted as uppercase four-digit hex.

## C4 — Scenario matrix (tests)

| ID | Scenario | Expected |
|----|----------|----------|
| CR-01 | IPv4 success via mock UDP server | `Ok`, mapped port matches client port |
| CR-02 | IPv6 success via mock UDP server | `Ok`, loopback IPv6 address |
| CR-03 | 420 with UNKNOWN-ATTRIBUTES `[0x0024]` | `ServerError { class: 4, number: 20, unknown_attributes: [0x0024] }` |
| CR-04 | 420 without UNKNOWN-ATTRIBUTES | `ServerError { unknown_attributes: [] }` |
| CR-05 | success + unknown optional `0x80FF` | `Ok` |
| CR-06 | success + unknown required `0x0024` | `Protocol` naming `0x0024` |
| CR-07 | success with wrong transaction ID | `Protocol` |
| CR-08 | malformed/short datagram | `Protocol` |
| CR-09 | IPv4 family with 20-byte value | `Protocol` |
| CR-10 | no reply | `Timeout` |

## C5 — Non-goals

- Credentialed requests (USERNAME/MESSAGE-INTEGRITY) are not sent.
- MESSAGE-INTEGRITY/FINGERPRINT are not cryptographically verified.
- Retransmission logic is unchanged.
