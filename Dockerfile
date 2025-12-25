# Build stage
FROM rust:1.75-slim-bookworm as builder

WORKDIR /usr/src/app

# Install dependencies required for building (e.g., pkg-config, libssl-dev)
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# 1. Cache dependencies
COPY Cargo.toml Cargo.lock ./
# Create a dummy src/main.rs to satisfy cargo
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release

# 2. Build actual application
# Remove the dummy build artifacts so the actual code is recompiled
RUN rm -f target/release/deps/bybit_arbitrage_bot*
COPY src ./src
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /usr/local/bin

# Install runtime dependencies (OpenSSL, CA Certificates)
RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/bybit-arbitrage-bot .
COPY precision_cache.json .

CMD ["./bybit-arbitrage-bot"]
