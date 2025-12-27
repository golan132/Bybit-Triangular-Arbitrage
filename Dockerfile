# Base stage for cargo-chef
FROM rust:1-slim-bookworm AS chef
# Install dependencies required for building (e.g., pkg-config, libssl-dev)
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef
WORKDIR /app

# Planner stage
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the cached layer
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim AS runtime

WORKDIR /usr/local/bin

# Install runtime dependencies (OpenSSL, CA Certificates)
RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/bybit-arbitrage-bot .
COPY precision_cache.json .

CMD ["./bybit-arbitrage-bot"]
