# llama-server con TurboQuant integrado
# Descarga llama.cpp + aplica parches TurboQuant durante el build

FROM debian:bookworm AS builder

RUN apt-get update && apt-get install -y \
    cmake \
    build-essential \
    git \
    patch \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# 1. Clonar llama.cpp
RUN git clone --depth 1 https://github.com/ggerganov/llama.cpp .

# 2. Aplicar parches TurboQuant
COPY patches/ /src/patches/
RUN /src/patches/apply.sh

# 3. Build con optimizaciones CPU
RUN cmake -B build \
    -DCMAKE_BUILD_TYPE=Release \
    -DLLAMA_BUILD_TESTS=OFF \
    -DLLAMA_BUILD_EXAMPLES=OFF \
    -DLLAMA_BUILD_SERVER=ON \
    -DLLAMA_AVX=ON \
    -DLLAMA_AVX2=ON \
    -DLLAMA_AVX512=ON \
    -DLLAMA_FMA=ON \
    -DLLAMA_F16C=ON \
    -DBUILD_SHARED_LIBS=OFF \
    && cmake --build build --target llama-server -j$(nproc)

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libgomp1 \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/build/bin/llama-server /usr/local/bin/llama-server

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["llama-server"]
