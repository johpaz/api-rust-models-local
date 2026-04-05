#!/bin/bash
# ═══════════════════════════════════════════════════════
# Benchmark para el LLM API Server
# ═══════════════════════════════════════════════════════
#
# Uso:
#   ./scripts/benchmark.sh
#
# Requiere: servidor corriendo en puerto configurado en .env
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
ENV_FILE="$PROJECT_ROOT/.env"

# Cargar .env
if [ -f "$ENV_FILE" ]; then
    set -a
    source "$ENV_FILE"
    set +a
fi

PORT="${PORT:-8080}"
BASE_URL="http://localhost:$PORT/v1/chat/completions"
MODEL="${MODEL_NAME:-actual}"
N_RUNS="${BENCHMARK_RUNS:-5}"

# Colores
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}╔══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║${NC}  LLM Benchmark                                ${BLUE}║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "   Modelo:    ${YELLOW}$MODEL${NC}"
echo -e "   Puerto:    ${YELLOW}$PORT${NC}"
echo -e "   Runs:      ${YELLOW}$N_RUNS${NC}"
echo ""

# Verificar servidor
if ! curl -sf "http://localhost:$PORT/health" > /dev/null 2>&1; then
    echo -e "${RED}❌ Servidor no disponible en puerto $PORT${NC}"
    echo "   Inicia con: ./scripts/start-llama-server.sh"
    exit 1
fi

echo -e "${GREEN}✅ Servidor disponible${NC}"
echo ""

# ═══════════════════════════════════════════════════════
# Test 1: Prompt corto, respuesta controlada
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}━━━ Test 1: Respuesta corta (50 tokens) ━━━${NC}"

total_prompt_ms=0
total_gen_ms=0
total_tokens=0

for i in $(seq 1 $N_RUNS); do
    RESULT=$(curl -s -X POST "$BASE_URL" \
      -H "Content-Type: application/json" \
      -d '{
        "messages": [{"role": "user", "content": "¿Qué es Rust?"}],
        "max_tokens": 50
      }')

    PROMPT_MS=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['timings']['prompt_ms'])" 2>/dev/null || echo "0")
    GEN_MS=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['timings']['predicted_ms'])" 2>/dev/null || echo "0")
    GEN_N=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['timings']['predicted_n'])" 2>/dev/null || echo "0")

    total_prompt_ms=$(echo "$total_prompt_ms + $PROMPT_MS" | bc)
    total_gen_ms=$(echo "$total_gen_ms + $GEN_MS" | bc)
    total_tokens=$((total_tokens + GEN_N))

    TOKENS_S=$(echo "scale=1; $GEN_N * 1000 / $GEN_MS" | bc 2>/dev/null || echo "?")
    echo -e "   Run $i: ${GEN_MS}ms | ${GEN_N} tok | ${YELLOW}${TOKENS_S} t/s${NC}"
done

avg_prompt=$(echo "scale=1; $total_prompt_ms / $N_RUNS" | bc)
avg_gen=$(echo "scale=1; $total_gen_ms / $N_RUNS" | bc)
avg_tokens=$(echo "$total_tokens / $N_RUNS" | bc)
avg_tps=$(echo "scale=1; $total_tokens * 1000 / $total_gen_ms" | bc)

echo -e "   ${GREEN}Avg: ${avg_gen}ms | ${avg_tokens} tok | ${avg_tps} t/s${NC}"
echo -e "   Prompt avg: ${avg_prompt}ms"
echo ""

# ═══════════════════════════════════════════════════════
# Test 2: Respuesta larga (500 tokens)
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}━━━ Test 2: Respuesta larga (500 tokens) ━━━${NC}"

total_prompt_ms=0
total_gen_ms=0
total_tokens=0

for i in $(seq 1 $N_RUNS); do
    RESULT=$(curl -s -X POST "$BASE_URL" \
      -H "Content-Type: application/json" \
      -d '{
        "messages": [{"role": "user", "content": "Explica detalladamente qué es un sistema operativo, sus componentes principales y cómo gestiona la memoria."}],
        "max_tokens": 500
      }')

    PROMPT_MS=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['timings']['prompt_ms'])" 2>/dev/null || echo "0")
    GEN_MS=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['timings']['predicted_ms'])" 2>/dev/null || echo "0")
    GEN_N=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['timings']['predicted_n'])" 2>/dev/null || echo "0")
    REASONING=$(echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['choices'][0]['message'].get('reasoning_content','')[:50]+'...')" 2>/dev/null || echo "(none)")

    total_prompt_ms=$(echo "$total_prompt_ms + $PROMPT_MS" | bc)
    total_gen_ms=$(echo "$total_gen_ms + $GEN_MS" | bc)
    total_tokens=$((total_tokens + GEN_N))

    TOKENS_S=$(echo "scale=1; $GEN_N * 1000 / $GEN_MS" | bc 2>/dev/null || echo "?")
    echo -e "   Run $i: ${GEN_MS}ms | ${GEN_N} tok | ${YELLOW}${TOKENS_S} t/s${NC}"
    echo -e "          Reasoning: ${REASONING}"
done

avg_prompt=$(echo "scale=1; $total_prompt_ms / $N_RUNS" | bc)
avg_gen=$(echo "scale=1; $total_gen_ms / $N_RUNS" | bc)
avg_tokens=$(echo "$total_tokens / $N_RUNS" | bc)
avg_tps=$(echo "scale=1; $total_tokens * 1000 / $total_gen_ms" | bc)

echo -e "   ${GREEN}Avg: ${avg_gen}ms | ${avg_tokens} tok | ${avg_tps} t/s${NC}"
echo -e "   Prompt avg: ${avg_prompt}ms"
echo ""

# ═══════════════════════════════════════════════════════
# Test 3: Test de reasoning profundo
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}━━━ Test 3: Razonamiento (math + code) ━━━${NC}"

RESULT=$(curl -s -X POST "$BASE_URL" \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [{"role": "user", "content": "Resuelve: (2^10 * 3^5) / 7 y explica paso a paso tu razonamiento"}],
    "max_tokens": 1000
  }')

PROMPT_MS=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['timings']['prompt_ms'])" 2>/dev/null || echo "0")
GEN_MS=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['timings']['predicted_ms'])" 2>/dev/null || echo "0")
GEN_N=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['timings']['predicted_n'])" 2>/dev/null || echo "0")
TOTAL_TOK=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['usage']['total_tokens'])" 2>/dev/null || echo "0")
HAS_REASONING=$(echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); print('YES' if d['choices'][0]['message'].get('reasoning_content','') else 'NO')" 2>/dev/null || echo "?")
REASONING_LEN=$(echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d['choices'][0]['message'].get('reasoning_content','')))" 2>/dev/null || echo "0")

TOKENS_S=$(echo "scale=1; $GEN_N * 1000 / $GEN_MS" | bc 2>/dev/null || echo "?")

echo -e "   Tiempo gen:  ${GEN_MS}ms"
echo -e "   Tokens gen:  ${GEN_N}"
echo -e "   Tokens total: ${TOTAL_TOK}"
echo -e "   Tokens/seg:  ${YELLOW}${TOKENS_S}${NC}"
echo -e "   Reasoning:   ${HAS_REASONING} (${REASONING_LEN} chars)"
echo ""

# ═══════════════════════════════════════════════════════
# Test 4: Concurrent requests (si MAX_CONCURRENCY > 1)
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}━━━ Test 4: Latencia de respuesta rápida ━━━${NC}"

START=$(date +%s%N)
RESULT=$(curl -s -X POST "$BASE_URL" \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [{"role": "user", "content": "Di OK"}],
    "max_tokens": 10
  }')
END=$(date +%s%N)

ELAPSED=$(( (END - START) / 1000000 ))
echo -e "   Tiempo total (curl): ${ELAPSED}ms"
echo ""

# ═══════════════════════════════════════════════════════
# Resumen
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}╔══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║${NC}  Resumen                                      ${BLUE}║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "  Modelo:       ${YELLOW}$MODEL${NC}"
echo -e "  GPU Layers:   ${YELLOW}$GPU_LAYERS${NC}"
echo -e "  Cache K/V:    ${YELLOW}$CACHE_TYPE_K${NC} / ${YELLOW}$CACHE_TYPE_V${NC}"
echo -e "  Context:      ${YELLOW}$CONTEXT_SIZE${NC}"
echo ""
