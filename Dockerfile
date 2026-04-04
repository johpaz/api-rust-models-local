# Stage 1: Build
FROM rust:1.86-slim-bookworm as builder

WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./

RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

COPY src ./src

RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r app && useradd -r -g app app
USER app

COPY --from=builder /usr/src/app/target/release/rust_llm_api /app/rust_llm_api

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:3000/health || exit 1

ENV PORT=3000 \
    HOST=0.0.0.0 \
    LLAMA_SERVER_URL=http://llama-server:8080 \
    API_TOKEN=change_me_immediately \
    DEFAULT_TEMPERATURE=0.7 \
    RATE_LIMIT_REQUESTS=100 \
    RATE_LIMIT_SECONDS=60

ENTRYPOINT ["/app/rust_llm_api"]
