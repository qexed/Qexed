# 多阶段构建基础镜像
FROM rust:1.91-slim-bookworm as builder

# 环境变量配置
ENV RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static
ENV RUSTUP_UPDATE_ROOT=https://mirrors.ustc.edu.cn/rust-static/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH="$CARGO_HOME/bin:$PATH"

# 系统依赖安装（带缓存优化）
RUN --mount=type=cache,target=/usr/share/doc \
    --mount=type=cache,target=/var/cache/apt \
    apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    libpq-dev \
    redis-server \
    sqlite3 \
    git-lfs \
    && rm -rf /var/lib/apt/lists/*

# 用户配置
RUN useradd -ms /bin/bash rust-dev
USER rust-dev
WORKDIR /workspace

# Cargo 配置
COPY .cargo/config.toml $CARGO_HOME/config.d/

# 工具链安装
RUN cargo install --locked \
    cargo-watch \
    cargo-edit \
    loco \
    sea-orm-cli \
    cargo-binstall && \
    cargo binstall cargo-audit cargo-outdated

WORKDIR /project