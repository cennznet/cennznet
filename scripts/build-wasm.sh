#!/usr/bin/env bash

# This script assumes that all pre-requisites are installed.
set -e

# Set the default to be the top level project repo, if not fall back to hopefully the project repo with docker
PROJECT_ROOT=$(git rev-parse --show-toplevel) || PROJECT_ROOT="/cennznet"

pushd $PROJECT_ROOT/runtime/wasm

cargo +nightly build --target=wasm32-unknown-unknown --release
wasm-gc target/wasm32-unknown-unknown/release/cennznet_runtime.wasm target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm
popd