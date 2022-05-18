FROM  rustlang/rust:nightly AS builder
WORKDIR /cennznet
COPY . /cennznet

ARG RUST_VERSION=1.57.0
ARG RUST_NIGHTLY=nightly-2021-12-23
RUN apt-get update && \
    apt-get -y install apt-utils cmake pkg-config libssl-dev git clang libclang-dev && \
    rustup uninstall nightly && \
    rustup install $RUST_VERSION && \
    rustup install $RUST_NIGHTLY && \
    rustup default $RUST_VERSION && \
    rustup target add --toolchain $RUST_NIGHTLY wasm32-unknown-unknown && \
    rustup target add --toolchain $RUST_VERSION x86_64-unknown-linux-musl && \
    mv /usr/local/rustup/toolchains/nightly* /usr/local/rustup/toolchains/nightly-x86_64-unknown-linux-gnu && \
    mkdir -p /cennznet/.cargo
ENV CARGO_HOME=/cennznet/.cargo
RUN cargo build --release

FROM debian:stretch-slim
LABEL maintainer="support@centrality.ai"

RUN apt-get update && \
    apt-get install -y ca-certificates openssl curl && \
    mkdir -p /root/.local/share/cennznet && \
    ln -s /root/.local/share/cennznet /data

COPY --from=0 /cennznet/target/release/cennznet /usr/local/bin
# copy in genesis files
COPY --from=0 /cennznet/genesis /cennznet/genesis
# copy in wasm blob
COPY --from=0 /cennznet/target/release/wbuild/cennznet-runtime/cennznet_runtime.compact.wasm /cennznet
EXPOSE 30333 9933 9944
VOLUME ["/data"]
ENTRYPOINT ["/usr/local/bin/cennznet"]
