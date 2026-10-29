# AGENTS.md

This file provides guidance to the AI agent when working with code in this repository.

## Project Overview

High-performance STUN server written in Rust. RFC 5389 compliant.

## Build & Test Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run tests
cargo fmt                # Format code
cargo clippy             # Lint
```

## Code Style

- Follow standard Rust conventions (rustfmt + clippy)
- Use `cargo fmt` before committing
- Address all clippy warnings

## Git Conventions

- **Commits**: Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, etc.)
- **Branches**: `type/description` (e.g., `feat/stun-binding`, `fix/attribute-handling`)

## Superpowers Framework

This project uses the superpowers-zh skill framework (`.qoder/rules/superpowers-zh.md`). Before implementing features:
1. Check for matching skills in `.qoder/skills/`
2. Use brainstorming skill for new feature design
3. Follow TDD: write tests before implementation
4. Run verification commands before claiming completion
