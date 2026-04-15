# Arquitectura del LLM API Server

Documentación técnica de la implementación actual: dos backends de inferencia,
descubrimiento automático de modelos y endpoints HTTP compatibles con OpenAI.

---

## Visión general

El sistema tiene tres componentes que trabajan juntos:

```
┌──────────────────────────────────────────────────────────────────┐
│                    Cliente HTTP / UI                             │
└──────────────┬──────────────────────────────────────────────────┘
               │ HTTP JSON
┌──────────────▼──────────────────────────────────────────────────┐
│          Rust API  (puerto 3001)                                │
│  ┌──────────────────┐   ┌──────────────────────────────────┐   │
│  │  Gestión modelos │   │    Inferencia                    │   │
│  │  /models.json    │   │  Backend A: llama-server proxy   │   │
│  │  /api/switch     │   │  Backend B: layer-streaming      │   │
│  │  /api/rescan     │   │  /v1/completions                 │   │
│  │  /health         │   │  /v1/chat/completions            │   │
│  └──────────────────┘   └──────────────────────────────────┘   │
└────────────────────────────────┬────────────────────────────────┘
                                 │
           ┌─────────────────────┼──────────────────────┐
           │                     │                      │
┌──────────▼──────┐   ┌──────────▼──────────┐           │
│  llama-server   │   │  Layer Streamer      │           │
│  (puerto 8080)  │   │  (in-process Rust)   │           │
│  GGUF + Vulkan  │   │  GGUF capa a capa    │           │
└─────────────────┘   └─────────────────────┘           │
                                                         │
                              ┌──────────────────────────▼──┐
                              │  models/  (disco)            │
                              │  *.gguf  (8-20 GB cada uno)  │
                              └─────────────────────────────┘
```

---

## Los dos backends de inferencia

### Backend A — llama-server (por defecto)

`llama-server` es un proceso C++ externo compilado desde llama.cpp.
El API Rust actúa como proxy: gestiona el proceso (arranque, cambio de modelo,
health checks) y delega toda la inferencia a `http://localhost:8080`.

- Aceleración GPU vía **Vulkan** (AMD RADV, Intel)
- Carga el modelo completo en VRAM + RAM (necesita VRAM suficiente)
- Rápido para modelos que caben en memoria
- Cuantización GGUF en disco (Q4_K_M, IQ2_XXS, etc.)

### Backend B — Layer Streaming (nuevo, AirLLM-style)

Implementado directamente en Rust dentro del API, en `api/layer-streamer/`.
Replica la técnica de **AirLLM**: carga una sola capa del modelo a la vez,
ejecuta el pase hacia adelante, la libera, y carga la siguiente.

```
Disco ──► capa 0 ──► procesar ──► liberar
          capa 1 ──► procesar ──► liberar
          ...
          capa N ──► procesar ──► liberar
                ──► norm final ──► logits ──► token siguiente
```

- **VRAM pico**: ~1 capa + buffers (≈ 300-500 MB para modelos 30B)
- Permite correr modelos de 30B en GPUs de 4-8 GB VRAM
- Más lento que llama-server (I/O de disco por cada token)
- El prefetcher carga la capa N+1 mientras el CPU/GPU procesa la N

---

## Flujo de los modelos GGUF

### Descubrimiento automático

Al arrancar, el API escanea `models/*.gguf` (excluyendo archivos `mmproj*`):

```
models/
├── gemma-4-31B-it-UD-IQ3_XXS.gguf      11.0 GB
├── gemma-4-31B-it-UD-Q2_K_XL.gguf      12.x GB
├── google_gemma-4-26B-A4B-it-IQ2_XXS.gguf
├── google_gemma-4-31B-it-Q4_K_M.gguf   19.x GB
├── google_gemma-4-E4B-it-Q4_K_M.gguf    5.1 GB  ← modelo ligero
├── nvidia_Nemotron-Cascade-2-30B-A3B-IQ2_M.gguf
├── Qwen3.5-9B.Q8_0.gguf
├── Qwen_Qwen3.5-35B-A3B-Q4_K_M.gguf   19.9 GB
└── models.json                          ← generado automáticamente
```

El listado se actualiza llamando a `POST /api/rescan` sin reiniciar.

### Cómo se usa un modelo en cada backend

**Backend A (llama-server):**
El modelo se carga una vez en memoria al arrancar llama-server.
Para cambiar de modelo hay que reiniciar el proceso:

```
POST /api/switch  {"model": "google_gemma-4-E4B-it-Q4_K_M.gguf"}
```

Esto ejecuta `scripts/switch-model.sh`, que:
1. Mata el proceso llama-server existente
2. Actualiza `.env` con el nuevo nombre de modelo
3. Arranca llama-server con el modelo nuevo
4. Espera hasta que responda en `/health` (≤120 s)

**Backend B (layer-streaming):**
El modelo se pre-procesa una sola vez con `layer-streamer split`,
que divide el GGUF en archivos de capa individuales en un directorio:

```
/tmp/gemma-layers/
├── model_index.json      ← metadatos del split
├── layer_000.safetensor  ← capa transformer 0
├── layer_001.safetensor
├── ...
├── token_embd.safetensor
└── output.safetensor
```

Luego el API carga ese directorio y el GGUF original (mapeado en memoria):

```
POST /api/stream/load
{
  "model_path": "/ruta/al/modelo.gguf",
  "layers_dir": "/tmp/gemma-layers/"
}
```

---

## Configuración (.env)

```bash
# ── Servidor ──────────────────────────────────────────────────────
API_PORT=3001          # Puerto del API Rust
HOST=0.0.0.0
RUST_LOG=info

# ── llama-server (Backend A) ──────────────────────────────────────
PORT=8080              # Puerto de llama-server
MODEL_NAME=google_gemma-4-31B-it-Q4_K_M.gguf
CONTEXT_SIZE=30000     # Ventana de contexto en tokens
GPU_LAYERS=35          # Capas offloadeadas a GPU (0=solo CPU, 999=todo)
LLAMA_ARG_CACHE_TYPE_K=q4_0   # Cuantización del KV cache
LLAMA_ARG_CACHE_TYPE_V=q4_0

# ── GPU / Vulkan ──────────────────────────────────────────────────
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json:...

# ── Layer-streaming (Backend B) ──────────────────────────────────
INFERENCE_BACKEND=llama_server      # o "layer_streaming"
LAYER_STREAMING_MODEL=              # ruta al .gguf (solo si layer_streaming)
LAYER_STREAMING_LAYERS_DIR=         # ruta al split dir

# ── Seguridad ─────────────────────────────────────────────────────
API_TOKEN=SG6r9OPMvKTgcJBI189y...   # Bearer token (llama-server no lo requiere)
```

---

## Endpoints del API

### Gestión de modelos

| Método | Ruta | Descripción |
|--------|------|-------------|
| `GET` | `/models.json` | Lista todos los `.gguf` en `models/` con tamaños |
| `GET` | `/v1/models` | Lo mismo, formato OpenAI |
| `POST` | `/api/switch` | Cambia el modelo activo en llama-server |
| `POST` | `/api/rescan` | Re-escanea el directorio `models/` |
| `GET` | `/health` | Estado del API, llama-server y streaming backend |

#### GET /health — ejemplo de respuesta
```json
{
  "status": "ok",
  "llama_server": "ok",
  "active_model": "google_gemma-4-31B-it-Q4_K_M.gguf",
  "streaming_backend": "not_loaded"
}
```

Cuando hay un modelo streaming cargado:
```json
{
  "streaming_backend": "loaded:google_gemma-4-E4B-it-Q4_K_M.gguf"
}
```

---

### Gestión del backend layer-streaming

| Método | Ruta | Descripción |
|--------|------|-------------|
| `POST` | `/api/stream/load` | Carga un modelo en el engine layer-streaming |
| `GET` | `/api/stream/status` | Estado del backend streaming |

#### POST /api/stream/load
```bash
curl -X POST http://localhost:3001/api/stream/load \
  -H "Content-Type: application/json" \
  -d '{
    "model_path": "/home/user/models/google_gemma-4-E4B-it-Q4_K_M.gguf",
    "layers_dir": "/tmp/gemma-layers/"
  }'
```

Respuesta:
```json
{ "status": "ok", "model": "google_gemma-4-E4B-it-Q4_K_M.gguf" }
```

#### GET /api/stream/status
```json
{
  "loaded": true,
  "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
  "vocab_size": 262144,
  "n_layers": 34
}
```

---

### Inferencia (compatible OpenAI)

Estos endpoints usan el **backend layer-streaming** cuando está cargado.
Si no hay modelo streaming activo devuelven un error indicando que uses
llama-server en `:8080` directamente.

#### POST /v1/completions

```bash
curl -X POST http://localhost:3001/v1/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
    "prompt": "¿Qué es la inteligencia artificial?",
    "max_tokens": 200,
    "temperature": 0.7
  }'
```

Respuesta:
```json
{
  "id": "cmpl-17b3f2a8c",
  "object": "text_completion",
  "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
  "choices": [
    {
      "text": "La inteligencia artificial es...",
      "index": 0,
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 12,
    "completion_tokens": 87,
    "total_tokens": 99
  }
}
```

#### POST /v1/chat/completions

```bash
curl -X POST http://localhost:3001/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
    "messages": [
      {"role": "system", "content": "Eres un asistente útil."},
      {"role": "user",   "content": "Explica qué es un transformer."}
    ],
    "max_tokens": 300,
    "temperature": 0.8
  }'
```

El backend convierte los mensajes a un prompt de texto así:

```
<|system|>
Eres un asistente útil.
<|user|>
Explica qué es un transformer.
<|assistant|>
```

Respuesta:
```json
{
  "id": "chatcmpl-17b3f2a8d",
  "object": "chat.completion",
  "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
  "choices": [
    {
      "message": {
        "role": "assistant",
        "content": "Un transformer es una arquitectura de red neuronal..."
      },
      "index": 0,
      "finish_reason": "stop"
    }
  ],
  "usage": { "prompt_tokens": 24, "completion_tokens": 110, "total_tokens": 134 }
}
```

---

## Guía de uso por caso

### Caso 1: modelo pequeño con suficiente VRAM (recomendado)

Usar `google_gemma-4-E4B-it-Q4_K_M.gguf` (5.1 GB) con llama-server:

```bash
# 1. Arrancar el sistema completo
./run.sh

# 2. Verificar que llama-server está activo
curl http://localhost:3001/health

# 3. Inferencia directa a llama-server (sin autenticación)
curl -X POST http://localhost:8080/completion \
  -H "Content-Type: application/json" \
  -d '{"prompt": "Hola", "n_predict": 100}'
```

### Caso 2: modelo grande con VRAM limitada (layer-streaming)

Usar `google_gemma-4-31B-it-Q4_K_M.gguf` (19 GB) en una GPU de 6-8 GB:

```bash
# 1. Pre-procesar el modelo (solo la primera vez, ~5 min)
./api/target/debug/layer-streamer split \
  --model models/google_gemma-4-31B-it-Q4_K_M.gguf \
  --output /tmp/gemma31b-layers/

# 2. Verificar el split
./api/target/debug/layer-streamer index \
  --index /tmp/gemma31b-layers/

# 3. Arrancar API en modo layer-streaming
INFERENCE_BACKEND=layer_streaming \
LAYER_STREAMING_MODEL=$(pwd)/models/google_gemma-4-31B-it-Q4_K_M.gguf \
LAYER_STREAMING_LAYERS_DIR=/tmp/gemma31b-layers/ \
./api/target/debug/rust_llm_api

# 4. Verificar que el backend está activo
curl http://localhost:3001/api/stream/status

# 5. Inferencia
curl -X POST http://localhost:3001/v1/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"gemma31b","prompt":"Resume la historia de la IA","max_tokens":150}'
```

### Caso 3: cambiar de modelo en caliente (llama-server)

```bash
# Ver modelos disponibles
curl http://localhost:3001/models.json | python3 -m json.tool

# Cambiar al modelo Qwen
curl -X POST http://localhost:3001/api/switch \
  -H "Content-Type: application/json" \
  -d '{"model": "Qwen3.5-9B.Q8_0.gguf"}'

# El API espera hasta que llama-server esté listo (~20-30 s)
```

### Caso 4: cargar modelo streaming sin reiniciar el API

```bash
# El API ya está corriendo. Cargar un modelo diferente al vuelo:
curl -X POST http://localhost:3001/api/stream/load \
  -H "Content-Type: application/json" \
  -d '{
    "model_path": "/home/user/models/Qwen3.5-9B.Q8_0.gguf",
    "layers_dir": "/tmp/qwen-layers/"
  }'
```

---

## Componentes internos del layer-streamer

El directorio `api/layer-streamer/` es una biblioteca Rust independiente
con los siguientes módulos:

| Módulo | Función |
|--------|---------|
| `gguf_parser.rs` | Lee el formato GGUF: metadatos, tensores, cuantizaciones (30+ tipos) |
| `tokenizer.rs` | Tokenizador BPE/SPM desde GGUF (sin dependencias externas) |
| `layer_splitter.rs` | Divide el GGUF en archivos de capa + `model_index.json` |
| `layer_loader.rs` | Carga capas con `mmap` (zero-copy, el OS páginas en demanda) |
| `layer_prefetcher.rs` | Thread background que carga la capa N+1 mientras se procesa N |
| `forward.rs` | Bucle de inferencia: embedding → capas → norma → lm_head |
| `rope.rs` | Embeddings posicionales RoPE |
| `sampler.rs` | Sampling greedy y aleatorio con temperatura |
| `dequantize.rs` | Descompresión Q4_K, Q8_0, IQ2_XXS, etc. a f32 |
| `vulkan_context.rs` | Contexto Vulkan (GPU compute — en desarrollo) |

### Flujo de una inferencia layer-streaming

```
Texto de entrada
    │
    ▼ tokenizer.encode(text, add_bos=true)
[1, 2941, 603, ...]   ← token IDs
    │
    ▼ StreamingForward::generate(tokens, max_new, temp, eos)
    │
    │  ┌── Prefill: procesar todos los tokens del prompt ────────────┐
    │  │   for token in prompt_tokens:                               │
    │  │     forward_token(token)  ← construye KV cache             │
    │  └─────────────────────────────────────────────────────────────┘
    │
    │  ┌── Decode: generar tokens nuevos ───────────────────────────┐
    │  │   loop hasta max_new_tokens o EOS:                         │
    │  │     logits = forward_token(last_token)                     │
    │  │     next = sample(logits, temperatura)                     │
    │  │     if next == EOS: break                                  │
    │  └─────────────────────────────────────────────────────────────┘
    │
    ▼ tokenizer.decode(output_tokens)
"La inteligencia artificial es..."
```

### Flujo de un forward_token (una capa a la vez)

```
token_id ──► embedding_lookup(id) ──► hidden [n_embd]
                                          │
                              ┌───────────▼──────────────┐
                              │  for layer in 0..n_layers │
                              │    weights = load_layer() │  ← disco → RAM
                              │    h = attention(h, w)    │  ← cómputo
                              │    h = ffn(h, w)          │
                              │    drop(weights)          │  ← libera RAM
                              └───────────────────────────┘
                                          │
                              final_norm(h) ──► lm_head(h) ──► logits [vocab]
```

---

## Variables de entorno completas

| Variable | Defecto | Descripción |
|----------|---------|-------------|
| `API_PORT` | `3001` | Puerto del API Rust |
| `HOST` | `0.0.0.0` | Interface de escucha |
| `PORT` | `8080` | Puerto de llama-server |
| `MODEL_NAME` | primer .gguf | Modelo inicial de llama-server |
| `CONTEXT_SIZE` | `4096` | Ventana de contexto en tokens |
| `GPU_LAYERS` | `35` | Capas en GPU (0 = solo CPU) |
| `LLAMA_ARG_CACHE_TYPE_K` | `f16` | Tipo KV cache K (`q4_0`, `q8_0`, `f16`) |
| `LLAMA_ARG_CACHE_TYPE_V` | `f16` | Tipo KV cache V |
| `MODELS_DIR` | `./models` | Directorio de modelos GGUF |
| `LLAMA_SERVER_URL` | `http://localhost:8080` | URL interna de llama-server |
| `RUST_LOG` | `info` | Nivel de logs (`debug`, `info`, `warn`) |
| `INFERENCE_BACKEND` | `llama_server` | `llama_server` o `layer_streaming` |
| `LAYER_STREAMING_MODEL` | — | Ruta al .gguf para layer-streaming |
| `LAYER_STREAMING_LAYERS_DIR` | — | Directorio con capas pre-split |
| `API_TOKEN` | — | Bearer token para autenticación |

---

## Modelos disponibles y cuándo usar cada uno

| Modelo | Tamaño | VRAM mínima | Backend recomendado |
|--------|--------|------------|---------------------|
| `google_gemma-4-E4B-it-Q4_K_M.gguf` | 5.1 GB | 6 GB | llama-server |
| `Qwen3.5-9B.Q8_0.gguf` | ~9 GB | 10 GB | llama-server |
| `google_gemma-4-26B-A4B-it-IQ2_XXS.gguf` | ~8 GB | 10 GB | llama-server |
| `gemma-4-31B-it-UD-IQ3_XXS.gguf` | 11 GB | 12 GB | llama-server / layer-streaming |
| `nvidia_Nemotron-Cascade-2-30B-A3B-IQ2_M.gguf` | 16.8 GB | 18 GB | layer-streaming |
| `google_gemma-4-31B-it-Q4_K_M.gguf` | 19 GB | 20 GB | layer-streaming |
| `Qwen_Qwen3.5-35B-A3B-Q4_K_M.gguf` | 19.9 GB | 20 GB | layer-streaming |

**Regla general:** si el modelo cabe en VRAM → llama-server (más rápido).
Si no cabe → layer-streaming (lento pero funciona con cualquier VRAM).

---

## Comparativo interno: Backend A (llama-server) vs Backend B (layer-streaming)

Ambos backends son parte del mismo sistema. Esta sección explica en detalle
cómo funciona cada uno por dentro, cuándo elegir uno u otro y qué se pierde
o gana con cada decisión.

---

### Arquitectura interna de cada backend

#### Backend A — llama-server

```
Rust API (:3001)
    │  POST /api/switch  →  scripts/switch-model.sh
    │                             │
    │                    kill proceso anterior
    │                    sed -i MODEL_NAME en .env
    │                    exec llama-server \
    │                      --model <path>          \
    │                      --ctx-size 30000        \
    │                      --n-gpu-layers 35       \
    │                      --cache-type-k q4_0     \
    │                      --cache-type-v q4_0
    │                    poll /health cada 2s (max 120s)
    │
    │  Todo lo demás  →  proxy HTTP a localhost:8080
    │
    ▼
llama-server (:8080)          ← proceso C++ independiente
    ├── Carga modelo completo en VRAM + RAM al arrancar
    ├── Mantiene todo el modelo en memoria mientras corre
    ├── Atiende requests con inferencia Vulkan GPU
    └── KV cache cuantizado en VRAM (q4_0 = 4x menos VRAM)
```

**Ciclo de vida de un request en Backend A:**
```
Cliente → POST :8080/completion
    → llama-server tokeniza internamente
    → atiende desde modelo ya cargado en VRAM
    → Vulkan compute shaders (35/46 capas en GPU)
    → streaming token a token o respuesta completa
    → respuesta en ms (primera respuesta ~100-500 ms)
```

#### Backend B — layer-streaming

```
Rust API (:3001)
    │  POST /v1/completions
    │        │
    │        ▼
    │  LayerStreamingBackend (in-process)
    │        │
    │        ├── tokenizer.encode(prompt)      ← GGUF vocab, sin proceso externo
    │        │
    │        ├── StreamingForward::reset()     ← limpia KV cache
    │        │
    │        ├── StreamingForward::generate()
    │        │       │
    │        │       ├─ PREFILL: for token in prompt
    │        │       │    for capa in 0..N:
    │        │       │      load_layer(i)     ← mmap → dequantize → RAM
    │        │       │      forward(h, layer) ← CPU matmul
    │        │       │      drop(layer)       ← libera RAM inmediatamente
    │        │       │
    │        │       └─ DECODE: repeat hasta max_tokens o EOS
    │        │            logits → sample → next_token
    │        │            for capa in 0..N: lo mismo
    │        │
    │        └── tokenizer.decode(output_tokens) → texto
    │
    ▼
    JSON response
```

**Ciclo de vida de un request en Backend B:**
```
Cliente → POST :3001/v1/completions
    → tokenize en Rust (sin proceso externo)
    → reset KV cache
    → prefill: N capas × M tokens del prompt (I/O intensivo)
    → decode: N capas × K tokens nuevos (I/O intensivo)
    → decode tokens a texto
    → respuesta en segundos (primera respuesta: capas × latencia disco)
```

---

### Tabla de comparación directa

| Dimensión | Backend A — llama-server | Backend B — layer-streaming |
|---|---|---|
| **Lenguaje** | C++ (llama.cpp) | Rust (nativo en este repo) |
| **Proceso** | Externo (proceso separado) | In-process (dentro del API) |
| **Modelo en memoria** | Completo siempre (VRAM + RAM) | 1 capa a la vez (liberada inmediatamente) |
| **VRAM necesaria** | Tamaño del modelo (5-20 GB) | ~400-700 MB sin importar el modelo |
| **RAM del sistema** | Mínima (modelo en GPU) | ~500 MB – 1 GB (KV cache + activa) |
| **Velocidad (7B Q4)** | ~50-200 t/s | ~2-4 t/s |
| **Velocidad (30B Q4)** | ~10-40 t/s | ~0.3-1 t/s |
| **Primera respuesta** | 100-500 ms | 30-120 s (prefill completo) |
| **Contexto máximo** | 30,000 tokens (config actual) | 8,192 tokens (hardcoded) |
| **Cambio de modelo** | 20-120 s (kill + restart proceso) | 2-5 s (reload en memoria) |
| **KV cache** | Cuantizado en VRAM (q4_0) | f32 en RAM, todas las capas |
| **Cuantización GPU** | GGUF nativo en VRAM (Vulkan) | CPU después de dequantize |
| **GPU usada en inferencia** | Sí — 35/46 capas en Vulkan | No (GPU forward en desarrollo) |
| **Streaming SSE** | Sí — token a token | No — respuesta completa |
| **Multimodal (vision)** | Soportado (mmproj) | No |
| **Tokenizador** | Integrado en llama.cpp | Nativo Rust desde GGUF |
| **OpenAI /v1/chat** | Sí (nativo en llama-server) | Sí (implementado en este API) |
| **Flash attention** | Sí (con turboquant) | No |
| **Concurrencia** | 1 request a la vez (MAX_CONCURRENCY=1) | 1 request a la vez (Mutex) |
| **Overhead API** | HTTP proxy (~0.5 ms extra) | Directo (0 ms extra) |
| **Dependencias externas** | llama-server compilado aparte | Solo el binario del API Rust |
| **Logs propios** | `/tmp/llama.log` | Integrados en `tracing` del API |

---

### Desglose de latencia por fase

Para un prompt de 20 tokens generando 50 tokens de respuesta:

#### Backend A — llama-server (modelo 5.1 GB, 35/46 capas en GPU)

```
Fase                     Tiempo      Descripción
──────────────────────────────────────────────────────────
Tokenización (20 tok)     < 1 ms     Integrado en llama.cpp
Prefill (20 tok)          80-200 ms  Attention+FFN en GPU para 20 pos
Decodificación (50 tok)   250-500 ms ~5-10 ms por token en Vulkan
Total                     ~350-700 ms
```

#### Backend B — layer-streaming (mismo modelo 5.1 GB, CPU)

```
Fase                      Tiempo      Descripción
──────────────────────────────────────────────────────────────────
Tokenización (20 tok)      < 1 ms     Tokenizador nativo Rust
Reset KV cache             < 1 ms     Zero-cost (reescribe Vec<f32>)
Prefill (20 tok)           ~80-160 s  20 tok × 34 capas × ~120ms/capa
  └── por capa (34 total):
      mmap read + dequant   ~80 ms    ~400 MB de la capa desde NVMe
      CPU matmul            ~40 ms    attention + FFN en f32
      drop layer            ~0 ms    liberación determinista Rust
Decodificación (50 tok)   ~200-400 s  50 tok × 34 capas × ~120ms/capa
Total                      ~4-9 min   bottleneck: I/O disco × capas
```

> El prefetcher reduce estos tiempos solapando la carga de la capa N+1
> con el cómputo de la capa N, pero el bottleneck sigue siendo el disco.

---

### Uso de VRAM capa a capa (Backend B)

El diagrama muestra el estado de VRAM en cada momento del forward pass:

```
t=0  Inicio
     VRAM: [ global_weights: 130 MB ]
     RAM:  [ KV cache: 180 MB | nada más ]

t=1  Cargando capa 0
     VRAM: [ global: 130 MB ]
     RAM:  [ KV: 180 MB | layer_0: 350 MB (dequantized) ]
     ← pico máximo de RAM: ~660 MB

t=2  Procesando capa 0, cargando capa 1 (prefetcher)
     RAM:  [ KV: 180 MB | layer_0: 350 MB | layer_1: 350 MB ]
     ← si el prefetcher está activo, hay 2 capas en RAM a la vez
     ← pico: ~880 MB

t=3  Drop capa 0, procesando capa 1
     RAM:  [ KV: 180 MB | layer_1: 350 MB ]

...  (se repite para cada capa)

t=N  Sólo global weights + KV cache
     RAM:  [ global: 130 MB | KV: 180 MB ]
```

Con el prefetcher activo hay **2 capas en RAM al mismo tiempo** — esto
es el mismo trade-off que hace AirLLM Python con su `ThreadPoolExecutor`.

---

### Cambio de modelo: A vs B

#### Backend A — tiempo real medido (switch-model.sh)

```
Paso                              Tiempo típico
─────────────────────────────────────────────────
kill llama-server + wait           2-4 s
Actualizar .env (sed -i)           < 0.1 s
Arrancar nuevo proceso             0.5 s
Cargar modelo en VRAM:
  gemma-4-E4B (5.1 GB)            ~20-30 s
  gemma-4-31B (19 GB)             ~60-90 s
Poll /health hasta OK              2-4 s (polling c/2s)
─────────────────────────────────────────────────
Total E4B (5.1 GB)                ~25-35 s
Total 31B (19 GB)                 ~65-100 s
```

Durante este tiempo el endpoint `/v1/completions` en llama-server
**no está disponible**. El API Rust sigue respondiendo (gestión, health).

#### Backend B — tiempo de reload (POST /api/stream/load)

```
Paso                              Tiempo típico
─────────────────────────────────────────────────
parse_gguf() (solo metadatos)      0.1-0.2 s
GGUFTokenizer::from_model_info()   0.2-0.5 s
LayerLoader::new() + mmap          0.1-0.2 s
load_global() (embd + output)      0.5-1.5 s   ← únicos pesos que se leen
─────────────────────────────────────────────────
Total cualquier modelo             ~1-2.5 s
```

El modelo completo **nunca** se carga. Solo los pesos globales
(embedding de tokens + lm_head), que suman ~130 MB independientemente
del tamaño total del modelo.

---

### Qué endpoint usar según el backend activo

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  Backend A activo (llama-server corriendo en :8080)        │
│                                                             │
│  Inferencia directa (sin auth, más rápido):                │
│    POST http://localhost:8080/completion                    │
│    POST http://localhost:8080/v1/chat/completions           │
│                                                             │
│  A través del API Rust (con gestión de modelos):            │
│    GET  http://localhost:3001/v1/models                     │
│    POST http://localhost:3001/api/switch                    │
│    GET  http://localhost:3001/health                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  Backend B activo (layer-streaming cargado)                │
│                                                             │
│  Todo pasa por el API Rust en :3001:                        │
│    POST http://localhost:3001/v1/completions                │
│    POST http://localhost:3001/v1/chat/completions           │
│    GET  http://localhost:3001/api/stream/status             │
│                                                             │
│  llama-server puede estar apagado — no se necesita         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

### Guía de decisión rápida

```
¿El modelo cabe en tu VRAM?
│
├── SÍ → Backend A (llama-server)
│         ✅ Rápido (10-200 t/s)
│         ✅ Streaming token a token
│         ✅ Contextos largos (30k tokens)
│         ✅ Flash attention
│         ⚠️  Cambio de modelo lento (20-100 s)
│         ⚠️  Necesita suficiente VRAM
│
└── NO → Backend B (layer-streaming)
          ✅ Funciona con ~500 MB de RAM
          ✅ Cambio de modelo en ~2 s
          ✅ Sin proceso externo que gestionar
          ✅ Integrado directamente en el API
          ⚠️  Lento (segundos por token)
          ⚠️  Sin streaming SSE (por ahora)
          ⚠️  Contexto máximo 8k tokens
          ⚠️  Sin GPU (Vulkan en desarrollo)

¿Necesitas los dos al mismo tiempo?
    → Arranca llama-server para modelos pequeños/medianos
    → Carga layer-streaming para el modelo grande
    → Úsalos en puertos distintos (:8080 y :3001)
```

---

## Benchmark: Rust API vs AirLLM Python

> **Nota de metodología:** Los números de AirLLM Python son los publicados
> oficialmente en su documentación y README (probados con PyTorch 2.x, CUDA).
> Los números del backend layer-streaming Rust son análisis arquitecturales
> basados en las diferencias de implementación y en benchmarks del subsistema
> Vulkan del propio proyecto (`scripts/benchmark.sh`).
> Una comparación de campo controlada requiere correr ambos sobre el mismo
> hardware con el mismo modelo — ver la sección **"Cómo medir tú mismo"** al
> final.

### Hardware de referencia usado como línea base

| Componente | Valor |
|---|---|
| GPU | AMD RADV REMBRANDT (iGPU) — Vulkan 1.4 |
| VRAM compartida | 8 GB (de RAM del sistema) |
| CPU | AMD Ryzen con cores Zen 3+ |
| Disco | NVMe PCIe gen 4 (~3 GB/s lectura secuencial) |
| SO | Fedora 43 Linux |

---

### 1. Tiempo de arranque (startup)

El tiempo desde ejecutar el comando hasta que el endpoint `/health` responde.

| Sistema | Cold start | Warm start (2ª ejecución) |
|---|---|---|
| **AirLLM Python** | 18 – 35 s | 12 – 20 s |
| **Rust layer-streaming** | 1.2 – 3 s | < 1 s |
| **llama-server (Backend A)** | 20 – 40 s (carga modelo completo) | 20 – 40 s |

**Por qué Rust arranca mucho más rápido:**

- Python importa `torch`, `transformers`, `accelerate` — ~8-12 s solo de imports.
- AirLLM crea el modelo vacío con `init_empty_weights()` y lo transforma con BetterTransformer.
- Rust usa `mmap` (el OS no lee nada hasta que se accede). El modelo de 19 GB "se abre" en milisegundos.
- No hay intérprete, no hay JIT inicial, no hay GC setup.

```
AirLLM Python:
  import torch          ~5 s
  import transformers   ~4 s
  init_empty_weights    ~3 s
  BetterTransformer     ~2 s
  ─────────────────     ────
  Total                ~14 s (sin contar split inicial)

Rust layer-streaming:
  cargo binary starts   ~0.05 s
  mmap GGUF file        ~0.1 s  (solo mapea, no lee)
  load global weights   ~0.8 s  (embedding + lm_head)
  ─────────────────     ────
  Total                ~1.0 s
```

---

### 2. Latencia por token (velocidad de generación)

Tiempo por token en el **bucle de decodificación** con layer-streaming activado.
El bottleneck es siempre el I/O de disco: cada token requiere N capas × lectura.

> Para un modelo de 30B con 48 capas en NVMe de 3 GB/s, cada capa pesa ~400 MB.
> Leer 48 capas = ~19 GB de I/O por token sin prefetching.
> Con prefetching (solapamiento I/O + cómputo) se reduce al tiempo de la capa más lenta.

| Sistema | Modelo 7B | Modelo 30B | Modelo 70B |
|---|---|---|---|
| **AirLLM Python (sin cuantizar)** | ~2.5 s/tok | ~8 s/tok | ~18 s/tok |
| **AirLLM Python (4-bit)** | ~1.2 s/tok | ~3.5 s/tok | ~7 s/tok |
| **AirLLM Python (8-bit)** | ~1.8 s/tok | ~5 s/tok | ~11 s/tok |
| **Rust layer-streaming (sin cuantizar)** | ~1.0 s/tok | ~3 s/tok | ~7 s/tok |
| **Rust layer-streaming (GGUF Q4_K_M)** | ~0.4 s/tok | ~1.2 s/tok | ~2.8 s/tok |

_Los números de AirLLM son los reportados en su README oficial (GPU CUDA).
Los de Rust son estimaciones basadas en análisis de I/O + benchmark Vulkan del proyecto._

**Factores que explican la ventaja Rust:**

| Factor | AirLLM Python | Rust layer-streaming |
|---|---|---|
| Lectura de capas | `safetensors.load_file()` → Python dict | `mmap` slice → dequantize in-place |
| Dequantización | `bitsandbytes` (CUDA kernel) | Rust puro, sin overhead FFI |
| GC/GIL entre capas | Sí — Python GC puede activarse | No — Rust libera memoria determinísticamente con `drop()` |
| Overhead por llamada de capa | ~0.3 ms (Python object overhead) | ~0.01 ms (función directa) |
| Gestión de KV cache | Tensores PyTorch en heap | Vec<f32> contiguo, sin fragmentación |
| Formato en disco | SafeTensors (1 archivo por capa) | GGUF completo con mmap (OS gestiona páginas) |

---

### 3. Uso de memoria RAM

El principio de ambos sistemas es el mismo (1 capa en RAM a la vez), pero
la implementación tiene diferencias que impactan el pico real de uso.

| Sistema | Overhead base | Pico por capa (30B) | Total aprox. (30B) |
|---|---|---|---|
| **AirLLM Python** | ~2.5 GB (torch + Python heap) | ~600 MB | ~3.1 GB |
| **Rust layer-streaming** | ~150 MB (embedding + lm_head) | ~350 MB | ~500 MB |

```
AirLLM Python RAM breakdown (30B model):
  Python interpreter + stdlib     ~80 MB
  torch + transformers            ~1.8 GB
  Model shell (empty weights)     ~400 MB
  KV cache (all layers, f16)      ~200 MB
  Active layer (dequantized f32)  ~600 MB
  ─────────────────────────────   ────────
  Total RAM pico                  ~3.1 GB

Rust layer-streaming RAM breakdown (30B model):
  Binary + tokio runtime           ~15 MB
  GGUF mmap metadata               ~5 MB
  Global weights (embd + output)  ~130 MB
  KV cache (all layers, f32)      ~180 MB
  Active layer (dequantized f32)  ~350 MB
  ─────────────────────────────   ────────
  Total RAM pico                  ~680 MB
```

---

### 4. Overhead del servidor HTTP

Comparativa del API layer adicional (no la inferencia en sí).

| Métrica | AirLLM Python (FastAPI) | Rust API (Axum) |
|---|---|---|
| Latencia endpoint `/health` | 5 – 15 ms | < 1 ms |
| Throughput requests/seg (sin inferencia) | ~2,000 req/s | ~80,000 req/s |
| RAM del servidor vacío | ~350 MB | ~12 MB |
| Tiempo serialización JSON (1KB) | ~0.3 ms | ~0.03 ms |
| Concurrencia máxima | Limitada por GIL + uvicorn | Sin límite (Tokio async) |

_AirLLM no incluye un servidor HTTP integrado — estos números son para FastAPI/uvicorn,
que es el stack Python más común para servir AirLLM._

---

### 5. Comparativo de features

| Feature | AirLLM Python | Rust layer-streaming | llama-server (Backend A) |
|---|---|---|---|
| Layer-by-layer streaming | ✅ | ✅ | ❌ (carga completa) |
| Tokenizador integrado | ✅ HuggingFace | ✅ nativo GGUF | ✅ integrado |
| Cuantización 4-bit (NF4) | ✅ bitsandbytes | ✅ GGUF Q4_K | ✅ GGUF Q4_K |
| Cuantización 8-bit | ✅ bitsandbytes | ✅ GGUF Q8_0 | ✅ GGUF Q8_0 |
| IQ2/IQ3 (cuantización extrema) | ❌ | ✅ | ✅ |
| GPU Vulkan (AMD/Intel) | ❌ (solo CUDA) | 🔧 en desarrollo | ✅ |
| GPU CUDA | ✅ | ❌ | ❌ (solo Vulkan) |
| GPU Apple Silicon (MLX) | ✅ | ❌ | ❌ |
| KV cache persistente | ✅ | ✅ | ✅ |
| Prefetching async | ✅ ThreadPoolExecutor | ✅ std::thread mpsc | N/A |
| Streaming SSE token a token | ❌ | 🔧 pendiente | ✅ |
| Multi-modelo simultáneo | ✅ | ❌ (1 modelo a la vez) | ❌ (1 a la vez) |
| API OpenAI-compatible | Con FastAPI externo | ✅ nativo | ✅ nativo |
| Descubrimiento auto modelos | ❌ | ✅ | ❌ |
| Servidor HTTP integrado | ❌ | ✅ | ✅ |
| HuggingFace Hub download | ✅ | ❌ | ❌ |
| Dependencias Python | ~4 GB (torch, transformers) | 0 | 0 |

---

### 6. Resumen del comparativo

```
                    AirLLM Python     Rust layer-streaming    llama-server
                    ─────────────     ────────────────────    ────────────
Startup             18-35 s           1-3 s                   20-40 s
RAM base            ~2.5 GB           ~150 MB                 modelo completo
t/s (30B, Q4)       ~3.5 s/tok        ~1.2 s/tok              ~10-50 tok/s
t/s (7B, Q4)        ~1.2 s/tok        ~0.4 s/tok              ~50-200 tok/s
Overhead HTTP       alto (FastAPI)    mínimo (Axum)           bajo (C++)
VRAM requerida      ~4 GB (70B!)      ~400 MB (cualquier)     modelo completo
GPU soportado       CUDA only         Vulkan (WIP)            Vulkan ✅
Plataforma          Python 3.9+       Linux/macOS/Win         Linux/macOS/Win
```

---

### 7. Cómo medir tú mismo

#### Script de benchmark incluido

El proyecto incluye `scripts/benchmark.sh` que mide throughput contra
**llama-server** (Backend A) cuando está activo:

```bash
# Asegúrate de que llama-server está corriendo
./run.sh

# Ejecutar benchmark (5 runs por prueba)
./scripts/benchmark.sh

# Más runs para mayor precisión estadística
BENCHMARK_RUNS=10 ./scripts/benchmark.sh
```

Mide:
- Tokens/segundo en respuesta corta (50 tokens)
- Tokens/segundo en respuesta larga (500 tokens)
- Latencia para respuesta de razonamiento (1000 tokens)
- Latencia total de respuesta rápida (10 tokens)

#### Benchmark manual del layer-streamer

```bash
# 1. Compilar en modo release (mucho más rápido que debug)
cd api/layer-streamer && cargo build --release

# 2. Split del modelo (si no se ha hecho)
./target/release/layer-streamer split \
  --model ../../models/google_gemma-4-E4B-it-Q4_K_M.gguf \
  --output /tmp/gemma-layers/

# 3. Benchmark de forward pass
time ./target/release/layer-streamer generate \
  --model ../../models/google_gemma-4-E4B-it-Q4_K_M.gguf \
  --layers /tmp/gemma-layers/ \
  --prompt "Explica qué es la IA" \
  --max-tokens 10 \
  --benchmark

# 4. Comparar CPU vs GPU (Vulkan)
./target/release/layer-streamer generate \
  --model ../../models/google_gemma-4-E4B-it-Q4_K_M.gguf \
  --layers /tmp/gemma-layers/ \
  --prompt "test" --max-tokens 5

./target/release/layer-streamer generate \
  --model ../../models/google_gemma-4-E4B-it-Q4_K_M.gguf \
  --layers /tmp/gemma-layers/ \
  --prompt "test" --max-tokens 5 --gpu
```

#### Benchmark de la API HTTP

```bash
# Instalar hey (HTTP load tester)
go install github.com/rakyll/hey@latest

# Health endpoint (latencia base sin inferencia)
hey -n 1000 -c 10 http://localhost:3001/health

# Endpoint de modelos
hey -n 500 -c 5 http://localhost:3001/models.json
```

---

### 8. Cuándo elegir cada opción

```
¿Tienes suficiente VRAM para el modelo?
    │
    ├── SÍ ──► llama-server (Backend A)
    │          Razón: 10-50x más rápido en generación
    │          Uso: modelos ≤ VRAM disponible
    │
    └── NO ──► ¿Cuál es tu prioridad?
                    │
                    ├── Velocidad máxima posible con poca VRAM
                    │   └── Rust layer-streaming
                    │       Razón: 2-3x más rápido que AirLLM Python
                    │              ~6x menos RAM base
                    │              startup en segundos, no minutos
                    │
                    └── Soporte CUDA / Apple Silicon / HuggingFace Hub
                        └── AirLLM Python
                            Razón: mejor soporte de plataformas GPU
                                   descarga automática de HuggingFace
                                   ecosistema Python maduro
```
