# Full-stack container (frontend + API)
# - Builds SvelteKit static site
# - Builds Rust server that serves / (static) and /api (JSON)

FROM node:20-bookworm-slim AS webbuilder
WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY svelte.config.js vite.config.ts tsconfig.json postcss.config.js tailwind.config.js ./
COPY src ./src
COPY static ./static

RUN npm run build


FROM rustlang/rust:nightly-slim AS serverbuilder
WORKDIR /app/server

RUN apt-get update \
  && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
  && rm -rf /var/lib/apt/lists/*

COPY server/Cargo.toml server/Cargo.lock* ./
COPY server/src ./src
COPY server/migrations ./migrations

RUN cargo build --release


FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates libssl3 \
  && rm -rf /var/lib/apt/lists/*

COPY --from=serverbuilder /app/server/target/release/jfnotes-server /app/jfnotes-server
COPY --from=webbuilder /app/build /app/static

ENV RUST_LOG=info
ENV STATIC_DIR=/app/static
EXPOSE 8080

CMD ["/app/jfnotes-server"]
