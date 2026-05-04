FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tini && rm -rf /var/lib/apt/lists/*

COPY suture-platform /usr/local/bin/suture-platform
RUN chmod +x /usr/local/bin/suture-platform

RUN mkdir -p /data

ENV SUTURE_DATA_DIR=/data
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
EXPOSE 8080

ENTRYPOINT ["tini", "--"]
CMD ["suture-platform"]
