# -- Stage 1: Builder -- #
FROM rust:slim AS builder

WORKDIR /usr/src/lumisync

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release --package lumisync_server && \
    cargo install sqlx-cli && \
    sqlx database create --database-url sqlite:file:lumisync.db

# -- Stage 2: Runtime -- #
FROM debian:bookworm-slim

ARG HOST=${HOST:-0.0.0.0}
ARG PORT=${PORT:-3000}
ARG RUN_MODE=${RUN_MODE:-production}
ARG CONFIG_PATH=${CONFIG_PATH:-/etc/lumisync/production.toml}

WORKDIR /opt/lumisync

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /var/lib/lumisync && chmod 777 /var/lib/lumisync
RUN mkdir -p /etc/lumisync

RUN printf '[server]\nhost = "%s"\nport = "%s"\n\n[database]\nurl = "sqlite:/var/lib/lumisync/lumisync.db"\n' \
    "${HOST}" "${PORT}" > /etc/lumisync/production.toml

COPY --from=builder \
    /usr/src/lumisync/target/release/lumisync_server \
    /usr/local/bin/

COPY --from=builder \
    /usr/src/lumisync/lumisync.db \
    /var/lib/lumisync/lumisync.db

EXPOSE $PORT

ENV RUN_MODE=${RUN_MODE}
ENV CONFIG_PATH=${CONFIG_PATH}

CMD ["lumisync_server"]
