# CENNZnet Node
[![GitHub license](https://img.shields.io/github/license/cennznet/cennznet)](LICENSE) [![CircleCI](https://circleci.com/gh/cennznet/cennznet.svg?style=shield)](https://circleci.com/gh/cennznet/cennznet) [![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](docs/CONTRIBUTING.adoc)

CENNZnet node based on [Plug](https://github.com/plugblockchain/plug-blockchain)

## Running a CENNZnet node

There are a number of ways to run a CENNZnet node. Please choose one that suits best for your interest.

### Using Docker

Make sure Docker is installed and running on your machine.
```
$ docker pull cennznet/cennznet:1.0.0
$ docker run -it cennznet/cennznet:1.0.0 [TODO: check params]
```

### Using the source code

Follow the steps to build and run a node from the source code.

#### 1. Make sure build environment is set up

For Linux (the example below is for Debian-based machines):
```
$ sudo apt install -y build-essential clang cmake gcc git libclang-dev libssl-dev pkg-config
```

For MacOS (via Homebrew):
```
$ brew install openssl cmake llvm
```

For Windows [TODO: may need a separate link]

#### 2. Install Rust and set up Rust environment

Install Rust on your machine through [here](https://rustup.rs/), and the following rust version and toolchains.
```
$ cargo --version
$ rustup install nightly-2019-12-19
$ rustup default nightly-2019-12-19
$ rustup target add --toolchain=nightly-2019-12-19 wasm32-unknown-unknown
$ rustup target add --toolchain=nightly-2019-12-19 x86_64-unknown-linux-musl
```

#### 3. Build the node binary and run

Then clone the repo, build the binary and run it.
```
$ git clone https://github.com/cennznet/cennznet.git
$ cd cennznet
$ cargo build --release
$ cargo run --release [TODO: check params]
```

------

## Contributing

All PRs are welcome! Please follow our contributing guidelines [here](docs/CONTRIBUTING.md).
