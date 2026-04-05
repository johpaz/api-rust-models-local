# 📊 Estado del Proyecto (Abril 2026)

Estado verificado de cada componente del LLM API Server.

## ✅ Componentes FuncIONANDO

### llama-server (Inferencia con GPU)

| Propiedad | Valor |
|-----------|-------|
| **Estado** | ✅ Funcionando |
| **Versión** | llama.cpp b8668 |
| **Binario** | `llama-server/build-native/llama.cpp/build/bin/llama-server` |
| **Tamaño** | ~71MB |
| **Puerto** | 8080 |
| **GPU** | ✅ AMD RADV REMBRANDT (Vulkan) |
| **GPU Layers** | 35/43 offloaded |
| **Modelo probado** | `google_gemma-4-E4B-it-Q4_K_M.gguf` (5.1GB) |
| **Health** | `{"status":"ok"}` |

#### Comandos de inicio verificados:
```bash
setsid env \
  VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json:/usr/share/vulkan/icd.d/intel_icd.x86_64.json \
  MESA_VK_WSI=1 \
  "./llama-server/build-native/llama.cpp/build/bin/llama-server" \
  --model "./models/google_gemma-4-E4B-it-Q4_K_M.gguf" \
  --host 0.0.0.0 --port 8080 \
  --ctx-size 4096 --n-gpu-layers 35 \
  --cache-type-k q4_0 --cache-type-v q4_0 \
  > /tmp/llama.log 2>&1 &
disown
```

#### Logs de verificación GPU:
```
load_tensors: offloading 34 repeating layers to GPU
load_tensors: offloaded 35/43 layers to GPU
...
llama_memory_breakdown_print: |   - Vulkan0 (Graphics (RADV REMBRANDT)) | 20066 = 10999 + ...
```

---

### TurboQuant Patches

| Propiedad | Valor |
|-----------|-------|
| **Estado** | ✅ Compilados |
| **Script** | `llama-server/patches/apply-turboquant.py` (idempotente) |
| **Wrapper** | `llama-server/patches/apply.sh` |
| **Compatible** | llama.cpp b8668 |
| **Tipos** | TURBO2, TURBO3, TURBO4 |

**Nota**: Los parches están compilados pero el cache KV usa `q4_0` en producción. TurboQuant para cache KV requiere ajuste adicional en las funciones `ggml_quantize_turbo` / `ggml_dequantize_turbo`.

---

### Build Scripts

| Script | Estado | Función |
|--------|--------|---------|
| `scripts/build-llama-server.sh` | ✅ Funcionando | Compila llama.cpp con Vulkan |
| `scripts/build-api.sh` | ✅ Funcionando | Compila Rust API en release |
| `scripts/install-native.sh` | ✅ Listo | Instalación completa + systemd |

---

### systemd Services

| Archivo | Estado | Función |
|---------|--------|---------|
| `systemd/llama-server.service` | ✅ Creado | Servicio para inference engine |
| `systemd/llm-api.service` | ✅ Creado | Servicio para Rust API |

---

## ⚠️ Componentes con Problemas

### Rust API Server

| Propiedad | Valor |
|-----------|-------|
| **Estado** | ⚠️ Compilado, no iniciado |
| **Binario** | `api/target/release/rust_llm_api` |
| **Problema** | Conflictos de puerto (Node.js ocupa 3000) |
| **Config** | Requiere `MODEL_PATH` en .env |

**Para iniciar**:
```bash
cd api
./target/release/rust_llm_api
# Requiere puerto libre y .env configurado
```

---

## 📦 Dependencias del Sistema (Fedora)

| Paquete | Versión | Estado |
|---------|---------|--------|
| rustc | 1.94.1 | ✅ |
| cargo | 1.94.1 | ✅ |
| cmake | 3.31.11 | ✅ |
| glslc | shaderc v2026.1 | ✅ |
| vulkan-headers | - | ✅ |
| vulkan-loader-devel | - | ✅ |
| curl-devel | - | ✅ |
| mesa-vulkan-drivers | 25.3.6 | ✅ |

---

## 🗂️ Ubicación de Archivos

```
/home/johnpaez/Documentos/llm/api rust model local/
├── llama-server/
│   ├── build-native/
│   │   └── llama.cpp/
│   │       └── build/
│   │           └── bin/
│   │               └── llama-server          ← Binario principal (71MB)
│   └── patches/
│       ├── apply.sh                          ← Wrapper bash
│       └── apply-turboquant.py               ← Script Python idempotente
├── api/
│   ├── target/release/
│   │   └── rust_llm_api                      ← Binario API
│   └── .env                                  ← Config API
├── models/
│   └── google_gemma-4-E4B-it-Q4_K_M.gguf     ← Modelo verificado
├── systemd/
│   ├── llama-server.service
│   └── llm-api.service
└── docs/
    ├── NATIVE-DEPLOY.md                      ← Guía principal
    ├── STATUS.md                             ← Este archivo
    └── DEPLOY.md                             ← Guía Docker
```

---

## 📝 Notas Técnicas

### Vulkan Configuration
Las variables de entorno necesarias para Vulkan:
```bash
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json:/usr/share/vulkan/icd.d/intel_icd.x86_64.json
MESA_VK_WSI=1
GGML_VK_VISIBLE_DEVICES=0
```

### CMake Configuration
```bash
cmake -B build \
  -DCMAKE_BUILD_TYPE=Release \
  -DLLAMA_BUILD_TESTS=OFF \
  -DLLAMA_BUILD_EXAMPLES=OFF \
  -DLLAMA_BUILD_SERVER=ON \
  -DGGML_VULKAN=ON \
  -DBUILD_SHARED_LIBS=OFF
```

### TurboQuant Limitation
Los tipos TURBO2/3/4 están definidos y compilados, pero las funciones de quantización pueden necesitar ajuste para funcionar correctamente con el cache KV. Se usa `q4_0` como fallback funcional.

---

## 🔄 Últimas Acciones

| Fecha | Acción |
|-------|--------|
| 2026-04-05 | ✅ Compilación exitosa llama-server b8668 + Vulkan |
| 2026-04-05 | ✅ GPU AMD RADV REMBRANDT detectada y funcionando |
| 2026-04-05 | ✅ 35 capas offloaded a GPU verificadas |
| 2026-04-05 | ✅ Parches TurboQuant hechos idempotentes (Python) |
| 2026-04-05 | ✅ Documentación actualizada |
