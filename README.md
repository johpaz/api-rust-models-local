# 🚀 LLM API Server con TurboQuant

API HTTP multi-modelo con compresión **TurboQuant** para el cache KV.

## 🏗️ Diagrama de Infraestructura

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                          Sistema Host (Linux / Fedora)                          │
│                                                                                 │
│  ┌──────────────┐         ┌──────────────────────────────────────────────┐     │
│  │   Cliente    │  HTTP   │          llama-server :8080                  │     │
│  │  curl/Post   │────────▶│  ┌──────────────────────────────────────┐   │     │
│  │  Python/Node │  :8080  │  │  Modelo: google_gemma-4-E4B-it      │   │     │
│  └──────────────┘         │  │  Cache KV: q4_0 (4-bit)             │   │     │
│                           │  │  Context: 4096 tokens               │   │     │
│                           │  │                                      │   │     │
│                           │  │  GPU Offload: 35/43 capas ──────────┼───┼────┐
│                           │  │                                      │   │    │
│                           │  │  TurboQuant: TURBO2/3/4 (compilado)  │   │    │
│                           │  └──────────────────────────────────────┘   │    │
│                           └──────────────────────────────────────────────┘    │
│                                                                              ▼│
│                           ┌───────────────────────────────────────────────────┐│
│                           │              GPU: AMD RADV REMBRANDT              ││
│                           │              Vulkan 1.4.341                       ││
│                           │              /dev/dri/renderD128                  ││
│                           └───────────────────────────────────────────────────┘│
│                                                                                 │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │                     Binarios Compilados (Nativo)                        │  │
│  │  ┌──────────────────────────────────────────────┐  ┌───────────────────┐ │  │
│  │  │ llama-server (71MB)                          │  │ Rust API (Axum)   │ │  │
│  │  │ llama-server/build-native/llama.cpp/build/   │  │ api/target/       │ │  │
│  │  │   bin/llama-server                           │  │   release/        │ │  │
│  │  └──────────────────────────────────────────────┘  │   rust_llm_api    │ │  │
│  │                                                     └───────────────────┘ │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                 │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │                     Modelos GGUF (./models/)                            │  │
│  │  ┌──────────────────────────────────────────────────────────────────┐   │  │
│  │  │ google_gemma-4-E4B-it-Q4_K_M.gguf    (5.1 GB, 7.5B params) ✅   │   │  │
│  │  │ google_gemma-4-31B-it-Q4_K_M.gguf    (19 GB, 30.7B params)      │   │  │
│  │  │ nvidia_Nemotron-Cascade-2-30B-A3B    (17 GB, 30B params)        │   │  │
│  │  │ Qwen_Qwen3.5-35B-A3B-Q4_K_M.gguf     (20 GB, 35B params)        │   │  │
│  │  └──────────────────────────────────────────────────────────────────┘   │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                 │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │                     Variables Vulkan Requeridas                         │  │
│  │  VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json:...   │  │
│  │  MESA_VK_WSI=1                                                          │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## ⚡ Inicio Rápido

### Opción 1: Despliegue Nativo (Recomendado - Máximo Rendimiento)

**Estado**: ✅ Verificado funcionando con GPU Vulkan (Abril 2026)

```bash
# 1. Iniciar llama-server directamente (puerto 8080)
cd "/api rust model local"

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

# 2. Esperar carga del modelo (~20s)
sleep 20

# 3. Verificar
curl http://localhost:8080/health
# Respuesta: {"status":"ok"}
```

**Verificar GPU activa:**
```bash
grep -i "offload" /tmp/llama.log
# Deberías ver: "offloaded 35/43 layers to GPU"
```

**Ventajas:**
- ✅ Acceso directo a GPU (Vulkan) - verificado con AMD RADV REMBRANDT
- ✅ Sin overhead de Docker
- ✅ Boot instantáneo
- ✅ Menor uso de memoria
- ✅ TurboQuant parches compilados (idempotentes)

Ver [docs/NATIVE-DEPLOY.md](docs/NATIVE-DEPLOY.md) para guía completa y systemd.

---

## 🔧 Gestión del Servicio

### Comandos Básicos

| Acción | Comando |
|--------|---------|
| **Detener** | `pkill -f "llama-server"` |
| **Ver logs** | `tail -f /tmp/llama.log` |
| **Verificar estado** | `curl http://localhost:8080/health` |
| **Verificar GPU** | `grep -i "offload" /tmp/llama.log` |

### 🔄 Cambiar de Modelo

```bash
# 1. Detener servidor actual
pkill -f "llama-server"
sleep 2

# 2. Iniciar con otro modelo
setsid env \
  VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json:/usr/share/vulkan/icd.d/intel_icd.x86_64.json \
  MESA_VK_WSI=1 \
  "./llama-server/build-native/llama.cpp/build/bin/llama-server" \
  --model "./models/google_gemma-4-26B-A4B-it-IQ2_XXS.gguf" \
  --host 0.0.0.0 --port 8080 \
  --ctx-size 4096 --n-gpu-layers 35 \
  --cache-type-k q4_0 --cache-type-v q4_0 \
  > /tmp/llama.log 2>&1 &
disown

# 3. Esperar carga y verificar
sleep 20 && curl http://localhost:8080/health
```

### 📊 Modelos Disponibles

| Modelo | Params | Tamaño | Comando `--model` |
|--------|--------|--------|-------------------|
| **Gemma 4 E4B** ⭐ | 7.5B | 5.1 GB | `./models/google_gemma-4-E4B-it-Q4_K_M.gguf` |
| Gemma 4 31B | 30.7B | 19 GB | `./models/google_gemma-4-31B-it-Q4_K_M.gguf` |
| Nemotron Cascade 2 | 30B | 17 GB | `./models/nvidia_Nemotron-Cascade-2-30B-A3B-IQ2_M.gguf` |
| Qwen 3.5 35B | 35B | 20 GB | `./models/Qwen_Qwen3.5-35B-A3B-Q4_K_M.gguf` |

### 📝 Logs y Debugging

```bash
# Logs en tiempo real
tail -f /tmp/llama.log

# Verificar GPU activa
grep -iE "vulkan|gpu|offload" /tmp/llama.log
# Esperado: "offloaded 35/43 layers to GPU"

# Ver uso de memoria VRAM
grep "memory breakdown" /tmp/llama.log
# Esperado: "Vulkan0 (Graphics (RADV REMBRANDT)) | 20066 = ..."

# Ver errores
grep -iE "error|fail|abort" /tmp/llama.log

# Ver proceso corriendo
ps aux | grep "[l]lama-server"
```

### ⚙️ Ajustar Capas GPU

| Situación | Valor | Comando |
|-----------|-------|---------|
| **Normal** (7.5B) | 35 capas | `--n-gpu-layers 35` |
| **Poca VRAM** | 20 capas | `--n-gpu-layers 20` |
| **Máxima GPU** (12GB+) | 999 capas | `--n-gpu-layers 999` |
| **Solo CPU** | 0 capas | `--n-gpu-layers 0` |

### ⚙️ Ajustar Contexto

| Valor | Uso RAM | Cuándo usar |
|-------|---------|-------------|
| `--ctx-size 2048` | Menor | Conversaciones cortas, poca RAM |
| `--ctx-size 4096` | Medio | **Default recomendado** |
| `--ctx-size 8192` | Mayor | Documentos largos, más RAM disponible |

### 📥 Descargar Nuevo Modelo

```bash
./scripts/download-model.sh <repo_id> <filename>

# Ejemplo:
./scripts/download-model.sh bartowski/google_gemma-4-E4B-it-GGUF \
    google_gemma-4-E4B-it-Q4_K_M.gguf
```

## 📁 Estructura

```
├── api/                    ← API Rust (Axum)
│   ├── src/
│   ├── Cargo.toml
├── llama-server/           ← llama.cpp + TurboQuant
│   ├── ggml/src/
│   │   ├── ggml-turboquant.c   ← Core TurboQuant
│   │   └── ggml-turboquant.h
│   ├── patches/
├── models/                 ← Modelos GGUF
├── docs/                   ← Documentación
│   ├── NATIVE-DEPLOY.md    ← Guía despliegue nativo
│   ├── TURBOQUANT.md       ← TurboQuant algoritmo
│   └── STATUS.md           ← Estado de componentes
├── scripts/                ← Utilidades
│   ├── install-native.sh   ← Instalador nativo
│   ├── build-api.sh        ← Build API Rust
│   └── build-llama-server.sh ← Build llama.cpp
├── systemd/                ← Servicios systemd
│   ├── llama-server.service
│   └── llm-api.service
└── .env.example
```

## 🔧 Comandos

| Comando | Descripción |
|---------|-------------|
| `pkill -f "llama-server"` | Detener servidor |
| `tail -f /tmp/llama.log` | Ver logs en tiempo real |
| `curl http://localhost:8080/health` | Verificar estado |
| `grep -i "offload" /tmp/llama.log` | Verificar GPU activa |
| `sudo ./scripts/install-native.sh` | Instalación completa + systemd |
| `sudo journalctl -u llama-server -f` | Ver logs systemd (si instalado) |

## 📡 API Endpoints

| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/health` | GET | Estado del sistema |
| `/v1/models` | GET | Modelos disponibles |
| `/v1/chat/completions` | POST | Generar texto (SSE) |
| `/v1/images/generations` | POST | Generar imágenes |
| `/v1/audio/transcriptions` | POST | Transcribir audio |

## 📖 Documentación

- **[NATIVE-DEPLOY](docs/NATIVE-DEPLOY.md)** — Guía completa de despliegue nativo
- **[TURBOQUANT](docs/TURBOQUANT.md)** — Algoritmo, benchmarks, troubleshooting
- **[STATUS](docs/STATUS.md)** — Estado actual de componentes
- **[API](docs/API.md)** — Endpoints, ejemplos
- **[MODELS](docs/MODELS.md)** — Modelos soportados y descargas

## 🧠 Modelos Soportados

| Modelo | Params | Tamaño | RAM Mínima |
|--------|--------|--------|-----------|
| Gemma 4 E4B | 7.5B | ~4.5 GB | ~6 GB |
| Gemma 4 31B | 30.7B | ~19 GB | ~22 GB |
| Nemotron Cascade 2 | 30B | ~12 GB | ~14 GB |
| Qwen 3.5 35B | 35B | ~20 GB | ~22 GB |

## 🏗️ TurboQuant

Compresión del cache KV con el algoritmo de Google Research:
- **2-4 bits/canal** vs FP16
- **5.3x menos memoria** en cache KV
- **Sin calibración** previa
- **Cualquier arquitectura** GGUF ≤30B

Ver [`docs/TURBOQUANT.md`](docs/TURBOQUANT.md) para detalles técnicos.

## 🐳 Variables de Entorno

| Variable | Default | Descripción |
|----------|---------|-------------|
| `MODEL_NAME` | `model.gguf` | Nombre del modelo en `models/` |
| `CONTEXT_SIZE` | `8192` | Tokens de contexto |
| `CACHE_TYPE_K` | `turbo3` | Cuantización cache K |
| `CACHE_TYPE_V` | `turbo3` | Cuantización cache V |
| `API_TOKEN` | *(requerido)* | Token de autenticación |
| `API_PORT` | `9000` | Puerto externo |

Ver [`.env.example`](.env.example) para todas las opciones.

## 📄 Licencia

Basado en:
- **llama.cpp** — MIT License
- **TurboQuant** — Paper Google Research (arXiv:2504.19874)
- **API Rust** — Implementación propia
