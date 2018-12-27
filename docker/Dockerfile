FROM debian:stretch-slim
LABEL maintainer="developers@centrality.ai"

RUN apt update && apt install -y ca-certificates \
    openssl

RUN rm -rf /usr/lib/python* && \
	mkdir -p /root/.local/share/Substrate && \
	ln -s /root/.local/share/Substrate /data

EXPOSE 30333 9933 9944
VOLUME ["/data"]

ARG PROFILE=release
COPY target/$PROFILE/cennznet /usr/local/bin

CMD ["/usr/local/bin/cennznet"]
