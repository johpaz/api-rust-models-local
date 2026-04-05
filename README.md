# 🚀 LLM API Server con TurboQuant

API HTTP multi-modelo con compresión **TurboQuant** para el cache KV.

## ⚡ Inicio Rápido

### Opción 1: Despliegue Nativo (Recomendado - Máximo Rendimiento)

```bash
# 1. Configurar
cp .env.example .env

# 2. Instalar (compila y configura systemd)
sudo ./scripts/install-native.sh

# 3. Editar configuración
sudo nano /etc/llm-api/.env

# 4. Descargar modelo
./scripts/download-model.sh bartowski/google_gemma-4-E4B-it-GGUF \
    google_gemma-4-E4B-it-Q4_K_M.gguf

# 5. Iniciar servicios
sudo systemctl start llama-server
sudo systemctl start llm-api

# 6. Probar
curl http://localhost:9000/health
```

**Ventajas:**
- ✅ Acceso directo a GPU (Vulkan)
- ✅ Sin overhead de Docker
- ✅ Boot instantáneo
- ✅ Menor uso de memoria

Ver [docs/NATIVE-DEPLOY.md](docs/NATIVE-DEPLOY.md) para guía completa.

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
│   └── Dockerfile
├── llama-server/           ← llama.cpp + TurboQuant
│   ├── ggml/src/
│   │   ├── ggml-turboquant.c   ← Core TurboQuant
│   │   └── ggml-turboquant.h
│   ├── patches/
│   └── Dockerfile
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
├── docker-compose.yml
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
