FROM  rustlang/rust:nightly AS builder
WORKDIR /cennznet
COPY . /cennznet

ENV RUST_VERSION nightly-2019-10-14
RUN apt-get update && \
    apt-get -y install apt-utils cmake pkg-config libssl-dev git clang libclang-dev && \
    rustup install $RUST_VERSION && \
    rustup default $RUST_VERSION && \
    rustup target add --toolchain $RUST_VERSION wasm32-unknown-unknown && \
    rustup target add --toolchain $RUST_VERSION x86_64-unknown-linux-musl && \
    mkdir -p /cennznet/.cargo
ENV CARGO_HOME=/cennznet/.cargo
RUN cargo build --release

FROM debian:stretch-slim
LABEL maintainer="support@centrality.ai"

RUN apt-get update && \
    apt-get install -y ca-certificates openssl && \
    mkdir -p /root/.local/share/cennznet && \
    ln -s /root/.local/share/cennznet /data

COPY --from=0 /cennznet/target/release/cennznet /usr/local/bin
EXPOSE 30333 9933 9944
VOLUME ["/data"]
ENTRYPOINT ["/usr/local/bin/cennznet"]
