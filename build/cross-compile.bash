#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

TARGET="${1:-x86_64-unknown-linux-gnu}"

echo "Building for $TARGET..."
rustup target add "$TARGET" 2>/dev/null || true
cargo build --release --target "$TARGET"

echo "Binary: target/$TARGET/release/sortie"
echo "Agent:  target/$TARGET/release/sortie-agent"
