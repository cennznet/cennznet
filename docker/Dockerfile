FROM rust:1.32.0 AS builder
WORKDIR /cennznet
RUN apt-get update && \
      apt-get -y install apt-utils cmake pkg-config libssl-dev git clang libclang-dev && \
      rustup default nightly && \
      rustup update nightly && \
      rustup target add wasm32-unknown-unknown --toolchain nightly && \
      cargo install --git https://github.com/alexcrichton/wasm-gc && \
      rustup target add x86_64-unknown-linux-musl --toolchain=nightly
ENV CARGO_HOME=/cennznet/.cargo
COPY . /cennznet
RUN cd /cennznet/runtime/wasm && \
      cargo +nightly build -Z offline --target=wasm32-unknown-unknown --release && \
      wasm-gc target/wasm32-unknown-unknown/release/cennznet_runtime.wasm target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm && \
      cd /cennznet && \
      cargo +nightly build -Z offline --release

FROM debian:stretch-slim
LABEL maintainer="developers@centrality.ai"

RUN apt update && apt install -y ca-certificates \
    openssl

RUN mkdir -p /root/.local/share/Substrate && \
      ln -s /root/.local/share/Substrate /data

EXPOSE 30333 9933 9944
VOLUME ["/data"]

ARG PROFILE=release
COPY --from=0 /cennznet/target/release/cennznet /usr/local/bin

ENTRYPOINT ["/usr/local/bin/cennznet"]
