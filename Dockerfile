# syntax=docker/dockerfile:1
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY configai /app/configai
RUN chmod +x /app/configai

VOLUME /app/config

ENV RUST_LOG=info

EXPOSE 3000

ENTRYPOINT ["/app/configai"]
CMD ["serve"]
