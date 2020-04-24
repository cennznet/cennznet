#!/bin/bash
nightly_pattern='^.+(nightly-[0-9]{4}-[0-9]{2}-[0-9]{2}).*$'
nightly_version=$(grep -E $nightly_pattern ./.circleci/config.yml | head -n 1)
nightly_version=$(sed -E 's/'$nightly_pattern'/\1/g' <<< "$nightly_version")

echo Installing the stable Rust toolchain...
rustup install stable
rustup default stable

echo Installing $nightly_version and wasm toolchains...
rustup install $nightly_version
rustup target add --toolchain=$nightly_version wasm32-unknown-unknown
