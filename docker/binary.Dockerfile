FROM debian:stretch-slim
LABEL maintainer="support@centrality.ai"

RUN apt-get update && apt-get install -y ca-certificates \
    openssl

RUN mkdir -p /root/.local/share/Substrate && \
      ln -s /root/.local/share/Substrate /data

EXPOSE 30333 9933 9944
VOLUME ["/data"]

ARG PROFILE=release
COPY ./target/$PROFILE/cennznet /usr/local/bin

ENTRYPOINT ["/usr/local/bin/cennznet"]