#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
cargo build --release
echo "Binary: target/release/sortie"
echo "Agent:  target/release/sortie-agent"
