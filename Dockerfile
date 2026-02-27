# syntax=docker/dockerfile:1

FROM rust:1.84-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
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
