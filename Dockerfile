FROM alpine:latest

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# 安装 Alpine 的 musl 环境和 OpenSSL 静态库
RUN apk add --no-cache \
    curl \
    build-base \
    git \
    file \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    musl-dev

# 安装 Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    && rustup target add x86_64-unknown-linux-musl

WORKDIR /workspace

CMD ["/bin/sh"]