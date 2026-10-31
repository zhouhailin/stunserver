# Contract: Server Configuration `[auth]` Section

**Feature**: `008-rfc-compliance-audit` | **File**: `config/stunserver.conf` (TOML)

The `[auth]` section is optional. When absent, the server runs in `disabled` mode and behaves as a public Binding server.

## Schema

```toml
[auth]
# disabled | short-term | long-term
mode = "disabled"

# Long-term realm. Required when mode = "long-term"; ignored otherwise.
realm = "stunserver"

# Lifetime of issued NONCE values in seconds (1-86400). Used only in long-term mode.
nonce_lifetime_sec = 600

# One entry per valid username. Required when mode is short-term or long-term.
[[auth.users]]
username = "alice"
password = "change-me"
```

## Validation rules

| Rule | Error behavior |
|------|----------------|
| `mode` is one of `disabled`, `short-term`, `long-term` | Reject configuration at startup with a clear message |
| `mode != "disabled"` requires at least one user | Reject configuration at startup |
| `username` is non-empty and unique across entries | Reject configuration at startup |
| `password` is non-empty | Reject configuration at startup |
| `mode == "long-term"` requires a non-empty `realm` | Reject configuration at startup |
| `mode == "long-term"` requires `1 <= nonce_lifetime_sec <= 86400` | Reject configuration at startup |
| No auth field is required when `mode == "disabled"` | Start normally |

## Key handling

- Short-term: the configured password bytes are the MESSAGE-INTEGRITY key.
- Long-term: each key is computed once at startup as `MD5(username ":" realm ":" SASLprep(password))`.
- Derived keys and passwords are held in memory only and are never logged.
- The file is plaintext. Documentation must instruct owners to set mode `0600` and to avoid committing production secrets.

## Backward compatibility

Existing configuration files without an `[auth]` section load unchanged and produce a public Binding server, preserving current behavior for deployments that do not opt in.

## CLI interaction

`stunserver --config <path>` continues to select the configuration file. No new server CLI flags are required; authentication is configured only through the file so secrets do not appear in process listings.
