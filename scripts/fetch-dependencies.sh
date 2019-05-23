#!/bin/bash
#
# Fetch cargo dependencies
#

# Fetch dependencies locally. Copied into container on build
# This is a workaround to avoid moutning an SSH key into the build container
PROJECT_ROOT="$(git rev-parse --show-toplevel)"
export CARGO_HOME="$PROJECT_ROOT/.cargo"

echo "Fetching project dependencies..."

# Have to fetch resursivley for all project modules as there is no
# `cargo fetch --all` type command to do it automatically
cargo +nightly metadata --format-version 1 | jq '.packages | map(.manifest_path)| .[] | select(contains("cennznet-node/.") | not)' | xargs -I{} dirname {} | xargs -I{} sh -c "cd {} && cargo fetch"

# Have to manually fetch the runtime/wasm dependencies

pushd "$PROJECT_ROOT/runtime/wasm"
cargo fetch
popd