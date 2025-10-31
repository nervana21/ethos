FROM rust:1.80-slim as builder

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder stage
COPY --from=builder /app/target/release/rust-lightning-harness /usr/local/bin/

# Expose the port
EXPOSE 9836

# Set default environment variables
ENV RUST_LIGHTNING_HARNESS_PORT=9836
ENV RUST_LOG=info

# Run the harness
CMD ["rust-lightning-harness"]
