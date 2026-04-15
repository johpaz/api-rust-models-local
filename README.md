# LLM API Server

API HTTP local para modelos LLM en formato GGUF. Motor de inferencia nativo en Rust con layer-streaming (sin dependencias externas).

## Arquitectura

```
api/src/
├── main.rs              ← startup, carga del actor en background
├── config.rs            ← variables de entorno
├── inference.rs         ← actor de inferencia (patrón Actor + canales)
├── engine/mod.rs        ← fachada pública del motor
├── middleware/mod.rs    ← CORS
└── routes/
    ├── mod.rs           ← AppState + build_router()
    ├── models.rs        ← listar y cambiar modelos
    ├── health.rs        ← salud y gestión del actor
    └── chat.rs          ← completions (batch + SSE streaming)

api/layer-streamer/      ← crate de inferencia GGUF layer-by-layer
models/                  ← archivos .gguf + capas pre-divididas
```

**Flujo de inferencia:**
```
Cliente HTTP → Axum (puerto 3001) → canal mpsc → Actor thread
                                                  (dueño exclusivo de StreamingForward)
                                                  ↓ token a token
                                               SSE / batch response
```

## Inicio rápido

```bash
# 1. Configurar
cp .env.example .env
# Editar LAYER_STREAMING_MODEL y LAYER_STREAMING_LAYERS_DIR

# 2. Iniciar
cd api
cargo run --release
```

El servidor arranca **inmediatamente** en el puerto 3001. El modelo se carga en background — mientras tanto `/health` y `/v1/models` ya responden.

## Configuración (.env)

| Variable | Default | Descripción |
|----------|---------|-------------|
| `API_PORT` | `3001` | Puerto del servidor |
| `HOST` | `0.0.0.0` | Interfaz de red |
| `INFERENCE_BACKEND` | `layer_streaming` | Motor de inferencia |
| `LAYER_STREAMING_MODEL` | — | Ruta al archivo `.gguf` |
| `LAYER_STREAMING_LAYERS_DIR` | — | Directorio con capas pre-divididas |
| `RUST_LOG` | `info` | Nivel de logging (`info`, `debug`, `trace`) |

Ver [`.env.example`](.env.example) para todas las opciones.

## Endpoints

| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/health` | GET | Estado del servidor y del actor |
| `/v1/models` | GET | Lista modelos disponibles (compatible OpenAI) |
| `/models.json` | GET | Lista modelos con metadatos |
| `/api/switch` | POST | Cambiar modelo activo |
| `/api/rescan` | POST | Re-escanear directorio de modelos |
| `/api/stream/status` | GET | Estado del actor (modelo cargado, capas, vocab) |
| `/api/stream/load` | POST | Cargar modelo con rutas explícitas |
| `/v1/completions` | POST | Completar texto (legacy) |
| `/v1/chat/completions` | POST | Chat — soporta `stream: true` (SSE) |

CORS abierto — cualquier origen puede llamar la API.

## Uso desde una UI externa

### Listar modelos
```bash
curl http://localhost:3001/v1/models
```

### Cambiar modelo activo
```bash
curl -X POST http://localhost:3001/api/switch \
  -H "Content-Type: application/json" \
  -d '{"model": "Qwen_Qwen3.5-35B-A3B-Q4_K_M.gguf"}'
```
> El modelo debe tener su directorio de capas en `models/layers/{nombre_sin_.gguf}/`

### Chat (batch)
```bash
curl -X POST http://localhost:3001/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
    "messages": [{"role": "user", "content": "Hola"}],
    "max_tokens": 256
  }'
```

### Chat con streaming SSE
```bash
curl -X POST http://localhost:3001/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
    "messages": [{"role": "user", "content": "Hola"}],
    "stream": true
  }'
```

### Python (SDK OpenAI)
```python
from openai import OpenAI

client = OpenAI(base_url="http://localhost:3001/v1", api_key="local")

response = client.chat.completions.create(
    model="google_gemma-4-E4B-it-Q4_K_M.gguf",
    messages=[{"role": "user", "content": "Hola"}],
    max_tokens=256,
)
print(response.choices[0].message.content)
```

### Verificar estado del actor
```bash
curl http://localhost:3001/api/stream/status
# {"loaded":true,"model":"google_gemma-4-E4B-it-Q4_K_M.gguf","vocab_size":256000,"n_layers":43}
```

## Modelos disponibles

| Modelo | Tamaño | Estado |
|--------|--------|--------|
| google_gemma-4-E4B-it-Q4_K_M.gguf | 5.1 GB | Capas pre-divididas listas |
| google_gemma-4-31B-it-Q4_K_M.gguf | 19 GB | — |
| Qwen_Qwen3.5-35B-A3B-Q4_K_M.gguf | 20 GB | — |
| nvidia_Nemotron-Cascade-2-30B-A3B-IQ2_M.gguf | 17 GB | — |

## Layer-streaming

El motor carga y procesa el modelo **capa por capa**, sin necesidad de tener todo el modelo en RAM o VRAM simultáneamente:

1. Para cada token: carga capa → forward (Attention + FFN) → descarta capa
2. Soporta arquitecturas Llama, Gemma, Mistral, Qwen
3. KV cache completo en RAM
4. Pre-dividir capas mejora la velocidad de carga:

```bash
# Las capas pre-divididas se ubican en:
models/layers/{nombre_del_modelo_sin_.gguf}/
#   layer_000.bin, layer_001.bin, ...
```

## Compilar

```bash
cd api
cargo build --release
# Binario: api/target/release/rust_llm_api
```
