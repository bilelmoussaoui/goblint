FROM rust:slim AS builder

WORKDIR /build

# musl toolchain for a fully static binary (no glibc dependency)
RUN apt-get update && \
    apt-get install -y --no-install-recommends musl-tools && \
    rm -rf /var/lib/apt/lists/* && \
    rustup target add x86_64-unknown-linux-musl

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./
COPY gobject-ast/Cargo.toml gobject-ast/
COPY tree-sitter-c-gobject/Cargo.toml tree-sitter-c-gobject/
# tree-sitter-c-gobject's build.rs compiles pre-generated C
COPY tree-sitter-c-gobject/bindings tree-sitter-c-gobject/bindings
COPY tree-sitter-c-gobject/src tree-sitter-c-gobject/src

# Create dummy Rust source files to cache dependencies
RUN mkdir -p src gobject-ast/src && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > gobject-ast/src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl --bin goblint && \
    rm -f src/main.rs gobject-ast/src/main.rs

# Copy actual source code
COPY src ./src
COPY gobject-ast ./gobject-ast
COPY docs ./docs

# Build the actual binary
RUN cargo build --release --target x86_64-unknown-linux-musl --bin goblint

# Runtime stage - minimal image
FROM debian:bookworm-slim

# Install git (often needed in CI) and ca-certificates for HTTPS
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        git \
        ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/goblint /usr/local/bin/goblint

# Set working directory
WORKDIR /workspace

# Run goblint by default
ENTRYPOINT ["goblint"]
CMD ["--help"]
