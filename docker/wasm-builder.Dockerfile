# Rust-musl builder with updated nightly and wasm32-unknown-unknown target support
FROM ekidd/rust-musl-builder
COPY init.sh init.sh
RUN ./init.sh
