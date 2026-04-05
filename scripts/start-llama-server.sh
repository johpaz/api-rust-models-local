#!/bin/bash
# ═══════════════════════════════════════════════════════
# Iniciar llama-server leyendo configuración desde .env
# ═══════════════════════════════════════════════════════
#
# Uso:
#   ./scripts/start-llama-server.sh
#
# Configuración: edita el archivo .env en la raíz del proyecto
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
ENV_FILE="$PROJECT_ROOT/.env"

# ═══════════════════════════════════════════════════════
# Cargar variables de entorno desde .env
# ═══════════════════════════════════════════════════════
if [ -f "$ENV_FILE" ]; then
    echo "📋 Cargando configuración desde .env..."
    set -a  # Auto-exportar variables
    source "$ENV_FILE"
    set +a
else
    echo "⚠️  No se encontró .env en $PROJECT_ROOT"
    echo "Copia .env.example a .env y edita la configuración:"
    echo "  cp .env.example .env"
    exit 1
fi

# ═══════════════════════════════════════════════════════
# Variables con valores por defecto
# ═══════════════════════════════════════════════════════
PORT="${PORT:-8080}"
HOST="${HOST:-0.0.0.0}"
MODEL_NAME="${MODEL_NAME:-}"
CONTEXT_SIZE="${CONTEXT_SIZE:-4096}"
GPU_LAYERS="${GPU_LAYERS:-35}"
CACHE_TYPE_K="${LLAMA_ARG_CACHE_TYPE_K:-q4_0}"
CACHE_TYPE_V="${LLAMA_ARG_CACHE_TYPE_V:-q4_0}"
N_PARALLEL="${MAX_CONCURRENCY:-1}"

# Ruta del binario compilado
LLAMA_BINARY="$PROJECT_ROOT/llama-server/build-native/llama.cpp/build/bin/llama-server"

# Ruta del modelo
MODEL_PATH="$PROJECT_ROOT/models/$MODEL_NAME"

# ═══════════════════════════════════════════════════════
# Validaciones
# ═══════════════════════════════════════════════════════
echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║  Llama Server - Inicio                              ║"
echo "╚══════════════════════════════════════════════════════╝"
echo ""

if [ ! -f "$LLAMA_BINARY" ]; then
    echo "❌ Binario no encontrado: $LLAMA_BINARY"
    echo "Compílalo primero con:"
    echo "  ./scripts/build-llama-server.sh"
    exit 1
fi

if [ -z "$MODEL_NAME" ]; then
    echo "❌ MODEL_NAME no está configurado en .env"
    echo "Agrega: MODEL_NAME=nombre_del_modelo.gguf"
    exit 1
fi

if [ ! -f "$MODEL_PATH" ]; then
    echo "❌ Modelo no encontrado: $MODEL_PATH"
    echo ""
    echo "Modelos disponibles:"
    ls -lh "$PROJECT_ROOT/models/"*.gguf 2>/dev/null | awk '{print "   " $NF " (" $5 ")"}' || echo "   (ninguno)"
    echo ""
    echo "Descarga uno con:"
    echo "  ./scripts/download-model.sh <repo> <filename>"
    exit 1
fi

# Verificar si ya hay un proceso corriendo
if pgrep -f "llama-server" > /dev/null 2>&1; then
    echo "⚠️  Ya hay un proceso llama-server corriendo"
    echo "PID: $(pgrep -f 'llama-server' | head -1)"
    echo ""
    read -p "¿Quieres detenerlo y reiniciar? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        pkill -f "llama-server"
        sleep 2
        echo "✅ Proceso anterior detenido"
    else
        echo "Cancelado"
        exit 0
    fi
fi

# ═══════════════════════════════════════════════════════
# Configuración de Vulkan GPU
# ═══════════════════════════════════════════════════════
export VK_ICD_FILENAMES="${VK_ICD_FILENAMES:-/usr/share/vulkan/icd.d/radeon_icd.x86_64.json:/usr/share/vulkan/icd.d/intel_icd.x86_64.json}"
export MESA_VK_WSI="${MESA_VK_WSI:-1}"

# ═══════════════════════════════════════════════════════
# Mostrar configuración
# ═══════════════════════════════════════════════════════
echo "📊 Configuración:"
echo "   Modelo:        $MODEL_NAME"
echo "   Modelo path:   $MODEL_PATH"
echo "   Host:          $HOST"
echo "   Puerto:        $PORT"
echo "   Contexto:      $CONTEXT_SIZE tokens"
echo "   GPU Layers:    $GPU_LAYERS"
echo "   Cache K:       $CACHE_TYPE_K"
echo "   Cache V:       $CACHE_TYPE_V"
echo "   Paralelo:      $N_PARALLEL"
echo "   Modelo size:   $(du -h "$MODEL_PATH" | cut -f1)"
echo ""

# ═══════════════════════════════════════════════════════
# Construir comando
# ═══════════════════════════════════════════════════════
ARGS=(
    "--model" "$MODEL_PATH"
    "--host" "$HOST"
    "--port" "$PORT"
    "--ctx-size" "$CONTEXT_SIZE"
    "--n-gpu-layers" "$GPU_LAYERS"
    "--cache-type-k" "$CACHE_TYPE_K"
    "--cache-type-v" "$CACHE_TYPE_V"
    "--n-parallel" "$N_PARALLEL"
)

# Agregar flash-attn si usa turboquant (turbo2/3/4)
if [[ "$CACHE_TYPE_K" == turbo* ]] || [[ "$CACHE_TYPE_V" == turbo* ]]; then
    echo "🔧 TurboQuant detectado: agregando --flash-attn on"
    ARGS+=("--flash-attn" "on")
fi

# ═══════════════════════════════════════════════════════
# Iniciar servidor
# ═══════════════════════════════════════════════════════
LOG_FILE="/tmp/llama.log"

echo ""
echo "🚀 Iniciando llama-server..."
echo "   Logs: tail -f $LOG_FILE"
echo "   Detener: pkill -f llama-server"
echo "   Health: curl http://localhost:$PORT/health"
echo ""

cd "$PROJECT_ROOT"
setsid env \
    VK_ICD_FILENAMES="$VK_ICD_FILENAMES" \
    MESA_VK_WSI="$MESA_VK_WSI" \
    "$LLAMA_BINARY" "${ARGS[@]}" > "$LOG_FILE" 2>&1 &

LLAMA_PID=$!
disown $LLAMA_PID

echo "✅ llama-server iniciado (PID: $LLAMA_PID)"
echo ""
echo "⏳ Esperando carga del modelo (~20-30s)..."

# Esperar y verificar
for i in $(seq 1 30); do
    sleep 1
    if curl -sf "http://localhost:$PORT/health" > /dev/null 2>&1; then
        echo ""
        echo "✅ Servidor listo!"
        curl -s "http://localhost:$PORT/health" | python3 -m json.tool 2>/dev/null || curl -s "http://localhost:$PORT/health"
        echo ""
        echo "📋 Para ver GPU activa:"
        echo "   grep -i 'offload' $LOG_FILE"
        exit 0
    fi
done

echo ""
echo "⚠️  El servidor no respondió en 30s"
echo "Verifica los logs:"
echo "   tail -f $LOG_FILE"
