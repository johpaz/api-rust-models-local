#!/bin/bash
# ═══════════════════════════════════════════════════════
# Cambiar modelo: mata llama-server y reinicia con nuevo modelo
# ═══════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
ENV_FILE="$PROJECT_ROOT/.env"
MODEL_NAME="${1}"
LOCK_FILE="/tmp/llama-server-switch.lock"

if [ -z "$MODEL_NAME" ]; then
    echo "❌ Uso: $0 <nombre_modelo.gguf>"
    exit 1
fi

MODEL_PATH="$PROJECT_ROOT/models/$MODEL_NAME"

if [ ! -f "$MODEL_PATH" ]; then
    echo "❌ Modelo no encontrado: $MODEL_PATH"
    exit 1
fi

# Prevent concurrent switches
if [ -f "$LOCK_FILE" ]; then
    echo "⚠️  Cambio de modelo en progreso..."
    exit 1
fi
touch "$LOCK_FILE"
trap "rm -f $LOCK_FILE" EXIT

echo "🔄 Cambiando modelo a: $MODEL_NAME"

# Kill existing llama-server
EXISTING_PID=$(pgrep -f "build/bin/llama-server" 2>/dev/null | head -1)
if [ -n "$EXISTING_PID" ]; then
    echo "⏹️  Deteniendo llama-server (PID: $EXISTING_PID)..."
    kill "$EXISTING_PID" 2>/dev/null
    sleep 2
    if kill -0 "$EXISTING_PID" 2>/dev/null; then
        kill -9 "$EXISTING_PID" 2>/dev/null
        sleep 1
    fi
fi

# Load env vars
if [ -f "$ENV_FILE" ]; then
    set -a
    source "$ENV_FILE"
    set +a
fi

# Defaults
PORT="${PORT:-8080}"
HOST="${HOST:-0.0.0.0}"
CONTEXT_SIZE="${CONTEXT_SIZE:-8192}"
GPU_LAYERS="${GPU_LAYERS:-35}"
CACHE_TYPE_K="${LLAMA_ARG_CACHE_TYPE_K:-q4_0}"
CACHE_TYPE_V="${LLAMA_ARG_CACHE_TYPE_V:-q4_0}"

LLAMA_BINARY="$PROJECT_ROOT/llama-server/build-native/llama.cpp/build/bin/llama-server"

ARGS=(
    "--model" "$MODEL_PATH"
    "--host" "$HOST"
    "--port" "$PORT"
    "--ctx-size" "$CONTEXT_SIZE"
    "--n-gpu-layers" "$GPU_LAYERS"
    "--cache-type-k" "$CACHE_TYPE_K"
    "--cache-type-v" "$CACHE_TYPE_V"
)

# Add flash-attn for turboquant
if [[ "$CACHE_TYPE_K" == turbo* ]] || [[ "$CACHE_TYPE_V" == turbo* ]]; then
    ARGS+=("--flash-attn" "on")
fi

# Update .env with new MODEL_NAME
sed -i "s/^MODEL_NAME=.*/MODEL_NAME=$MODEL_NAME/" "$ENV_FILE" 2>/dev/null

echo "🚀 Iniciando llama-server con: $MODEL_NAME"

env VK_ICD_FILENAMES="${VK_ICD_FILENAMES:-/usr/share/vulkan/icd.d/radeon_icd.x86_64.json:/usr/share/vulkan/icd.d/intel_icd.x86_64.json}" \
    MESA_VK_WSI="${MESA_VK_WSI:-1}" \
    "$LLAMA_BINARY" "${ARGS[@]}" > /tmp/llama.log 2>&1 &
LLAMA_PID=$!
disown

echo "✅ llama-server iniciado (PID: $LLAMA_PID)"
echo "📊 Esperando carga del modelo..."

# Wait for health
for i in $(seq 1 60); do
    if curl -sf "http://localhost:$PORT/health" > /dev/null 2>&1; then
        echo "✅ Modelo cargado y listo!"
        echo "{\"status\":\"ok\",\"model\":\"$MODEL_NAME\",\"pid\":$LLAMA_PID}"
        exit 0
    fi
    sleep 2
done

echo "❌ Timeout: modelo no cargó en 120s"
echo "{\"status\":\"error\",\"model\":\"$MODEL_NAME\"}"
exit 1
