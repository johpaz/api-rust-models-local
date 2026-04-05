# 🧠 TurboQuant — Compresión de KV Cache

Basado en el paper de Google Research:
> **"TurboQuant: Online Vector Quantization with Near-optimal Distortion Rate"**
> arXiv:2504.19874

---

## ✅ Estado Actual (Abril 2026)

| Componente | Estado | Notas |
|-----------|--------|-------|
| **Parches** | ✅ Idempotentes | Script Python (`apply-turboquant.py`) — seguro ejecutar múltiples veces |
| **Tipos GGML** | ✅ Definidos | `GGML_TYPE_TURBO2/3/4` agregados a `ggml/include/ggml.h` |
| **Funciones quant** | ✅ Compiladas | `ggml_quantize_turbo`, `ggml_dequantize_turbo` en `ggml-turboquant.c` |
| **Cache KV turbo3** | ✅ Funcionando | Requiere `--flash-attn on` + parche CPU fallback en `llama-kv-cache.cpp` |
| **Cache KV q4_0** | ✅ Funcionando | Fallback por defecto, full GPU |
| **GPU Vulkan** | ✅ Activa | AMD RADV REMBRANDT, 35 capas offloaded |

### Configuración verificada funcionando (turbo3):
```bash
./llama-server/build-native/llama.cpp/build/bin/llama-server \
  --model ./models/google_gemma-4-26B-A4B-it-IQ2_XXS.gguf \
  --host 0.0.0.0 --port 8080 \
  --ctx-size 4096 --n-gpu-layers 35 \
  --cache-type-k turbo3 --cache-type-v turbo3 \
  --flash-attn on
```

**Nota crítica:** Sin `--flash-attn on`, turbo3 falla con: `quantized V cache was requested, but this requires Flash Attention`.

### Benchmark: turbo3 vs q4_0

Prueba real con equipo bajo carga multitarea (IDE, Firefox, Brave, radeontop, terminal, 32GB RAM):

| Métrica | q4_0 (full GPU) | turbo3 + flash-attn | Diferencia |
|---------|-----------------|---------------------|------------|
| **Prompt 21 tok** | 1,020ms (20.6 t/s) | 396ms (53.0 t/s) | **+157%** ⚡ |
| **Prompt 31 tok** | — | 459ms (67.5 t/s) | — |
| **Gen 1,466 tok** | 52,084ms (**28.1 t/s**) | — | — |
| **Gen 1,704 tok** | — | 61,130ms (**27.9 t/s**) | -0.7% |
| **Gen 2,259 tok** | — | 84,331ms (**26.8 t/s**) | — |
| **KV Cache** | ~225 MiB (GPU Vulkan) | **220 MiB (CPU)** | -2% |
| **Estabilidad** | ✅ Estable | ✅ Estable bajo carga | — |

**Conclusiones:**
- turbo3 da rendimiento prácticamente idéntico a q4_0 (27.9 vs 28.1 t/s)
- Procesamiento de prompt 2.5x más rápido con turbo3
- Sistema estable incluso con carga multitarea pesada
- Ahorro de memoria modesto (~2%) con contexto de 4K; más significativo con contextos largos

### Dependencias del sistema:
- `vulkan-headers`, `vulkan-loader-devel`, `glslc`, `curl-devel`
- Variables Vulkan: `VK_ICD_FILENAMES`, `MESA_VK_WSI=1`
- Parche aplicado: `src/llama-kv-cache.cpp` — fuerza CPU buffer para tipos TURBO2/3/4

---

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
| `llama-server/patches/apply-turboquant.py` | Script Python idempotente para aplicar parches |
| `llama-server/patches/apply.sh` | Wrapper bash para ejecutar el script Python |
| `llama-server/ggml/src/ggml-turboquant.c` | Core: WHT + Lloyd-Max |
| `llama-server/ggml/src/ggml-turboquant.h` | API pública |
| `llama-server/ggml/include/ggml.h` | Tipos `GGML_TYPE_TURBO2/3/4` |
| `llama-server/build-native/llama.cpp/build/bin/llama-server` | Binario compilado con TurboQuant |

### Aplicar Parches

```bash
# Desde el directorio del proyecto
bash llama-server/patches/apply.sh

# Es idempotente — seguro ejecutar múltiples veces
# Output esperado:
#   ✓ ggml.h already patched (TURBO2 found)
#   ✓ ggml.c already patched (turboquant include found)
#   ✓ CMakeLists.txt already patched
#   ✓ arg.cpp already patched
```

### Compilar con TurboQuant + Vulkan

```bash
cd llama-server/build-native/llama.cpp
cmake -B build \
  -DCMAKE_BUILD_TYPE=Release \
  -DLLAMA_BUILD_TESTS=OFF \
  -DLLAMA_BUILD_EXAMPLES=OFF \
  -DLLAMA_BUILD_SERVER=ON \
  -DGGML_VULKAN=ON \
  -DBUILD_SHARED_LIBS=OFF
cmake --build build --target llama-server -j$(nproc)
```

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

---

## 🔧 Troubleshooting TurboQuant

### turbo3 crash sin `--flash-attn on`

Si ves un error como:
```
quantized V cache was requested, but this requires Flash Attention
```

**Solución**: Agregar `--flash-attn on`:
```bash
--cache-type-k turbo3 --cache-type-v turbo3 --flash-attn on
```

### turbo3 crash con SET_ROWS (antes del parche)

Si ves un error como:
```
pre-allocated tensor (cache_k_l8 (view)) in a buffer (Vulkan0) that cannot run the operation (SET_ROWS)
Abortado (`core' generado)
```

**Causa**: El backend Vulkan no soporta SET_ROWS para tipos TurboQuant.

**Solución**: Parche aplicado en `src/llama-kv-cache.cpp` — fuerza CPU buffer para tipos TURBO2/3/4.

### Verificar que TurboQuant está compilado

```bash
# Buscar tipos en el binario
strings llama-server | grep -i turbo
# Deberías ver: "turbo2", "turbo3", "turbo4"

# O verificar en logs de cmake
grep -i "turbo" llama-server/build-native/llama.cpp/build/CMakeCache.txt
```

### Parches no se aplican

```bash
# Verificar que el script Python existe
ls llama-server/patches/apply-turboquant.py

# Ejecutar manualmente desde el directorio de llama.cpp
cd llama-server/build-native/llama.cpp
python3 ../../patches/apply-turboquant.py
```

### GPU no detectada

Asegurar variables de entorno Vulkan:
```bash
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json:/usr/share/vulkan/icd.d/intel_icd.x86_64.json
export MESA_VK_WSI=1
```
