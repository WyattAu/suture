# Suture development tasks
set shell := ["fish", "-c"]

default: check

check:
    cargo check --workspace

test:
    cargo test --workspace

lint:
    cargo clippy --workspace -- -D warnings

fmt:
    cargo fmt --workspace

fmt-check:
    cargo fmt --workspace -- --check

build:
    cargo build --workspace --release

clean:
    cargo clean

run *args:
    cargo run --bin suture-cli -- {{args}}
