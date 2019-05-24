# CENNZnet Node

CENNZnet node based on Substrate

## Development

__Install rust__
```bash
# Install rustup
curl -sSf https://static.rust-lang.org/rustup.sh | sh

# Make installed tool available to current shell
source ~/.cargo/env

# Install nightly version of rust and required tools
./scripts/init.sh
```


__Build__

```bash
# compile runtime to wasm
./scripts/build.sh

# compile the node
cargo build
```


__Run__
```bash
# Run your own testnet with a validator
cargo run -- --dev
# or
./target/debug/cennznet --dev
```


__Purge chain__
```bash
# For local testnet
cargo run -- purge-chain --dev
# or
./target/debug/cennznet purge-chain --dev
```

