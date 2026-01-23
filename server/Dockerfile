# Builder
# Use a nightly toolchain to satisfy edition2024 deps
FROM rustlang/rust:nightly-slim AS builder
WORKDIR /app

# Install build deps
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

# Build
COPY . .
RUN cargo build --release

# Runtime
FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/beck-server /app/beck-server
# Ensure static dir exists (actual assets are mounted at runtime via compose volume)
RUN mkdir -p /app/static
ENV RUST_LOG=info
ENV STATIC_DIR=/app/static
CMD ["/app/beck-server"]
