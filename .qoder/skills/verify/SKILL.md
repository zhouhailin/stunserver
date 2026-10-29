---
name: verify
description: Run full verification before claiming completion — format check, lint, and tests. Use before committing or marking tasks done.
---

Run all verification checks in sequence:

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

Exit on first failure. Report which step failed and the error output.

Only claim "all checks pass" after running this full sequence successfully.
