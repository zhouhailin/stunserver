# Quickstart: Validate STUN RFC Error Handling

**Feature**: `007-stun-rfc-error-handling` | **Date**: 2026-10-31

## Prerequisites

- Repository at `/opt/stunserver` on branch `007-stun-rfc-error-handling`
- Rust stable toolchain with `cargo`, `rustfmt`, `clippy`
- Loopback UDP access (no privileged port needed)

## 1. Protocol unit and vector tests

```bash
cargo test -p stun-protocol
```

Expected:

- `rfc5769_vectors` passes all four published vectors with byte-identical re-encoding.
- Registry tests report `0x0003` CHANGE-REQUEST, `0x0026` PADDING, `0x0027` RESPONSE-PORT, `0x8027` CACHE-TIMEOUT, `0x802B` RESPONSE-ORIGIN, `0x802C` OTHER-ADDRESS, and no `0x802D` OTHER-ADDRESS mapping.
- `ErrorCode`/`UnknownAttributes` encode/decode tests pass, including 420 round trip.

## 2. Server rejection tests (real UDP loopback)

```bash
cargo test -p stunserver
```

Expected cases in `src/server/tests/integration.rs`:

| Test intent | Request | Expected reply |
|-------------|---------|----------------|
| Unknown required | PRIORITY `0x0024` | `0x0111`, ERROR-CODE 420, UNKNOWN-ATTRIBUTES `[0x0024]` |
| Duplicate required | `0x0024` twice | list contains `0x0024` once |
| Optional only | `0x80FF` | `0x0101` success |
| CHANGE-REQUEST | `0x0003` | 420 with `[0x0003]` |
| PADDING | `0x0026` | 420 with `[0x0026]` |
| RESPONSE-PORT | `0x0027` | 420 with `[0x0027]` |
| Plain request | no attributes | `0x0101` with XOR-MAPPED-ADDRESS |

## 3. Client response scenarios

```bash
cargo test -p stunclient
```

Expected: CR-01 through CR-10 from [contracts/client-response.md](contracts/client-response.md) pass, including structured 420 reporting and rejection of unknown required attributes.

## 4. Manual end-to-end probe (optional)

Start the server on a loopback port:

```bash
cargo run -p stunserver -- --listen 127.0.0.1:34780 --debug
```

In another shell, send a Binding Request carrying PRIORITY (`0x0024`) and print the reply summary. Any UDP client that can emit 12 bytes works; a short Python probe is sufficient:

```bash
python3 - <<'PY'
import socket
req = bytes.fromhex(
    "000100082112a442" + "0102030405060708090a0b0c" +
    "00240004" + "6e0001ff"
)
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.settimeout(2)
s.sendto(req, ("127.0.0.1", 34780))
data, _ = s.recvfrom(2048)
print(data.hex())
PY
```

Expected: reply starts with `0111` (Binding Error Response) and contains attribute `0009` (ERROR-CODE with `00000414` + "Unknown Attribute") and `000a` (UNKNOWN-ATTRIBUTES with `0024`). The server's DEBUG log shows:

```text
Response: Binding Error Response txn=... attrs=[ERROR-CODE=420 "Unknown Attribute", UNKNOWN-ATTRIBUTES=[0x0024]]
```

## 5. Full quality gate (CI parity)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

All three must pass before the feature is considered complete.
