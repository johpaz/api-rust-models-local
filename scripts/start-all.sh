#!/bin/bash
# ═══════════════════════════════════════════════════════
# Iniciar todo: llama-server + UI server
# ═══════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
ENV_FILE="$PROJECT_ROOT/.env"

echo ""
echo "┌─────────────────────────────────────────────────────────┐"
echo "│           LLM Server - Sin Rust API                     │"
echo "│                                                         │"
echo "│  llama-server → :8080 (modelo, inferencia)             │"
echo "│  UI Server    → :3000 (UI, lista modelos, switch)      │"
echo "└─────────────────────────────────────────────────────────┘"
echo ""

# Load env
if [ -f "$ENV_FILE" ]; then
    set -a
    source "$ENV_FILE"
    set +a
fi

# Kill existing
pkill -f "llama-server" 2>/dev/null
pkill -f "serve-ui.py" 2>/dev/null
sleep 1

# Scan models
echo "📋 Escaneando modelos..."
bash "$SCRIPT_DIR/scan-models.sh"

# Start llama-server
echo ""
echo "🚀 Iniciando llama-server..."
bash "$SCRIPT_DIR/start-llama-server.sh" &
sleep 5

# Check llama-server
if curl -sf http://localhost:8080/health > /dev/null 2>&1; then
    echo "✅ llama-server OK en :8080"
else
    echo "⚠️  llama-server aún cargando..."
fi

# Start UI server
echo ""
echo "🌐 Iniciando UI server..."
python3 "$SCRIPT_DIR/serve-ui.py" &
sleep 2

echo ""
echo "═══════════════════════════════════════════════════════"
echo "  ✅ Todo listo!"
echo ""
echo "  🖥️  UI:       http://localhost:3001"
echo "  🧠  llama:    http://localhost:8080"
echo "  📁  models:   http://localhost:3001/models.json"
echo ""
echo "  Para detener:  pkill -f 'llama-server'; pkill -f 'serve-ui.py'"
echo "═══════════════════════════════════════════════════════"

# Wait
wait
