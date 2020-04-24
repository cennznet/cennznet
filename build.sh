#!/bin/bash
stable_pattern='^\s*RUST_VERSION:\s'
nightly_pattern='^.+(nightly-[0-9]{4}-[0-9]{2}-[0-9]{2}).*$'

stable_version=$(grep -E $stable_pattern ./.circleci/config.yml | uniq)
stable_version=${stable_version#*:}

nightly_version=$(grep -E $nightly_pattern ./.circleci/config.yml | head -n 1)
nightly_version=$(sed -E 's/'$nightly_pattern'/\1/g' <<< "$nightly_version")

echo Installing rustc version $stable_version...
rustup install $stable_version
rustup default $stable_version

echo Installing $nightly_version and wasm toolchain...
rustup install $nightly_version
rustup target add --toolchain=$nightly_version wasm32-unknown-unknown

echo Building CENNZnet...
cargo build --release
