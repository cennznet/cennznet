FROM cennznet/rust-builder AS builder
WORKDIR /cennznet
COPY . /cennznet
RUN cd /cennznet/runtime/wasm && \
      cargo +nightly build --target=wasm32-unknown-unknown --release && \
      wasm-gc target/wasm32-unknown-unknown/release/cennznet_runtime.wasm target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm
RUN cargo +nightly build --release

FROM debian:stretch-slim
LABEL maintainer="support@centrality.ai"

RUN apt-get update && apt-get install -y ca-certificates openssl && \
        mkdir -p /root/.local/share/cennznet && \
      ln -s /root/.local/share/cennznet /data

COPY --from=0 /cennznet/target/release/cennznet /usr/local/bin
EXPOSE 30333 9933 9944
VOLUME ["/data"]
ENTRYPOINT ["/usr/local/bin/cennznet"]
