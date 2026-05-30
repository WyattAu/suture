# Suture development tasks
set shell := ["bash", "-c"]

EXCLUDE := "--exclude suture-fuzz --exclude suture-py --exclude suture-node --exclude suture-e2e --exclude suture-bench --exclude suture-vfs --exclude suture-lsp --exclude suture-daemon --exclude suture-wasm-plugin"

default: check

check:
    cargo check --workspace {{EXCLUDE}}

test:
    cargo test --workspace {{EXCLUDE}} -- --test-threads=1

lint:
    cargo clippy --workspace {{EXCLUDE}} -- -D warnings

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

build:
    cargo build --workspace --release -p suture-cli

clean:
    cargo clean

run *args:
    cargo run --bin suture-cli -- {{args}}

install-hooks:
    cp scripts/pre-commit .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
    cp scripts/pre-push .git/hooks/pre-push && chmod +x .git/hooks/pre-push
    @echo "Hooks installed. Pre-commit and pre-push will run fmt + clippy + test."

doc:
    cargo doc --workspace {{EXCLUDE}} --no-deps
