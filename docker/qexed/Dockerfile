# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.98
ARG DEBIAN_VERSION=bookworm

FROM rust:${RUST_VERSION}-${DEBIAN_VERSION} AS builder

WORKDIR /app

ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENV CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt

RUN --mount=type=secret,id=ca_cert,target=/usr/local/share/ca-certificates/qexed-build-ca.crt,required=false \
    set -eux; \
    if [ -s /usr/local/share/ca-certificates/qexed-build-ca.crt ]; then \
        update-ca-certificates; \
    fi

RUN set -eux; \
    find /etc/apt -type f \( -name "*.list" -o -name "*.sources" \) \
        -exec sed -i "s|https://deb.debian.org|http://deb.debian.org|g" {} +; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl-dev \
        pkg-config; \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release -p qexed --bin qexed \
    && cp /app/target/release/qexed /tmp/qexed

FROM debian:${DEBIAN_VERSION}-slim AS runtime-cpu

ARG QEXED_UID=10001
ARG QEXED_GID=10001

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

RUN set -eux; \
    find /etc/apt -type f \( -name "*.list" -o -name "*.sources" \) \
        -exec sed -i "s|https://deb.debian.org|http://deb.debian.org|g" {} +; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        libssl3; \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN groupadd --system --gid "${QEXED_GID}" qexed \
    && useradd --system \
        --uid "${QEXED_UID}" \
        --gid qexed \
        --home-dir /app \
        --shell /usr/sbin/nologin \
        qexed \
    && mkdir -p /app/config /app/cache /app/run /app/log

COPY --from=builder /tmp/qexed /usr/local/bin/qexed

RUN chown -R qexed:qexed /app /usr/local/bin/qexed

USER qexed

EXPOSE 25565/tcp
EXPOSE 25565/udp

VOLUME ["/app/config", "/app/cache", "/app/log", "/app/run"]

ENTRYPOINT ["/usr/local/bin/qexed"]
