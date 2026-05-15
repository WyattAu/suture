FROM rust:1.85-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends protobuf-compiler pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p crates/suture-platform/src && echo "" > crates/suture-platform/src/lib.rs
RUN cargo build --release -p suture-platform 2>/dev/null || true
COPY . .
RUN touch crates/suture-platform/src/*.rs
RUN cargo build --release -p suture-platform

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tini && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/suture-platform /usr/local/bin/suture-platform
RUN chmod +x /usr/local/bin/suture-platform

RUN mkdir -p /data && groupadd -r suture && useradd -r -g suture -d /data suture && chown -R suture:suture /data

ENV SUTURE_DATA_DIR=/data
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
EXPOSE 8080

USER suture
ENTRYPOINT ["tini", "--"]
CMD ["suture-platform"]
