#!/bin/bash
# ═══════════════════════════════════════════════════════
# Escanear modelos .gguf y generar models.json
# ═══════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
MODELS_DIR="$PROJECT_ROOT/models"
OUTPUT_FILE="$MODELS_DIR/models.json"

# Escanear archivos .gguf (excluir mmproj)
models=()
for f in "$MODELS_DIR"/*.gguf; do
    [ -f "$f" ] || continue
    filename=$(basename "$f")
    # Skip mmproj files
    case "$filename" in
        mmproj*) continue ;;
    esac
    size_bytes=$(stat -c%s "$f" 2>/dev/null || stat -f%z "$f" 2>/dev/null || echo "0")
    size_gb=$(echo "scale=1; $size_bytes / 1073741824" | bc 2>/dev/null || echo "?")
    models+=("{\"id\":\"$filename\",\"name\":\"$filename\",\"size_bytes\":$size_bytes,\"size_human\":\"${size_gb} GB\"}")
done

# Generar JSON
if [ ${#models[@]} -eq 0 ]; then
    echo '{"models":[],"count":0}' > "$OUTPUT_FILE"
else
    echo -n '{"models":[' > "$OUTPUT_FILE"
    for i in "${!models[@]}"; do
        if [ $i -gt 0 ]; then echo -n ',' >> "$OUTPUT_FILE"; fi
        echo -n "${models[$i]}" >> "$OUTPUT_FILE"
    done
    echo -n "],\"count\":${#models[@]}}" >> "$OUTPUT_FILE"
fi

echo "✅ Escaneados ${#models[@]} modelos → $OUTPUT_FILE"
