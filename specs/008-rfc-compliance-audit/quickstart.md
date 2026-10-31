# Quickstart: Validate RFC 5389/5769/5780 Conformance

**Feature**: `008-rfc-compliance-audit` | **Date**: 2026-10-31

This guide proves the feature works end to end. It assumes a Linux shell at the repository root.

## Prerequisites

- Rust stable toolchain with Cargo
- No external network service required; all scenarios use localhost
- `chmod` for protecting the temporary credential file

## 1. Build and run the full automated suite

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```

Expected: all commands exit 0. Protocol tests include `rfc5769_vectors` (HMAC-SHA1 and CRC-32 assertions), `authentication` (short- and long-term flows), and server/client integration tests.

## 2. Verify the RFC 5769 vectors

```bash
cargo test -p stun-protocol rfc5769 -- --nocapture
```

Expected: the four published vectors decode to their documented values, MESSAGE-INTEGRITY and FINGERPRINT match the published bytes, and re-encoding is byte-identical.

## 3. Run a short-term authenticated exchange

Create a temporary configuration:

```bash
cat > /tmp/stun-short.conf <<'EOF'
[server]
listen_address = "127.0.0.1"
port = 3478

[auth]
mode = "short-term"
realm = "stunserver"

[[auth.users]]
username = "alice"
password = "correct-horse"
EOF
chmod 600 /tmp/stun-short.conf
```

Start the server in one terminal:

```bash
cargo run -p stunserver -- --config /tmp/stun-short.conf
```

In a second terminal, send an authenticated request:

```bash
cargo run -p stunclient -- 127.0.0.1:3478 \
  --auth-mode short-term --username alice --password correct-horse
```

Expected: `Mapped Address: 127.0.0.1:<port>` and exit code 0.

Negative checks:

```bash
# Wrong password: exit code 5, ERROR-CODE 401, no MESSAGE-INTEGRITY in the response
cargo run -p stunclient -- 127.0.0.1:3478 \
  --auth-mode short-term --username alice --password wrong

# No credentials: exit code 5, ERROR-CODE 400 for missing USERNAME/MESSAGE-INTEGRITY
cargo run -p stunclient -- 127.0.0.1:3478
```

## 4. Run a long-term authenticated exchange

Stop the server, change `mode` to `long-term` in `/tmp/stun-short.conf` (or create a new file), and restart:

```bash
sed -i 's/mode = "short-term"/mode = "long-term"/' /tmp/stun-short.conf
cargo run -p stunserver -- --config /tmp/stun-short.conf
```

Then:

```bash
cargo run -p stunclient -- 127.0.0.1:3478 \
  --auth-mode long-term --username alice --password correct-horse --debug
```

Expected: the debug log shows the initial 401 challenge followed by a signed retry, the final response's MESSAGE-INTEGRITY validates, and the mapped address is printed with exit code 0.

Negative checks:

```bash
# No credentials: initial 401 is reported as a server error, exit code 5, no retry
cargo run -p stunclient -- 127.0.0.1:3478

# Wrong password: 401 after the challenge, exit code 5
cargo run -p stunclient -- 127.0.0.1:3478 \
  --auth-mode long-term --username alice --password wrong
```

## 5. Confirm unauthenticated mode is unchanged

```bash
cargo run -p stunserver -- --listen 127.0.0.1:3478
cargo run -p stunclient -- 127.0.0.1:3478
```

Expected: success with a mapped address; a request carrying USERNAME/MESSAGE-INTEGRITY is ignored rather than rejected.

## 6. Confirm RFC 5780 non-claims

```bash
cargo test --all rfc5780
```

Expected: CHANGE-REQUEST, PADDING, and RESPONSE-PORT still produce 420, and no response contains OTHER-ADDRESS or RESPONSE-ORIGIN. Search project documentation for `stun-behavior` and confirm no service advertising exists.

## 7. Confirm no secrets leak

While the authenticated server runs, inspect its console and `log/stunserver.log`:

```bash
grep -iE 'correct-horse|password|server_secret' log/stunserver.log
```

Expected: no matches. MESSAGE-INTEGRITY summaries show a length, not a value.

## 8. Check the conformance matrix

Open `docs/rfc-conformance.md` and confirm:

- Every in-scope RFC 5389, RFC 5769, and RFC 5780 clause has status `conformant`, `gap`, or `out-of-scope`.
- Every former gap names the closed FR and the test evidence.
- Out-of-scope rows state why (RFC 3489, TCP/TLS, NAT behavior discovery).

## 9. Performance gate

With the release server running and authentication disabled, record a baseline:

```bash
cargo run -p stunbench --release -- 127.0.0.1:3478 --qps 1000 --duration 10
cargo run -p stunbench --release -- 127.0.0.1:3478 --qps 1000 --duration 10 --fingerprint
```

Expected: the plain run matches the pre-change baseline, and the `--fingerprint` run's p99 is at most 5% above it (SC-008). The flag makes `stunbench` append a valid FINGERPRINT to every request so the server's validation path is exercised.
