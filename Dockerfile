# syntax=docker/dockerfile:1

# ---- 构建阶段 ----
FROM rust:1.84-bookworm AS builder

WORKDIR /app

# 先复制依赖清单，利用 Docker 缓存
COPY Cargo.toml Cargo.lock ./

# 创建空 src 用于预编译依赖
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release && rm -rf src target/release/deps/configai*

# 复制真正的源码并构建
COPY src ./src
RUN cargo build --release

# ---- 运行阶段 ----
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/configai /app/configai

VOLUME /app/config

ENV RUST_LOG=info

EXPOSE 3000

ENTRYPOINT ["/app/configai"]
CMD ["serve"]
