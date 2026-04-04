# Stage 1: Build
FROM rust:1.81-slim-bookworm as builder

WORKDIR /usr/src/app

# Install build dependencies for llama.cpp
RUN apt-get update && apt-get install -y \
    cmake \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Pre-build dependencies for caching
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Copy source code
COPY src ./src

# Build the application
# We use target-cpu=native if possible, but for generic images we might want something more stable.
# However, the user requested target-cpu=native for performance.
RUN RUSTFLAGS="-C target-cpu=native" cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r app && useradd -r -g app app
USER app

# Copy binary from builder
COPY --from=builder /usr/src/app/target/release/rust_llm_api /app/rust_llm_api

# Expose port
EXPOSE 8080

# Healthcheck
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1

# Environment defaults
ENV PORT=8080 \
    HOST=0.0.0.0 \
    MODEL_PATH=/models/model.gguf \
    API_TOKEN=change_me_immediately \
    CONTEXT_SIZE=4096 \
    DEFAULT_TEMPERATURE=0.7 \
    MAX_CONCURRENCY=1 \
    RATE_LIMIT_REQUESTS=100 \
    RATE_LIMIT_SECONDS=60

ENTRYPOINT ["/app/rust_llm_api"]
