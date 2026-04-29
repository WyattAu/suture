# Build stage
FROM rust:1.85-bookworm AS builder

RUN apt-get update && apt-get install -y protobuf-compiler pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/suture-platform/Cargo.toml crates/suture-platform/
COPY crates/suture-hub/Cargo.toml crates/suture-hub/
COPY crates/suture-driver/Cargo.toml crates/suture-driver/
COPY crates/suture-driver-json/Cargo.toml crates/suture-driver-json/
COPY crates/suture-driver-yaml/Cargo.toml crates/suture-driver-yaml/
COPY crates/suture-driver-toml/Cargo.toml crates/suture-driver-toml/
COPY crates/suture-driver-xml/Cargo.toml crates/suture-driver-xml/
COPY crates/suture-driver-csv/Cargo.toml crates/suture-driver-csv/
COPY crates/suture-driver-sql/Cargo.toml crates/suture-driver-sql/
COPY crates/suture-driver-html/Cargo.toml crates/suture-driver-html/
COPY crates/suture-driver-markdown/Cargo.toml crates/suture-driver-markdown/
COPY crates/suture-driver-properties/Cargo.toml crates/suture-driver-properties/
COPY crates/suture-driver-ini/Cargo.toml crates/suture-driver-ini/
COPY crates/suture-driver-svg/Cargo.toml crates/suture-driver-svg/
COPY crates/suture-driver-env/Cargo.toml crates/suture-driver-env/
COPY crates/suture-driver-dotenv/Cargo.toml crates/suture-driver-dotenv/
COPY crates/suture-driver-json5/Cargo.toml crates/suture-driver-json5/
COPY crates/suture-driver-hcl/Cargo.toml crates/suture-driver-hcl/
COPY crates/suture-driver-protobuf/Cargo.toml crates/suture-driver-protobuf/
COPY crates/suture-driver-graphql/Cargo.toml crates/suture-driver-graphql/
COPY crates/suture-common/Cargo.toml crates/suture-common/
COPY crates/suture-protocol/Cargo.toml crates/suture-protocol/
COPY crates/suture-core/Cargo.toml crates/suture-core/
COPY crates/suture-raft/Cargo.toml crates/suture-raft/
COPY crates/suture-s3/Cargo.toml crates/suture-s3/
COPY crates/suture-daemon/Cargo.toml crates/suture-daemon/
COPY crates/suture-tui/Cargo.toml crates/suture-tui/
COPY crates/suture-cli/Cargo.toml crates/suture-cli/
COPY crates/suture-merge/Cargo.toml crates/suture-merge/
COPY crates/suture-bench/Cargo.toml crates/suture-bench/
COPY crates/suture-vfs/Cargo.toml crates/suture-vfs/
COPY crates/suture-fuzz/Cargo.toml crates/suture-fuzz/
COPY crates/suture-otio/Cargo.toml crates/suture-otio/
COPY crates/suture-e2e/Cargo.toml crates/suture-e2e/
COPY crates/suture-py/Cargo.toml crates/suture-py/

# Create dummy source files for dependency caching
RUN mkdir -p crates/suture-platform/src && echo "fn main() {}" > crates/suture-platform/src/main.rs
RUN for crate in suture-hub suture-driver suture-driver-json suture-driver-yaml suture-driver-toml suture-driver-xml suture-driver-csv suture-driver-sql suture-driver-html suture-driver-markdown suture-driver-properties suture-driver-ini suture-driver-svg suture-driver-env suture-driver-dotenv suture-driver-json5 suture-driver-hcl suture-driver-protobuf suture-driver-graphql suture-common suture-protocol suture-core suture-raft suture-s3 suture-daemon suture-tui suture-cli suture-merge suture-bench suture-vfs suture-fuzz suture-otio suture-e2e; do
    mkdir -p crates/$crate/src && echo "" > crates/$crate/src/lib.rs;
done

# Build dependencies only (cached layer)
RUN cargo build --release -p suture-platform 2>/dev/null || true

# Copy actual source code
COPY . .

# Touch source files to invalidate cache for this layer only
RUN find crates -name "*.rs" -exec touch {} +

# Build the platform binary
RUN cargo build --release -p suture-platform

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/suture-platform /usr/local/bin/suture-platform
COPY --from=builder /app/target/release/suture /usr/local/bin/suture

RUN groupadd -r suture && useradd -r -g suture -d /data suture
RUN mkdir -p /data && chown suture:suture /data

USER suture
WORKDIR /data

EXPOSE 8080

ENV SUTURE_DB=/data/platform.db
ENV SUTURE_HUB_DB=/data/hub.db
ENV SUTURE_ADDR=0.0.0.0:8080
ENV RUST_LOG=info

ENTRYPOINT ["suture-platform"]
CMD ["--db", "/data/platform.db", "--hub-db", "/data/hub.db", "--addr", "0.0.0.0:8080"]
