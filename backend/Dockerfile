# ── Stage 1: Build ────────────────────────────────────────────────────────────
FROM --platform=linux/amd64 rust:1.86-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy everything and build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --bin api

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
FROM --platform=linux/amd64 debian:bookworm-slim

WORKDIR /app

# Runtime dependencies only
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/api ./api

# FC sets FC_SERVER_PORT; fallback to 3000
ENV PORT=3000
EXPOSE 3000

CMD ["./api"]
