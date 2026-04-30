ARG RUST_VERSION=1.85

FROM rust:${RUST_VERSION}-slim AS builder
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
ARG BUILD_TARGET=suture-platform
RUN cargo build --release -p ${BUILD_TARGET}

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
ARG BUILD_TARGET=suture-platform
COPY --from=builder /app/target/release/${BUILD_TARGET} /usr/local/bin/suture
EXPOSE 3000 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/ || exit 1
CMD ["suture"]
