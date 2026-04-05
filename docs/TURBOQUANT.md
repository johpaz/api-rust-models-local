# 🧠 TurboQuant — Compresión de KV Cache

Basado en el paper de Google Research:
> **"TurboQuant: Online Vector Quantization with Near-optimal Distortion Rate"**
> arXiv:2504.19874

## ¿Qué es?

TurboQuant es un algoritmo que **comprime el cache KV** durante la inferencia del LLM,
reduciendo la memoria necesaria para mantener contexto largo.

## ¿Cómo Funciona?

```
Vector KV x ∈ R^d
    │
    ├── 1. Extraer norma: γ = ||x||
    ├── 2. Normalizar: x̂ = x/γ
    ├── 3. Rotación aleatoria: WHT + signos aleatorios
    │      → Induce distribución Gaussiana i.i.d.
    ├── 4. Cuantización escalar Lloyd-Max
    │      → Centroides pre-computados para N(0,1)
    └── 5. Output: índices empaquetados (2-4 bits) + norma γ
```

## Tipos de Cache

| Tipo | Bits | Centroides | Memoria/32 valores | Uso recomendado |
|------|------|-----------|-------------------|----------------|
| `f16` | 16 | - | 64 bytes | Sin compresión |
| `q8_0` | 8 | 256 | 36 bytes | Compresión ligera |
| `q4_0` | 4 | 16 | 20 bytes | Balance general |
| **`turbo2`** | **2** | **4** | **12 bytes** | Máxima compresión |
| **`turbo3`** | **3** | **8** | **16 bytes** | **Calidad neutral** ⭐ |
| **`turbo4`** | **4** | **16** | **20 bytes** | Alta precisión |

## Benchmarks

### Memoria del KV Cache

| Contexto | FP16 | q4_0 | turbo3 | Ahorro |
|----------|------|------|--------|--------|
| 4,096 tokens | ~500 MB | ~125 MB | ~94 MB | **5.3x** |
| 16,384 tokens | ~2 GB | ~500 MB | ~375 MB | **5.3x** |
| 65,536 tokens | ~8 GB | ~2 GB | ~1.5 GB | **5.3x** |
| 131,072 tokens | ~16 GB | ~4 GB | ~3 GB | **5.3x** |

### Impacto en Calidad

| Bits | Perplejidad | LongBench | Observación |
|------|------------|-----------|-------------|
| 3.5 (turbo3) | ≈ FP16 | ≈ FP16 | **Neutral** ⭐ |
| 3.0 | +0.01 | -0.3% | Mínimo |
| 2.5 | +0.03 | -1.2% | Marginal |

## Configuración

### En docker-compose.yml

```yaml
environment:
  - LLAMA_ARG_CACHE_TYPE_K=turbo3
  - LLAMA_ARG_CACHE_TYPE_V=turbo3
```

### En .env

```bash
CACHE_TYPE_K=turbo3
CACHE_TYPE_V=turbo3
```

### Valores Disponibles

```
f16, q8_0, q4_0, q5_0, q5_1, q2_k, q3_k, q4_k, q5_k, q6_k,
turbo2, turbo3, turbo4
```

## Implementación Técnica

### Archivos

| Archivo | Función |
|---------|---------|
| `llama-server/ggml/src/ggml-turboquant.c` | Core: WHT + Lloyd-Max |
| `llama-server/ggml/src/ggml-turboquant.h` | API pública |
| `llama-server/ggml/include/ggml.h` | Tipos `GGML_TYPE_TURBO2/3/4` |

### Algoritmo

1. **Walsh-Hadamard Transform (WHT):** Rotación ortogonal que "gaussianiza" las coordenadas
2. **Lloyd-Max Quantizer:** Cuantización escalar óptima para N(0,1)
   - Centroides y boundaries pre-computados offline
   - Sin calibración en runtime

### Lloyd-Max Centroides

**2-bit (4 niveles):**
```
[-1.5104, -0.4527, 0.4527, 1.5104]
```

**3-bit (8 niveles):**
```
[-2.1553, -1.3454, -0.7556, -0.2500,
  0.2500,  0.7556,  1.3454,  2.1553]
```

**4-bit (16 niveles):**
```
[-2.5183, -2.0325, -1.6472, -1.3193, -1.0235, -0.7482, -0.4855, -0.2305,
  0.2305,  0.4855,  0.7482,  1.0235,  1.3193,  1.6472,  2.0325,  2.5183]
```

## Referencias

- **Paper:** https://arxiv.org/abs/2504.19874
- **Google Blog:** https://research.google/blog/turboquant-redefining-ai-efficiency-with-extreme-compression/
- **turboquant_plus:** https://github.com/TheTom/turboquant_plus
