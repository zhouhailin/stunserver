# Multi-stage Dockerfile for STUN Server
# Stage 1: Builder - Compile Rust application
FROM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev

# Set working directory
WORKDIR /build

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY src/ ./src/
COPY build.rs ./

# Build release binary
RUN cargo build --release

# Stage 2: Runtime - Minimal Alpine image
FROM alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    tzdata

# Create non-root user for security
RUN addgroup -g 1000 stunserver && \
    adduser -D -u 1000 -G stunserver stunserver

# Set working directory
WORKDIR /opt/stunserver

# Copy binary from builder
COPY --from=builder /build/target/release/stunserver ./bin/stunserver

# Copy default configuration
COPY deploy/stunserver.conf.example ./etc/stunserver.conf

# Create log directory with proper permissions
RUN mkdir -p ./log && \
    chown -R stunserver:stunserver /opt/stunserver && \
    chmod -R 755 ./log

# Expose STUN ports (UDP and TCP)
EXPOSE 3478/udp
EXPOSE 3478/tcp

# Switch to non-root user
USER stunserver

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD nc -z localhost 3478 || exit 1

# Entry point
ENTRYPOINT ["./bin/stunserver"]

# Default command
CMD ["--config", "./etc/stunserver.conf"]
