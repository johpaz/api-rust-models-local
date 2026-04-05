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

### Opción 2: Docker (Alternativa)

```bash
# 1. Configurar
cp .env.example .env

# 2. Descargar modelo
./scripts/download-model.sh bartowski/google_gemma-4-E4B-it-GGUF \
    google_gemma-4-E4B-it-Q4_K_M.gguf

# 3. Configurar modelo en .env
echo "MODEL_NAME=google_gemma-4-E4B-it-Q4_K_M.gguf" >> .env

# 4. Levantar
docker compose up -d --build

# 5. Probar
curl http://localhost:9000/health
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
│   ├── NATIVE-DEPLOY.md    ← Guía despliegue nativo (¡NUEVO!)
│   └── DEPLOY.md           ← Guía Docker
├── scripts/                ← Utilidades
│   ├── install-native.sh   ← Instalador nativo (¡NUEVO!)
│   ├── build-api.sh        ← Build API Rust (¡NUEVO!)
│   └── build-llama-server.sh ← Build llama.cpp (¡NUEVO!)
├── systemd/                ← Servicios systemd (¡NUEVO!)
│   ├── llama-server.service
│   └── llm-api.service
└── .env.example
```

## 🔧 Comandos

### Despliegue Nativo

| Comando | Descripción |
|---------|-------------|
| `sudo ./scripts/install-native.sh` | Instalar todo |
| `sudo systemctl start llama-server` | Iniciar inference engine |
| `sudo systemctl start llm-api` | Iniciar API |
| `sudo journalctl -u llama-server -f` | Ver logs |

### Docker

| Comando | Descripción |
|---------|-------------|
| `docker compose up -d --build` | Levantar todo |
| `docker compose down` | Detener |
| `./scripts/download-model.sh <repo> <file>` | Descargar modelo |
| `./scripts/health-check.sh` | Verificar servicios |

## 📡 API Endpoints

| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/health` | GET | Estado del sistema |
| `/v1/models` | GET | Modelos disponibles |
| `/v1/chat/completions` | POST | Generar texto (SSE) |
| `/v1/images/generations` | POST | Generar imágenes |
| `/v1/audio/transcriptions` | POST | Transcribir audio |

## 📖 Documentación

- **[API Completa](docs/API.md)** — Endpoints, ejemplos curl/Python/Node
- **[Deploy](docs/DEPLOY.md)** — Docker, variables, troubleshooting
- **[TurboQuant](docs/TURBOQUANT.md)** — Algoritmo, benchmarks
- **[Modelos](docs/MODELS.md)** — Modelos soportados y descargas

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
