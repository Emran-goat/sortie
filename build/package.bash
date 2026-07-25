#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION="${1:-$(cargo metadata --format-version 1 --no-deps | python3 -c 'import sys,json; print(json.load(sys.stdin)["packages"][0]["version"])')}"
TARGET="${2:-x86_64-unknown-linux-gnu}"

cargo build --release --target "$TARGET"

mkdir -p "dist/sortie-$VERSION"
cp "target/$TARGET/release/sortie" "dist/sortie-$VERSION/"
cp "target/$TARGET/release/sortie-agent" "dist/sortie-$VERSION/"
cp README.md LICENSE "dist/sortie-$VERSION/"

cd dist
tar czf "sortie-$VERSION-$TARGET.tar.gz" "sortie-$VERSION/"
echo "dist/sortie-$VERSION-$TARGET.tar.gz"
