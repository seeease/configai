# syntax=docker/dockerfile:1

FROM rust:1.84-bookworm AS builder

ARG TARGETARCH

RUN if [ "$TARGETARCH" = "arm64" ]; then \
      apt-get update && apt-get install -y gcc-aarch64-linux-gnu && \
      rustup target add aarch64-unknown-linux-gnu; \
    fi

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN if [ "$TARGETARCH" = "arm64" ]; then \
      CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
      cargo build --release --target aarch64-unknown-linux-gnu && \
      cp target/aarch64-unknown-linux-gnu/release/configai /app/configai; \
    else \
      cargo build --release && \
      cp target/release/configai /app/configai; \
    fi

# ---- 运行阶段 ----
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/configai /app/configai

VOLUME /app/config

ENV RUST_LOG=info

EXPOSE 3000

ENTRYPOINT ["/app/configai"]
CMD ["serve"]
