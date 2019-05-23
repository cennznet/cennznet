#!/usr/bin/env bash

set -e

echo "*** Initialising WASM build environment"

if [ -z $CI_PROJECT_NAME ] ; then
   rustup update nightly
   rustup default nightly
fi

rustup target add wasm32-unknown-unknown --toolchain nightly
rustup target add x86_64-unknown-linux-musl --toolchain=nightly

# Install wasm-gc. It's useful for stripping slimming down wasm binaries.
command -v wasm-gc || \
	cargo +nightly install --git https://github.com/alexcrichton/wasm-gc --force
