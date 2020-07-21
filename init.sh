#!/bin/bash
nightly_version=nightly-2020-06-25

echo Installing the stable Rust toolchain...
rustup install stable
rustup default stable

echo Installing $nightly_version and wasm toolchains...
rustup install $nightly_version
rustup target add --toolchain=$nightly_version wasm32-unknown-unknown
