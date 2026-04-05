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
| **Cache KV turbo3** | ⚠️ No verificado | Crash al iniciar con `--cache-type-k turbo3` — necesita debug |
| **Cache KV q4_0** | ✅ Funcionando | Tipo de cache verificado con GPU Vulkan |
| **GPU Vulkan** | ✅ Activa | AMD RADV REMBRANDT, 35 capas offloaded |

### Configuración verificada funcionando:
```bash
# llama-server con GPU + cache q4_0 (funcionando)
./llama-server/build-native/llama.cpp/build/bin/llama-server \
  --model ./models/google_gemma-4-E4B-it-Q4_K_M.gguf \
  --host 0.0.0.0 --port 8080 \
  --ctx-size 4096 --n-gpu-layers 35 \
  --cache-type-k q4_0 --cache-type-v q4_0
```

### Dependencias del sistema:
- `vulkan-headers`, `vulkan-loader-devel`, `glslc`, `curl-devel`
- Variables Vulkan: `VK_ICD_FILENAMES`, `MESA_VK_WSI=1`

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

### Crash al iniciar con `--cache-type-k turbo3`

Si ves un error como:
```
pre-allocated tensor (cache_k_l8 (view)) in a buffer (Vulkan0) that cannot run the operation (SET_ROWS)
Abortado (`core' generado)
```

**Causa**: Las operaciones SET_ROWS no están implementadas para tipos TurboQuant en el buffer Vulkan.

**Solución**: Usar `q4_0` como cache type (verificado funcionando):
```bash
--cache-type-k q4_0 --cache-type-v q4_0
```

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
