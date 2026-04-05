#!/bin/bash
# Descarga un modelo GGUF desde HuggingFace
#
# Uso:
#   ./scripts/download-model.sh <repo_id> <filename>
#
# Ejemplo:
#   ./scripts/download-model.sh bartowski/google_gemma-4-E4B-it-GGUF google_gemma-4-E4B-it-Q4_K_M.gguf

set -e

REPO_ID="$1"
FILENAME="$2"

if [ -z "$REPO_ID" ] || [ -z "$FILENAME" ]; then
    echo "Uso: $0 <repo_id> <filename>"
    echo ""
    echo "Ejemplo:"
    echo "  $0 bartowski/google_gemma-4-E4B-it-GGUF google_gemma-4-E4B-it-Q4_K_M.gguf"
    exit 1
fi

# Check if huggingface-cli is installed
if ! command -v huggingface-cli &> /dev/null; then
    echo "❌ huggingface-cli no está instalado."
    echo "Instálalo con: pip install huggingface_hub"
    echo "O con: pipx install huggingface_hub"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODELS_DIR="$(dirname "$SCRIPT_DIR")/models"
mkdir -p "$MODELS_DIR"

echo "📦 Descargando modelo..."
echo "   Repositorio: $REPO_ID"
echo "   Archivo: $FILENAME"
echo "   Destino: $MODELS_DIR/"
echo ""

huggingface-cli download "$REPO_ID" "$FILENAME" --local-dir "$MODELS_DIR"

echo ""
echo "✅ Modelo descargado: $MODELS_DIR/$FILENAME"
echo ""
echo "📋 Tamaño: $(du -h "$MODELS_DIR/$FILENAME" | cut -f1)"
echo ""
echo "🔧 Para usarlo, agrega a tu .env:"
echo "   MODEL_NAME=$FILENAME"
echo ""
echo "🚀 Luego ejecuta: docker compose up -d"
