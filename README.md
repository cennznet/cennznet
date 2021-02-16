# CENNZnet Node
[![license: LGPL v3](https://img.shields.io/badge/License-LGPL%20v3-blue.svg)](LICENSE) ![ci status badge](https://github.com/cennznet/cennznet/workflows/CI/badge.svg) [![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](docs/CONTRIBUTING.adoc)

CENNZnet node based on [Plug](https://github.com/plugblockchain/plug-blockchain)

## Running a CENNZnet node

There are a number of ways to run a CENNZnet node. Please choose one that suits best for your interest.

### Using Docker

Make sure Docker is installed and running on your machine.
If you need to install [Docker](https://www.docker.com/), head over to [Docker for Desktop](https://www.docker.com/products/docker-desktop) first, get it installed, create an account and login. Make sure Docker is running in the background.

Now you can proceed to the terminal with the following command:

```
# Start a local validator on a development chain
$ docker run \
    -p 9933:9933 -p 9944:9944 \
    cennznet/cennznet:1.2.3 \
    --dev \
    --unsafe-ws-external \
    --unsafe-rpc-external \
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
$ rustup install nightly
$ rustup target add --toolchain=nightly wasm32-unknown-unknown
```

#### 3. Build the node binary and run

Then clone the repo, build the binary and run it.
```
$ git clone https://github.com/cennznet/cennznet.git
$ cd cennznet
$ cargo build --release
$ ./target/release/cennznet --help

# start a validator node for development
$ ./target/release/cennznet --dev
```

------

## Contributing

All PRs are welcome! Please follow our contributing guidelines [here](docs/CONTRIBUTING.md).

## Community

Join our official CENNZnet Discord server 🤗

* Get CENNZnet technical support 🛠
* Meet startups and DApp developers 👯‍♂️
* Learn more about CENNZnet and blockchain 🙌
* Get updates on CENNZnet bounties and grants 💰
* Hear about the latest hackathons, meetups and more 👩‍💻

Join the Discord server by clicking on the badge below!

[![Support Server](https://img.shields.io/discord/591914197219016707.svg?label=Discord&logo=Discord&colorB=7289da&style=for-the-badge)](https://discord.gg/AnB3tRtkJ4)
