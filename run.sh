#!/bin/bash
# ═══════════════════════════════════════════════════════
# Iniciar todo: llama-server + archivos estáticos
# ═══════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "📋 Escaneando modelos..."
bash "$SCRIPT_DIR/scripts/scan-models.sh"

# Copiar models.json al directorio examples para acceso directo
cp "$SCRIPT_DIR/models/models.json" "$SCRIPT_DIR/examples/models.json"

echo ""
echo "🌐 Para ver la UI, abre en tu navegador:"
echo "   file://$SCRIPT_DIR/examples/vision-template.html"
echo ""
echo "   O si el navegador bloquea fetch() (CORS):"
echo "   cd $SCRIPT_DIR/examples && python3 -m http.server 3001"
echo "   http://localhost:3001/vision-template.html"
echo ""
echo "🧠 llama-server ya corre en :8080"
echo ""
echo "Para cambiar modelo:"
echo "   ./scripts/switch-model.sh <modelo.gguf>"
echo ""
