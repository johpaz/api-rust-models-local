#!/bin/bash
# ═══════════════════════════════════════════════════════
# Iniciar llama-server leyendo configuración desde .env
# ═══════════════════════════════════════════════════════
#
# LLM API Server - Developed by @johpaz
#
# Uso:
#   ./scripts/start-llama-server.sh
#
# Configuración: edita el archivo .env en la raíz del proyecto
#

set -e

# ═══════════════════════════════════════════════════════
# Configuración de la aplicación
# ═══════════════════════════════════════════════════════
APP_NAME="LLM API Server"
APP_VERSION="1.0.0"
APP_AUTHOR="@johpaz"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
ENV_FILE="$PROJECT_ROOT/.env"

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

# Ruta del binario compilado
LLAMA_BINARY="$PROJECT_ROOT/llama-server/build-native/llama.cpp/build/bin/llama-server"

# ═══════════════════════════════════════════════════════
# Auto-descubrimiento de modelos
# ═══════════════════════════════════════════════════════
MODELS_DIR="$PROJECT_ROOT/models"

# Si no hay MODEL_NAME en .env, seleccionar primer .gguf alfabéticamente
if [ -z "$MODEL_NAME" ]; then
    echo "🔍 Auto-descubriendo modelos en $MODELS_DIR..."
    # Excluir archivos mmproj (no son modelos principales)
    MODEL_NAME=$(find "$MODELS_DIR" -maxdepth 1 -name "*.gguf" ! -name "mmproj*" -printf "%f\n" 2>/dev/null | sort | head -1)
    if [ -z "$MODEL_NAME" ]; then
        echo "❌ No se encontraron modelos .gguf en $MODELS_DIR"
        echo "Descarga un modelo con:"
        echo "  ./scripts/download-model.sh <repo> <filename>"
        exit 1
    fi
    echo "✅ Modelo seleccionado automáticamente: $MODEL_NAME"
    echo ""
fi

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

# MODEL_NAME ya fue validado en auto-descubrimiento arriba
if [ -z "$MODEL_NAME" ]; then
    echo "❌ No se pudo seleccionar un modelo .gguf"
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
EXISTING_PID=$(pgrep -f "build/bin/llama-server" 2>/dev/null | head -1)
if [ -n "$EXISTING_PID" ]; then
    echo "⚠️  Deteniendo proceso anterior (PID: $EXISTING_PID)..."
    kill "$EXISTING_PID" 2>/dev/null
    sleep 2
    # Verificar que se detuvo
    if kill -0 "$EXISTING_PID" 2>/dev/null; then
        kill -9 "$EXISTING_PID" 2>/dev/null
        sleep 1
    fi
    echo "✅ Proceso anterior detenido"
    echo ""
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
echo "   Proyecto:      LLM API Server (@johpaz)"
echo "   Modelo:        $MODEL_NAME"
echo "   Modelo path:   $MODEL_PATH"
echo "   Host:          $HOST"
echo "   Puerto:        $PORT"
echo "   Contexto:      $CONTEXT_SIZE tokens"
echo "   GPU Layers:    $GPU_LAYERS"
echo "   Cache K:       $CACHE_TYPE_K"
echo "   Cache V:       $CACHE_TYPE_V"
echo "   Modelo size:   $(du -h "$MODEL_PATH" | cut -f1)"
echo ""

# ═══════════════════════════════════════════════════════
# Construir comando con array (maneja espacios en rutas)
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

# Add multimodal projector if available (skip for now - wrong version causes crash)
# MMPROJ_FILE=$(find "$MODELS_DIR" -maxdepth 1 -name "mmproj*.gguf" ! -name "mmproj-F32*" 2>/dev/null | head -1)
# if [ -n "$MMPROJ_FILE" ]; then
#     echo "🖼️  Multimodal projector encontrado: $(basename "$MMPROJ_FILE")"
#     ARGS+=("--mmproj" "$MMPROJ_FILE")
# fi

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

env VK_ICD_FILENAMES="$VK_ICD_FILENAMES" MESA_VK_WSI="$MESA_VK_WSI" \
  "$LLAMA_BINARY" "${ARGS[@]}" > "$LOG_FILE" 2>&1 &
LLAMA_PID=$!
disown

echo "✅ llama-server iniciado (PID: $LLAMA_PID)"
echo ""
echo "⏳ Cargando modelo... espera ~30-60s"
echo "   Verifica con: curl http://localhost:$PORT/health"
