#!/bin/bash
# ═══════════════════════════════════════════════════════
# Iniciar llama-server en macOS (Metal backend)
# ═══════════════════════════════════════════════════════
#
# LLM API Server - Developed by @johpaz
# macOS version - Apple Silicon / Intel + Metal GPU
#
# Uso:
#   ./scripts/macos/start-llama-server.sh
#
# Configuración: edita el archivo .env en la raíz del proyecto
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
ENV_FILE="$PROJECT_ROOT/.env"

# ═══════════════════════════════════════════════════════
# Configuración de la aplicación
# ═══════════════════════════════════════════════════════
APP_NAME="LLM API Server"
APP_VERSION="1.0.0"
APP_AUTHOR="@johpaz"

# ═══════════════════════════════════════════════════════
# Banner de inicio
# ═══════════════════════════════════════════════════════
echo ""
echo "┌─────────────────────────────────────────────────────────┐"
echo "│                                                         │"
echo "│   ██╗     ███████╗ █████╗ ██████╗                       │"
echo "│   ██║     ██╔════╝██╔══██╗██╔══██╗                      │"
echo "│   ██║     █████╗  ███████║██████╔╝                      │"
echo "│   ██║     ██╔══╝  ██╔══██║██╔══██╗                      │"
echo "│   ███████╗███████╗██║  ██║██║  ██║                      │"
echo "│   ╚══════╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝                      │"
echo "│                                                         │"
echo "│               S  E  R  V  E  R                          │"
echo "│                    (macOS Metal)                        │"
echo "│                                                         │"
echo "│         Version: ${APP_VERSION}                         │"
echo "│         Author:  ${APP_AUTHOR}                          │"
echo "│                                                         │"
echo "└─────────────────────────────────────────────────────────┘"
echo ""

# ═══════════════════════════════════════════════════════
# Cargar variables de entorno desde .env
# ═══════════════════════════════════════════════════════
if [ -f "$ENV_FILE" ]; then
    echo "📋 Cargando configuración desde .env..."
    set -a
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

# macOS: detectar arquitectura
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    echo "🍎 Detectado: Apple Silicon ($ARCH)"
    echo "   GPU Backend: Metal"
elif [ "$ARCH" = "x86_64" ]; then
    echo "🍎 Detectado: Intel ($ARCH)"
    echo "   GPU Backend: Metal (si GPU compatible)"
fi

# Ruta del binario compilado
LLAMA_BINARY="$PROJECT_ROOT/llama-server/build-macos/llama.cpp/build/bin/llama-server"

# Ruta del modelo
MODEL_PATH="$PROJECT_ROOT/models/$MODEL_NAME"

# ═══════════════════════════════════════════════════════
# Validaciones
# ═══════════════════════════════════════════════════════
echo ""
echo "🔍 Validaciones..."

if [ ! -f "$LLAMA_BINARY" ]; then
    echo "❌ Binario no encontrado: $LLAMA_BINARY"
    echo "Compílalo primero con:"
    echo "  ./scripts/macos/build-llama-server.sh"
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

# Detener proceso anterior si existe
EXISTING_PID=$(pgrep -f "build-macos/llama.cpp/build/bin/llama-server" 2>/dev/null | head -1)
if [ -n "$EXISTING_PID" ]; then
    echo "⚠️  Deteniendo proceso anterior (PID: $EXISTING_PID)..."
    kill "$EXISTING_PID" 2>/dev/null
    sleep 2
    if kill -0 "$EXISTING_PID" 2>/dev/null; then
        kill -9 "$EXISTING_PID" 2>/dev/null
        sleep 1
    fi
    echo "✅ Proceso anterior detenido"
    echo ""
fi

# ═══════════════════════════════════════════════════════
# Mostrar configuración
# ═══════════════════════════════════════════════════════
echo ""
echo "📊 Configuración:"
echo "   Modelo:        $MODEL_NAME"
echo "   Host:          $HOST"
echo "   Puerto:        $PORT"
echo "   Contexto:      $CONTEXT_SIZE tokens"
echo "   GPU Layers:    $GPU_LAYERS"
echo "   Cache K:       $CACHE_TYPE_K"
echo "   Cache V:       $CACHE_TYPE_V"
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
)

# macOS Metal: agregar flash-attn si usa turboquant
if [[ "$CACHE_TYPE_K" == turbo* ]] || [[ "$CACHE_TYPE_V" == turbo* ]]; then
    echo "🔧 TurboQuant detectado: agregando --flash-attn on"
    ARGS+=("--flash-attn" "on")
fi

# ═══════════════════════════════════════════════════════
# Iniciar servidor
# ═══════════════════════════════════════════════════════
LOG_FILE="/tmp/llama.log"

echo ""
echo "🚀 Iniciando llama-server con Metal GPU..."
echo "   Logs: tail -f $LOG_FILE"
echo "   Detener: pkill -f llama-server"
echo "   Health: curl http://localhost:$PORT/health"
echo ""

cd "$PROJECT_ROOT"

"$LLAMA_BINARY" "${ARGS[@]}" > "$LOG_FILE" 2>&1 &
LLAMA_PID=$!
disown

echo "✅ llama-server iniciado (PID: $LLAMA_PID)"
echo ""
echo "⏳ Cargando modelo... espera ~30-60s"
echo "   Verifica con: curl http://localhost:$PORT/health"
