FROM alpine:latest as builder

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUSTUP_UPDATE_ROOT=https://mirrors.aliyun.com/rustup/rustup \
    RUSTUP_DIST_SERVER=https://mirrors.aliyun.com/rustup
# 配置国内镜像源（阿里云）
RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.aliyun.com/g' /etc/apk/repositories
RUN mkdir -p $CARGO_HOME/config.d && \
    echo '[source.crates-io]' > $CARGO_HOME/config.d/mirror.toml && \
    echo 'replace-with = "aliyun"' >> $CARGO_HOME/config.d/mirror.toml && \
    echo '[source.aliyun]' >> $CARGO_HOME/config.d/mirror.toml && \
    echo 'registry = "sparse+https://mirrors.aliyun.com/crates.io-index"' >> $CARGO_HOME/config.d/mirror.toml
# 安装 Alpine 的 musl 环境和 OpenSSL 静态库（需要配置镜像源）
RUN apk add --no-cache \
    curl \
    build-base \
    git \
    file \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    musl-dev \
    clang \
    llvm \
    lld \
    cmake \
    ninja \
    # 清理缓存
    && rm -rf /var/cache/apk/*

# 安装 Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://mirrors.aliyun.com/repo/rust/rustup-init.sh | sh -s -- -y \
    && rustup target add x86_64-unknown-linux-musl
# 配置cargo镜像
# RUN xxx
# 安装cargo-make库
WORKDIR /workspace

CMD ["/bin/sh"]