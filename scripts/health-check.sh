#!/bin/bash
# Verifica que ambos servicios están funcionando

set -e

API_PORT="${API_PORT:-9000}"
API_URL="http://localhost:$API_PORT"

echo "🔍 Verificando servicios..."
echo ""

# Check llama-server
echo "📡 llama-server (interno):"
if curl -sf http://localhost:8080/health &>/dev/null; then
    echo "   ✅ Saludable"
    curl -s http://localhost:8080/health 2>/dev/null | python3 -m json.tool 2>/dev/null || true
else
    echo "   ❌ No disponible"
fi

echo ""

# Check API
echo "🌐 API Server (puerto $API_PORT):"
if curl -sf "$API_URL/health" &>/dev/null; then
    echo "   ✅ Saludable"
    curl -s "$API_URL/health" 2>/dev/null | python3 -m json.tool 2>/dev/null || true
else
    echo "   ❌ No disponible"
fi

echo ""
echo "📊 Estado de contenedores:"
docker compose ps 2>/dev/null || docker ps --filter "name=llm" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
