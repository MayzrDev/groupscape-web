# syntax=docker/dockerfile:1
###############################################
# Backend Image
###############################################
FROM rust:1.89 AS builder
WORKDIR /app

# Build dependencies against dummy sources first so this layer (and the cargo
# registry/target caches below) survive an app-only code change instead of
# recompiling every dependency from scratch on every deploy.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin \
    && echo "fn main() {}" > src/main.rs \
    && echo "" > src/lib.rs \
    && echo "fn main() {}" > src/bin/seed.rs
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked

COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    touch src/main.rs src/lib.rs src/bin/seed.rs \
    && cargo build --release --locked \
    && cp target/release/server /app/server-bin \
    && cp target/release/seed /app/seed-bin

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y openssl ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/server-bin ./server
COPY --from=builder /app/seed-bin ./seed
COPY ./docker-entrypoint.sh ./

ENTRYPOINT ["/app/docker-entrypoint.sh"]
CMD ["/app/server"]
