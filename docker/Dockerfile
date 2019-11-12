FROM rustlang/rust:nightly AS builder
WORKDIR /cennznet
ENV RUST_VERSION="nightly-2019-07-14"
RUN apt-get update && \
      apt-get -y install apt-utils cmake pkg-config libssl-dev git clang libclang-dev && \
      rustup toolchain install $RUST_VERSION && \
      rustup default $RUST_VERSION && \
      rustup target add wasm32-unknown-unknown && \
      cargo install --git https://github.com/alexcrichton/wasm-gc && \
      rustup target add x86_64-unknown-linux-musl
ENV CARGO_HOME=/cennznet/.cargo
COPY . /cennznet
RUN cd /cennznet/runtime/wasm && \
      cargo build --target=wasm32-unknown-unknown --release && \
      wasm-gc target/wasm32-unknown-unknown/release/cennznet_runtime.wasm target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm && \
      cd /cennznet && \
      cargo build --release

FROM debian:stretch-slim
LABEL maintainer="support@centrality.ai"

RUN apt update && apt install -y ca-certificates \
    openssl

RUN mkdir -p /root/.local/share/Substrate && \
      ln -s /root/.local/share/Substrate /data

EXPOSE 30333 9933 9944
VOLUME ["/data"]

ARG PROFILE=release
COPY --from=0 /cennznet/target/release/cennznet /usr/local/bin

ENTRYPOINT ["/usr/local/bin/cennznet"]
