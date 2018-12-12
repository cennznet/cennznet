FROM alpine:edge AS builder
LABEL maintainer="developers@centrality.ai"
LABEL description="This is the build stage for cennznet-node. Here we create the binary."

RUN apk add build-base \
    cmake \
    linux-headers \
    openssl-dev \
    cargo \
    clang-libs

ENV CARGO_HOME=/root/.cargo
ARG PROFILE=release
RUN USER=root cargo new --bin cennznet-node
WORKDIR /cennznet-node
