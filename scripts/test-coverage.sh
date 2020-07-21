#!/bin/sh

# Build and test the specified rust packages to produce the coverage files which is then fed to grcov to generate a report
example="$0 crml-cennzx-spot crml-sylo"

if [ $# -eq 0 ]; then
  echo "Error: no rust package name is specified"
  echo "Usage example: $example"
  exit 1
fi

for p in "$@"
do
    packages="--package $p $packages";
done

set -o xtrace

cargo install grcov

rustup target add --toolchain=$RUST_NIGHTLY x86_64-unknown-linux-gnu
mv ~/.rustup/toolchains/nightly-* ~/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu

export CARGO_INCREMENTAL=0
export RUSTFLAGS="-Zprofile -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off -Zpanic_abort_tests -Cpanic=abort"
export RUSTDOCFLAGS="-Cpanic=abort"

export SKIP_WASM_BUILD=1
export LLVM_CONFIG_PATH="/usr/local/opt/llvm/bin/llvm-config"

cargo +nightly build $packages --target x86_64-unknown-linux-gnu
cargo +nightly test $packages --target x86_64-unknown-linux-gnu

grcov ./target/x86_64-unknown-linux-gnu/debug/ -s . -t html --llvm --branch --ignore-not-existing -o /tmp/coverage/

set +o xtrace
