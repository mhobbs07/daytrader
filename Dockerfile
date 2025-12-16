# Multi-stage build for optimal image size
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY examples ./examples

# Build release binary
RUN cargo build --release --bin daytrader

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1000 trader

# Create directories
RUN mkdir -p /app/data/db /app/data/logs /app/config && \
    chown -R trader:trader /app

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/daytrader /app/daytrader

# Copy configuration
COPY config/default.toml /app/config/default.toml
COPY src/db/schema.sql /app/schema.sql

# Switch to app user
USER trader

# Expose ports (if needed for future UI)
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD pgrep -x daytrader || exit 1

# Run the application
ENTRYPOINT ["/app/daytrader"]