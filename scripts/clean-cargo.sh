#!/bin/bash
#
# Clean build related artifacts
#
# Usage:
#   ./scripts/clean.sh
#
echo "Cleaning cargo cache..."
cargo +nightly clean
rm -rf .cargo/
rm -rf runtime/wasm/target/
