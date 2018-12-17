FROM rustlang/rust:nightly
RUN apt update && apt install -y clang