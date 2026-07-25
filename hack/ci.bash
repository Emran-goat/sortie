#!/usr/bin/env bash
set -euo pipefail

echo "==> Format check"
cargo fmt --check

echo "==> Clippy"
cargo clippy -- -D warnings

echo "==> Tests"
cargo test

echo "==> Build (release)"
cargo build --release

echo "CI passed."
