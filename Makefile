# STUN Server - Root Makefile (Development Commands)
# Purpose: Provide common development commands for builders
# Usage: make [target]

.PHONY: all build test package clean fmt lint help

# Default target
all: build

# Build release binary
build:
	@echo "Building STUN server (release mode)..."
	cargo build --release
	@echo "Build complete: target/release/stunserver"

# Run all tests
test:
	@echo "Running tests..."
	cargo test
	@echo "Running ignored tests..."
	cargo test -- --ignored
	@echo "All tests complete"

# Create distribution package
package: build
	@echo "Creating distribution package..."
	@mkdir -p dist/stunserver-{bin,etc,log}
	@cp target/release/stunserver dist/stunserver-bin/
	@cp deploy/stunserver.conf.example dist/stunserver-etc/stunserver.conf
	@cp systemd/stunserver.service dist/
	@echo "Package created in dist/"
	@echo "Contents:"
	@ls -la dist/

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	rm -rf dist/
	@echo "Clean complete"

# Format code
fmt:
	@echo "Formatting code..."
	cargo fmt --all
	@echo "Formatting complete"

# Run clippy lints
lint:
	@echo "Running clippy..."
	cargo clippy -- -D warnings
	@echo "Lint check complete"

# Show help
help:
	@echo "STUN Server - Development Commands"
	@echo ""
	@echo "Available targets:"
	@echo "  make build    - Build release binary"
	@echo "  make test     - Run all tests (including ignored)"
	@echo "  make package  - Create distribution package"
	@echo "  make clean    - Clean build artifacts"
	@echo "  make fmt      - Format code with rustfmt"
	@echo "  make lint     - Run clippy lints (zero warnings policy)"
	@echo "  make help     - Show this help message"
	@echo ""
	@echo "For deployment commands, see deploy/Makefile"
