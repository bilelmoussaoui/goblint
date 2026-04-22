FROM rust:1.92-slim AS builder

WORKDIR /build

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./
COPY gobject-ast/Cargo.toml gobject-ast/

# Create dummy source files to cache dependencies
RUN mkdir -p src gobject-ast/src && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > gobject-ast/src/main.rs && \
    cargo build --release --bin goblint && \
    rm -rf src gobject-ast/src

# Copy actual source code
COPY src ./src
COPY gobject-ast ./gobject-ast
COPY docs ./docs

# Build the actual binary
RUN cargo build --release --bin goblint

# Runtime stage - minimal image
FROM debian:bookworm-slim

# Install git (often needed in CI) and ca-certificates for HTTPS
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        git \
        ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /build/target/release/goblint /usr/local/bin/goblint

# Set working directory
WORKDIR /workspace

# Run goblint by default
ENTRYPOINT ["goblint"]
CMD ["--help"]
