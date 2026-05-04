# Build stage
FROM rust:1.85-bookworm AS builder
WORKDIR /app

# Install protoc (required by suture-hub)
RUN apt-get update && apt-get install -y protobuf-compiler pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/suture-platform/Cargo.toml crates/suture-platform/
COPY crates/suture-hub/Cargo.toml crates/suture-hub/
# Create dummy source files for dependency caching
RUN mkdir -p crates/suture-platform/src && echo "fn main() {}" > crates/suture-platform/src/main.rs
RUN mkdir -p crates/suture-hub/src && echo "" > crates/suture-hub/src/lib.rs
# Create all dependency crate dirs
RUN for crate in suture-common suture-core suture-protocol suture-driver suture-merge suture-daemon suture-raft suture-s3 suture-vfs suture-tui suture-lsp suture-wasm-plugin suture-bench suture-e2e suture-ooxml suture-driver-json suture-driver-yaml suture-driver-toml suture-driver-xml suture-driver-csv suture-driver-sql suture-driver-html suture-driver-markdown suture-driver-svg suture-driver-docx suture-driver-xlsx suture-driver-pptx suture-driver-pdf suture-driver-image suture-driver-feed suture-driver-ical suture-driver-otio suture-driver-example suture-cli suture-node; do mkdir -p crates/$crate/src && touch crates/$crate/src/lib.rs; done
# Build dependencies only (this layer is cached)
RUN cargo build --release -p suture-platform 2>/dev/null || true

# Copy actual source code
COPY . .

# Touch all source files to invalidate the cache for real build
RUN find crates -name "*.rs" -exec touch {} +

# Build for real
RUN cargo build --release -p suture-platform

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/suture-platform /usr/local/bin/suture-platform

ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
EXPOSE 8080

CMD ["suture-platform"]
