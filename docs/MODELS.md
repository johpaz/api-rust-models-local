# 📦 Modelos Soportados

## Modelos Recomendados (≤30B)

| Modelo | Params | Activos | Tamaño GGUF | RAM Mínima | Uso Ideal |
|--------|--------|---------|------------|-----------|----------|
| **Gemma 4 E4B** | 7.5B | 7.5B | ~4.5 GB | ~6 GB | Chat, resumen rápido |
| **Gemma 4 31B** | 30.7B | 30.7B | ~19 GB | ~22 GB | Documentos largos |
| **Nemotron Cascade 2** | 30B | 3B (MoE) | ~12 GB | ~14 GB | Código, razonamiento |
| **Qwen 3.5 35B** | 35B | 3B (MoE) | ~20 GB | ~22 GB | Multi-idioma, agentic |

## Descarga de Modelos

Usa el script incluido:

```bash
# Gemma 4 E4B (recomendado para empezar)
./scripts/download-model.sh bartowski/google_gemma-4-E4B-it-GGUF \
    google_gemma-4-E4B-it-Q4_K_M.gguf

# Gemma 4 31B (mejor calidad)
./scripts/download-model.sh bartowski/google_gemma-4-31B-it-GGUF \
    google_gemma-4-31B-it-Q4_K_M.gguf

# Nemotron Cascade 2 (eficiente MoE)
./scripts/download-model.sh bartowski/nvidia_Nemotron-Cascade-2-30B-A3B-GGUF \
    nvidia_Nemotron-Cascade-2-30B-A3B-IQ2_M.gguf

# Qwen 3.5 35B (multi-idioma)
./scripts/download-model.sh bartowski/Qwen_Qwen3.5-35B-A3B-GGUF \
    Qwen_Qwen3.5-35B-A3B-Q4_K_M.gguf
```

## Formato GGUF

Todos los modelos deben estar en formato **GGUF** con cuantización:
- `Q4_K_M` — Balance calidad/tamaño (recomendado)
- `Q5_K_M` — Mejor calidad, más grande
- `IQ2_M`, `IQ3_S` — Máxima compresión
- `Q8_0` — Máxima calidad

## Cambio Rápido de Modelo

```bash
# Cambiar modelo en .env
nano .env  # Editar MODEL_NAME

# Reiniciar servidor
pkill -f "llama-server"
./scripts/start-llama-server.sh
```

## Modelos Multi-Modales

Los modelos modernos como **Qwen 3.5** y **Gemma 4** soportan nativamente:
- ✅ Texto → Texto
- 🔄 Imagen → Texto (requiere soporte en llama.cpp)
- 🔄 Audio → Texto (requiere modelo Whisper separado)

La API tiene endpoints preparados para todas las modalidades.
