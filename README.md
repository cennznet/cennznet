# CENNZnet Node
[![license: LGPL v3](https://img.shields.io/badge/License-LGPL%20v3-blue.svg)](LICENSE) ![ci status badge](https://github.com/cennznet/cennznet/workflows/CI/badge.svg) [![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](docs/CONTRIBUTING.adoc)

CENNZnet node built on [Substrate](https://github.com/paritytech/substrate).
For getting started and technical guides, please refer to the [CENNZnet Wiki](https://wiki.cennz.net/#/).

## Contributing

All PRs are welcome! Please follow our contributing guidelines [here](docs/CONTRIBUTING.md).

------

## Community

Join our official CENNZnet Discord server 🤗

* Get CENNZnet technical support 🛠
* Meet startups and DApp developers 👯‍♂️
* Learn more about CENNZnet and blockchain 🙌
* Get updates on CENNZnet bounties and grants 💰
* Hear about the latest hackathons, meetups and more 👩‍💻

Join the Discord server by clicking on the badge below!

[![Support Server](https://img.shields.io/discord/801219591636254770.svg?label=Discord&logo=Discord&colorB=7289da&style=for-the-badge)](https://discord.gg/AnB3tRtkJ4)

### Run with Docker

Use the latest CENNZnet docker image to get started quickly
```bash
# Start a local validator on a development chain
$ docker run \
    -p 9933:9933 -p 9944:9944 \
    cennznet/cennznet:latest \
    --dev \
    --unsafe-ws-external \
    --unsafe-rpc-external
```

### Run from Source

Follow the steps to build and run a CENNZnet node from the source code.

#### 1) Set up build environment

For Linux (the example below is for Debian-based machines):
```bash
$ sudo apt install -y build-essential clang cmake gcc git libclang-dev libssl-dev pkg-config
```

For MacOS (via Homebrew):
```bash
$ brew install openssl cmake llvm
```

#### 2) Install Rust

Install Rust on your machine through [here](https://rustup.rs/), and the following rust version and toolchains.
```bash
$ cargo --version
$ rustup install nightly
$ rustup target add --toolchain=nightly wasm32-unknown-unknown
```

#### 3) Build and Run

Clone the repo, build the binary and run it.
```bash
$ git clone https://github.com/cennznet/cennznet.git
$ cd cennznet
$ cargo build --release # or remove  '--release' for quick debug build
$ ./target/release/cennznet --help

# start a validator node for development
$ ./target/release/cennznet --dev
```

### Build Docker Image

Prepare your docker engine, and make sure it is running.

```bash
# To use the default image name and tag
$ make 

# To custom your image name and tag
$ IMAGE_NAME='cennznet' IMAGE_TAG='v1.5.1' DOCKER_BUILD_ARGS='--no-cache --quiet' make build

# Without using make
$ docker build --no-cache -t cennznet:v1.5.1 .
```
