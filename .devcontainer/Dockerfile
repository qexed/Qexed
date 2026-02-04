# 使用Ubuntu 22.04作为基础镜像（支持docker-in-docker）
FROM ubuntu:22.04

# 设置非交互式环境变量，避免apt安装时出现交互提示
ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUSTUP_UPDATE_ROOT=https://mirrors.aliyun.com/rustup/rustup \
    RUSTUP_DIST_SERVER=https://mirrors.aliyun.com/rustup

# 配置Ubuntu阿里云镜像源
RUN sed -i 's/archive.ubuntu.com/mirrors.aliyun.com/g' /etc/apt/sources.list && \
    sed -i 's/security.ubuntu.com/mirrors.aliyun.com/g' /etc/apt/sources.list

# 配置Rust国内镜像源（阿里云）
RUN mkdir -p $CARGO_HOME/config.d && \
    echo '[source.crates-io]' > $CARGO_HOME/config.d/mirror.toml && \
    echo 'replace-with = "aliyun"' >> $CARGO_HOME/config.d/mirror.toml && \
    echo '[source.aliyun]' >> $CARGO_HOME/config.d/mirror.toml && \
    echo 'registry = "sparse+https://mirrors.aliyun.com/crates.io-index"' >> $CARGO_HOME/config.d/mirror.toml

# 安装必要的系统包
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    git \
    file \
    pkg-config \
    libssl-dev \
    clang \
    llvm \
    lld \
    cmake \
    ninja-build \
    python3 \
    python3-pip \
    python3-venv \
    golang \
    bash \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 创建虚拟环境
RUN python3 -m venv /opt/venv

# 在虚拟环境中升级pip并设置清华镜像源
RUN /opt/venv/bin/pip install --upgrade pip \
    && /opt/venv/bin/pip config set global.index-url https://mirrors.tuna.tsinghua.edu.cn/pypi/web/simple

# 在虚拟环境中安装scons
RUN /opt/venv/bin/pip install --no-cache-dir scons

# 安装Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://mirrors.aliyun.com/repo/rust/rustup-init.sh | sh -s -- -y

# Ubuntu默认使用glibc，如果需要musl target可以安装musl-tools
# 注意：Ubuntu使用glibc，所以target改为gnu而不是musl
RUN rustup target add x86_64-unknown-linux-gnu
RUN rustup target add wasm32-unknown-unknown

# 设置Go代理
RUN go env -w GO111MODULE=on && go env -w GOPROXY=https://goproxy.cn,direct

# 设置环境变量，确保后续命令使用虚拟环境
ENV PATH="/opt/venv/bin:$PATH"
ENV VIRTUAL_ENV="/opt/venv"

# 清理缓存
RUN apt-get clean && rm -rf /var/cache/apt/* \
    && rm -rf /root/.cache/pip

# 验证安装
RUN rustc --version \
    && cargo --version \
    && go version \
    && scons --version \
    && python3 --version \
    && /opt/venv/bin/pip --version

# 设置工作目录
WORKDIR /workspace

# 默认命令改为bash（与原始Dockerfile一致）
CMD ["/bin/bash"]
