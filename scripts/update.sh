#!/usr/bin/env bash

#
# Usage: ./scripts/update.sh
#

cargo update
cargo check

pushd runtime/wasm
cargo update
popd

./scripts/build.sh
cargo run -- --chain=kauri
