# 📡 Documentación de la API

## Endpoints

### `GET /health`

Verifica que el sistema está funcionando.

```bash
curl http://localhost:9000/health
```

**Respuesta:**
```json
{
  "status": "ok",
  "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
  "context_size": 8192
}
```

---

### `GET /v1/models`

Lista el modelo cargado y su configuración.

```bash
curl http://localhost:9000/v1/models \
  -H "Authorization: Bearer $API_TOKEN"
```

**Respuesta:**
```json
{
  "object": "list",
  "data": [{
    "id": "google_gemma-4-E4B-it-Q4_K_M.gguf",
    "object": "model",
    "created": 1712300000,
    "owned_by": "local-gguf",
    "context_size": 8192
  }]
}
```

---

### `POST /v1/chat/completions`

Generación de texto con formato OpenAI-compatible.

#### Parámetros

| Campo | Tipo | Default | Descripción |
|-------|------|---------|-------------|
| `messages` | array | **Requerido** | Historial de conversación |
| `messages[].role` | string | **Requerido** | `system`, `user`, `assistant` |
| `messages[].content` | string | **Requerido** | Contenido del mensaje |
| `temperature` | float | `0.7` | Creatividad (0.0 - 2.0) |
| `max_tokens` | int | `1024` | Máximo tokens a generar |
| `stream` | bool | `true` | Habilitar streaming SSE |
| `stop` | string[] | `[]` | Secuencias de parada |

#### Ejemplo: Generación Normal

```bash
curl http://localhost:9000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "messages": [
      {"role": "system", "content": "Eres un asistente útil."},
      {"role": "user", "content": "Explícame qué es Rust en 3 frases."}
    ],
    "max_tokens": 256,
    "temperature": 0.7,
    "stream": false
  }'
```

#### Ejemplo: Streaming (SSE)

```bash
curl -N http://localhost:9000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "messages": [{"role": "user", "content": "Escribe un poema corto"}],
    "max_tokens": 128,
    "stream": true
  }'
```

#### Python con OpenAI SDK

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:9000/v1",
    api_key="mi-token-seguro"
)

response = client.chat.completions.create(
    model="gemma-4",
    messages=[
        {"role": "user", "content": "¿Qué es la inteligencia artificial?"}
    ],
    max_tokens=512,
    temperature=0.7,
    stream=True
)

for chunk in response:
    if chunk.choices[0].delta.content:
        print(chunk.choices[0].delta.content, end="", flush=True)
```

#### Node.js con fetch

```javascript
const response = await fetch("http://localhost:9000/v1/chat/completions", {
  method: "POST",
  headers: {
    "Content-Type": "application/json",
    "Authorization": "Bearer mi-token-seguro"
  },
  body: JSON.stringify({
    messages: [{ role: "user", content: "¿Qué es JavaScript?" }],
    max_tokens: 256,
    temperature: 0.7,
  }),
});

const data = await response.json();
console.log(data.choices[0].message.content);
```

---

### `POST /v1/images/generations`

> ⚠️ Placeholder — llama.cpp no soporta generación de imágenes nativamente.
> Requiere modelo dedicado (FLUX, Stable Diffusion) con backend separado.

### `POST /v1/audio/speech`

> ⚠️ Placeholder — Text-to-Speech requiere modelo dedicado (VITS, Bark).

### `POST /v1/audio/transcriptions`

Transcripción de audio a texto. Requiere modelo Whisper GGUF.

```bash
curl http://localhost:9000/v1/audio/transcriptions \
  -H "Authorization: Bearer $API_TOKEN" \
  -F "file=@audio.mp3" \
  -F "model=whisper" \
  -F "language=es"
```

---

## 🔐 Autenticación

Todas las rutas `/v1/*` requieren el header:
```
Authorization: Bearer <tu-token>
```

Sin token: `401 Unauthorized`

## ⚡ Rate Limiting

Configurable en `.env`:
```bash
RATE_LIMIT_REQUESTS=100   # 100 requests
RATE_LIMIT_SECONDS=60     # por ventana de 60 segundos
```

Respuesta al exceder: `429 Too Many Requests`
