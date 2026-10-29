# Contributing to STUN Server

Thank you for your interest in contributing to STUN Server! This document provides guidelines and workflows for development.

## Development Workflow

### 1. Fork and Clone

```bash
# Fork the repository on GitHub, then clone your fork
git clone https://github.com/YOUR_USERNAME/stunserver.git
cd stunserver

# Add upstream remote
git remote add upstream https://github.com/original/stunserver.git
```

### 2. Create a Feature Branch

```bash
# Update main branch
git checkout main
git pull upstream main

# Create feature branch
git checkout -b feat/your-feature-name
```

Branch naming convention:
- `feat/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation changes
- `refactor/` - Code refactoring
- `test/` - Test additions
- `chore/` - Maintenance tasks

### 3. Development

```bash
# Install dependencies and build
cargo build

# Run tests
cargo test --all

# Check formatting
cargo fmt --all

# Run linter
cargo clippy --all-targets --all-features -- -D warnings
```

### 4. Write Tests

All new features must include tests. We maintain 95%+ code coverage.

```bash
# Run tests with coverage
cargo install cargo-tarpaulin
cargo tarpaulin --all

# Check coverage report
# Ensure new code has adequate test coverage
```

### 5. Commit Changes

We use [Conventional Commits](https://www.conventionalcommits.org/) for commit messages:

```bash
git add .
git commit -m "feat: add IPv6 dual-stack support"
git commit -m "fix: resolve memory leak in connection pool"
git commit -m "docs: update configuration examples"
git commit -m "test: add integration tests for benchmark tool"
```

Types:
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation
- `test` - Tests
- `refactor` - Code refactoring
- `chore` - Maintenance
- `perf` - Performance improvements

### 6. Push and Create PR

```bash
# Push to your fork
git push origin feat/your-feature-name

# Create a pull request on GitHub
```

## Code Style

### Rust Conventions

- Follow standard Rust conventions (rustfmt + clippy)
- Use `cargo fmt` before committing
- Address all clippy warnings
- Write clear, descriptive variable and function names
- Add documentation comments (`///`) for public APIs

### Error Handling

- Use `anyhow` for application-level errors
- Use `thiserror` for library-level errors
- Provide clear, actionable error messages
- Handle errors at appropriate levels

### Testing

- Unit tests in the same file as the code (`#[cfg(test)] mod tests`)
- Integration tests in `tests/` directory
- Test both success and failure cases
- Use descriptive test names

## Pull Request Process

### Before Submitting

1. **Run all checks**:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all
   ```

2. **Update documentation** if you changed:
   - Public APIs
   - Configuration options
   - CLI arguments
   - Behavior

3. **Add tests** for:
   - New features
   - Bug fixes
   - Edge cases

### PR Description

Include:
- **What**: Description of changes
- **Why**: Motivation and context
- **How**: Implementation approach
- **Testing**: How you tested the changes
- **Screenshots**: If applicable (for UI changes)

### Review Process

1. CI checks must pass (formatting, clippy, tests, coverage)
2. At least one maintainer must approve
3. Address review comments
4. Squash commits if requested
5. Maintainer will merge

## Architecture Overview

### Workspace Structure

```
stunserver/
├── stun-protocol/      # Core STUN protocol library
│   ├── src/
│   │   ├── message.rs  # STUN message parsing/encoding
│   │   ├── attributes.rs # STUN attributes
│   │   └── address.rs  # Mapped address types
│   └── tests/
│
├── stunserver/         # Server binary
│   ├── src/
│   │   ├── main.rs     # CLI and entry point
│   │   ├── config.rs   # Configuration
│   │   ├── logging.rs  # Logging system
│   │   └── server.rs   # Server logic
│   └── tests/
│
├── stunclient/         # Client binary
│   ├── src/
│   │   ├── main.rs     # CLI and entry point
│   │   └── client.rs   # Client logic
│   └── tests/
│
└── stunbench/          # Benchmark binary
    ├── src/
    │   ├── main.rs     # CLI and entry point
    │   ├── bench.rs    # Benchmark engine
    │   └── stats.rs    # Statistics collection
    └── tests/
```

### Key Concepts

- **Async I/O**: All network operations use Tokio async runtime
- **Protocol Compliance**: Strict RFC 5389 implementation
- **Performance**: Pre-allocated buffers, minimal allocations
- **Observability**: Build metadata, structured logging

## Getting Help

- **Issues**: Use GitHub issues for bugs and feature requests
- **Discussions**: Use GitHub Discussions for questions
- **Documentation**: Check README.md and docs/

## Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Assume good intentions
- Help others learn and grow

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

Thank you for contributing!
