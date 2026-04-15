#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# Arrancar el API en modo Backend B (layer-streaming)
# Sin llama-server — inferencia nativa Rust, VRAM mínima
#
# Uso:
#   ./scripts/start-streaming.sh [modelo.gguf]
#
# Ejemplo:
#   ./scripts/start-streaming.sh google_gemma-4-E4B-it-Q4_K_M.gguf
#
# Si no se especifica modelo, usa google_gemma-4-E4B-it-Q4_K_M.gguf
# ═══════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colores
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

echo ""
echo -e "${BLUE}╔══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  Layer-Streaming Backend (Backend B)                ║${NC}"
echo -e "${BLUE}║  Rust nativo — sin llama-server — VRAM mínima       ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════╝${NC}"
echo ""

# ─────────────────────────────────────────────────────────────
# Config
# ─────────────────────────────────────────────────────────────
MODEL_NAME="${1:-google_gemma-4-E4B-it-Q4_K_M.gguf}"
MODEL_PATH="$PROJECT_ROOT/models/$MODEL_NAME"
LAYERS_DIR="$PROJECT_ROOT/models/layers/$(basename "$MODEL_NAME" .gguf)"
API_PORT="${API_PORT:-3001}"
API_BIN="$PROJECT_ROOT/api/target/release/rust_llm_api"
STREAMER_BIN="$PROJECT_ROOT/api/layer-streamer/target/release/layer-streamer"

# ─────────────────────────────────────────────────────────────
# Validaciones
# ─────────────────────────────────────────────────────────────
if [ ! -f "$MODEL_PATH" ]; then
    echo -e "${RED}❌ Modelo no encontrado: $MODEL_PATH${NC}"
    echo ""
    echo "Modelos disponibles:"
    ls -lh "$PROJECT_ROOT/models/"*.gguf 2>/dev/null \
        | awk '{print "   " $5 "  " $NF}' \
        | sed "s|$PROJECT_ROOT/models/||"
    exit 1
fi

if [ ! -f "$STREAMER_BIN" ]; then
    echo -e "${YELLOW}⚙️  Compilando layer-streamer (release)...${NC}"
    cd "$PROJECT_ROOT/api/layer-streamer" && cargo build --release --bin layer-streamer 2>&1 | tail -3
    cd "$PROJECT_ROOT"
    echo -e "${GREEN}✅ layer-streamer compilado${NC}"
fi

if [ ! -f "$API_BIN" ]; then
    echo -e "${YELLOW}⚙️  Compilando rust_llm_api (release)...${NC}"
    cd "$PROJECT_ROOT/api" && cargo build --release --bin rust_llm_api 2>&1 | tail -3
    cd "$PROJECT_ROOT"
    echo -e "${GREEN}✅ rust_llm_api compilado${NC}"
fi

MODEL_SIZE=$(du -h "$MODEL_PATH" | cut -f1)
echo -e "   Modelo:      ${YELLOW}$MODEL_NAME${NC} ($MODEL_SIZE)"
echo -e "   Layers dir:  ${YELLOW}$LAYERS_DIR${NC}"
echo -e "   API puerto:  ${YELLOW}$API_PORT${NC}"
echo ""

# ─────────────────────────────────────────────────────────────
# Split del modelo (si no se hizo antes)
# ─────────────────────────────────────────────────────────────
if [ ! -f "$LAYERS_DIR/model_index.json" ]; then
    echo -e "${CYAN}━━━ Paso 1/2: Split del modelo ━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}   Primera vez con este modelo — dividiendo en capas...${NC}"
    echo -e "   Destino: $LAYERS_DIR"
    echo -e "   Espacio necesario: ~$MODEL_SIZE"
    echo ""

    mkdir -p "$LAYERS_DIR"

    echo -e "   Iniciando split (puede tardar 2-5 min)..."
    "$STREAMER_BIN" split \
        --model "$MODEL_PATH" \
        --output "$LAYERS_DIR"

    if [ ! -f "$LAYERS_DIR/model_index.json" ]; then
        echo -e "${RED}❌ Split falló — model_index.json no generado${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ Split completo${NC}"
    echo ""
else
    echo -e "${GREEN}✅ Paso 1/2: Capas ya divididas — saltando split${NC}"
    LAYER_COUNT=$(python3 -c "import json; d=json.load(open('$LAYERS_DIR/model_index.json')); print(d['n_layers'])" 2>/dev/null || echo "?")
    echo -e "   Capas disponibles: ${YELLOW}$LAYER_COUNT${NC}"
    echo ""
fi

# ─────────────────────────────────────────────────────────────
# Verificar el índice
# ─────────────────────────────────────────────────────────────
echo -e "${CYAN}━━━ Info del modelo ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
"$STREAMER_BIN" info --model "$MODEL_PATH" 2>/dev/null | grep -E "Architecture|Layers|Embedding|Vocab|Attention" || true
echo ""

# ─────────────────────────────────────────────────────────────
# Matar API anterior si existe
# ─────────────────────────────────────────────────────────────
EXISTING=$(pgrep -f "rust_llm_api" 2>/dev/null | head -1)
if [ -n "$EXISTING" ]; then
    echo -e "${YELLOW}⚠️  Deteniendo API anterior (PID: $EXISTING)...${NC}"
    kill "$EXISTING" 2>/dev/null
    sleep 1
fi

# ─────────────────────────────────────────────────────────────
# Arrancar API en modo layer-streaming
# ─────────────────────────────────────────────────────────────
echo -e "${CYAN}━━━ Paso 2/2: Arrancando API Backend B ━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

export INFERENCE_BACKEND=layer_streaming
export LAYER_STREAMING_MODEL="$MODEL_PATH"
export LAYER_STREAMING_LAYERS_DIR="$LAYERS_DIR"
export API_PORT="$API_PORT"
export RUST_LOG="${RUST_LOG:-info}"

LOG_FILE="/tmp/llm-api-streaming.log"

cd "$PROJECT_ROOT"
"$API_BIN" > "$LOG_FILE" 2>&1 &
API_PID=$!
disown

echo -e "   PID:  ${YELLOW}$API_PID${NC}"
echo -e "   Logs: ${YELLOW}tail -f $LOG_FILE${NC}"
echo ""

# Esperar a que cargue (el modelo puede tardar unos segundos)
echo -ne "   Esperando que el backend cargue"
for i in $(seq 1 30); do
    sleep 1
    echo -n "."
    if curl -sf "http://localhost:$API_PORT/api/stream/status" 2>/dev/null | grep -q '"loaded":true'; then
        echo ""
        echo ""
        echo -e "${GREEN}✅ Backend B listo!${NC}"
        break
    fi
    if ! kill -0 "$API_PID" 2>/dev/null; then
        echo ""
        echo -e "${RED}❌ El proceso terminó inesperadamente${NC}"
        echo "Últimas líneas del log:"
        tail -20 "$LOG_FILE"
        exit 1
    fi
done
echo ""

# ─────────────────────────────────────────────────────────────
# Estado final
# ─────────────────────────────────────────────────────────────
STATUS=$(curl -sf "http://localhost:$API_PORT/api/stream/status" 2>/dev/null || echo '{"loaded":false}')
LOADED=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('loaded','?'))" 2>/dev/null || echo "?")
VOCAB=$(echo "$STATUS"  | python3 -c "import sys,json; print(json.load(sys.stdin).get('vocab_size','?'))" 2>/dev/null || echo "?")
LAYERS=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('n_layers','?'))" 2>/dev/null || echo "?")

echo -e "${BLUE}╔══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  API Backend B — Listo para pruebas                 ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "   URL base:    ${GREEN}http://localhost:$API_PORT${NC}"
echo -e "   Modelo:      ${YELLOW}$MODEL_NAME${NC}"
echo -e "   Backend:     ${YELLOW}layer_streaming${NC} (loaded=$LOADED)"
echo -e "   Capas:       ${YELLOW}$LAYERS${NC}"
echo -e "   Vocab:       ${YELLOW}$VOCAB${NC}"
echo ""
echo -e "${CYAN}Pruebas rápidas:${NC}"
echo ""
echo -e "  # Estado del backend"
echo -e "  ${YELLOW}curl http://localhost:$API_PORT/api/stream/status | python3 -m json.tool${NC}"
echo ""
echo -e "  # Listar modelos"
echo -e "  ${YELLOW}curl http://localhost:$API_PORT/v1/models | python3 -m json.tool${NC}"
echo ""
echo -e "  # Inferencia (completion)"
echo -e "  ${YELLOW}curl -X POST http://localhost:$API_PORT/v1/completions \\${NC}"
echo -e "  ${YELLOW}  -H 'Content-Type: application/json' \\${NC}"
echo -e "  ${YELLOW}  -d '{\"model\":\"$MODEL_NAME\",\"prompt\":\"Hola, ¿qué es la IA?\",\"max_tokens\":50}' \\${NC}"
echo -e "  ${YELLOW}  | python3 -m json.tool${NC}"
echo ""
echo -e "  # Inferencia (chat)"
echo -e "  ${YELLOW}curl -X POST http://localhost:$API_PORT/v1/chat/completions \\${NC}"
echo -e "  ${YELLOW}  -H 'Content-Type: application/json' \\${NC}"
echo -e "  ${YELLOW}  -d '{\"messages\":[{\"role\":\"user\",\"content\":\"Explica los transformers\"}],\"max_tokens\":60}' \\${NC}"
echo -e "  ${YELLOW}  | python3 -m json.tool${NC}"
echo ""
echo -e "  # Ver logs en vivo"
echo -e "  ${YELLOW}tail -f $LOG_FILE${NC}"
echo ""
echo -e "  # Detener"
echo -e "  ${YELLOW}kill $API_PID${NC}  o  ${YELLOW}pkill -f rust_llm_api${NC}"
echo ""
