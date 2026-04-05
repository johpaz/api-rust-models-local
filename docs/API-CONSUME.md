# 📡 Guía para Consumir la API

Guía práctica para interactuar con el LLM API Server (llama-server :8080).

## 🔗 Información General

| Propiedad | Valor |
|-----------|-------|
| **URL base** | `http://localhost:8080` |
| **Formato** | OpenAI-compatible |
| **Auth** | No requerida |
| **Modelo** | El que se cargó al iniciar el servidor |

### Formato mínimo de request

El campo `model` es **opcional** — llama-server solo tiene un modelo cargado. Solo necesitas:

```json
{
  "messages": [{"role": "user", "content": "Tu pregunta aquí"}],
  "max_tokens": 1024
}
```

---

## 🚀 Chat Completions

### curl (mínimo)

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [
      {"role": "user", "content": "¿Qué es Rust?"}
    ],
    "max_tokens": 1024
  }'
```

### curl (completo con opciones)

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [
      {"role": "user", "content": "¿Qué es Rust?"}
    ],
    "max_tokens": 1024,
    "temperature": 0.7,
    "top_p": 0.95
  }'
```

### Python (requests)

```python
import requests

response = requests.post(
    "http://localhost:8080/v1/chat/completions",
    json={
        "messages": [{"role": "user", "content": "¿Qué es Rust?"}],
        "max_tokens": 1024,
    },
)

data = response.json()
print(data["choices"][0]["message"]["content"])
```

### Python (OpenAI SDK)

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="not-needed",
)

response = client.chat.completions.create(
    messages=[{"role": "user", "content": "¿Qué es Rust?"}],
    max_tokens=1024,
)

print(response.choices[0].message.content)
```

### JavaScript/Node.js (fetch)

```javascript
const response = await fetch("http://localhost:8080/v1/chat/completions", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({
    messages: [{ role: "user", content: "¿Qué es Rust?" }],
    max_tokens: 1024,
  }),
});

const data = await response.json();
console.log(data.choices[0].message.content);
```

### JavaScript (OpenAI SDK)

```javascript
import OpenAI from "openai";

const openai = new OpenAI({
  baseURL: "http://localhost:8080/v1",
  apiKey: "not-needed",
});

const response = await openai.chat.completions.create({
  messages: [{ role: "user", content: "¿Qué es Rust?" }],
  max_tokens: 1024,
});

console.log(response.choices[0].message.content);
```

### Bun (fetch directo)

```typescript
// cliente.ts
const response = await fetch(
  "http://localhost:8080/v1/chat/completions",
  {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      model: "google_gemma-4-26B-A4B-it-IQ2_XXS",
      messages: [{ role: "user", content: "Explica async/await" }],
      max_tokens: 512,
    }),
  },
);

const data = await response.json();
console.log(data.choices[0].message.content);
```

```bash
bun run cliente.ts
```

### Insomnia / Postman

1. Crear nueva request `POST`
2. URL: `http://localhost:8080/v1/chat/completions`
3. Headers: `Content-Type: application/json`
4. Body (JSON):

```json
{
  "model": "google_gemma-4-26B-A4B-it-IQ2_XXS",
  "messages": [
    { "role": "user", "content": "¿Qué es Rust?" }
  ],
  "max_tokens": 512,
  "temperature": 0.7
}
```

---

## 📋 Streaming (SSE)

Respuesta token por token en tiempo real.

### curl

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [
      {"role": "user", "content": "Explica async/await en Rust"}
    ],
    "stream": true,
    "max_tokens": 1024
  }'
```

### Python (streaming)

```python
import requests
import json

response = requests.post(
    "http://localhost:8080/v1/chat/completions",
    json={
        "messages": [{"role": "user", "content": "Explica async/await"}],
        "stream": True,
        "max_tokens": 1024,
    },
    stream=True,
)

for line in response.iter_lines():
    if line:
        line = line.decode("utf-8")
        if line.startswith("data: ") and line != "data: [DONE]":
            chunk = json.loads(line[6:])
            token = chunk["choices"][0].get("delta", {}).get("content", "")
            print(token, end="", flush=True)
print()
```

### Python (OpenAI SDK streaming)

```python
from openai import OpenAI

client = OpenAI(base_url="http://localhost:8080/v1", api_key="not-needed")

stream = client.chat.completions.create(
    messages=[{"role": "user", "content": "Explica async/await"}],
    max_tokens=1024,
    stream=True,
)

for chunk in stream:
    token = chunk.choices[0].delta.content or ""
    print(token, end="", flush=True)
print()
```

### JavaScript/Node.js (streaming)

```javascript
const response = await fetch("http://localhost:8080/v1/chat/completions", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({
    messages: [{ role: "user", content: "Explica async/await" }],
    stream: true,
    max_tokens: 1024,
  }),
});

const reader = response.body.getReader();
const decoder = new TextDecoder();

while (true) {
  const { done, value } = await reader.read();
  if (done) break;

  const text = decoder.decode(value);
  for (const line of text.split("\n")) {
    if (line.startsWith("data: ") && line !== "data: [DONE]") {
      const chunk = JSON.parse(line.slice(6));
      const token = chunk.choices[0]?.delta?.content || "";
      process.stdout.write(token);
    }
  }
}
console.log();
```

### JavaScript (OpenAI SDK streaming)

```javascript
import OpenAI from "openai";

const openai = new OpenAI({
  baseURL: "http://localhost:8080/v1",
  apiKey: "not-needed",
});

const stream = await openai.chat.completions.create({
  messages: [{ role: "user", content: "Explica async/await" }],
  stream: true,
});

for await (const chunk of stream) {
  process.stdout.write(chunk.choices[0]?.delta?.content || "");
}
console.log();
```

---

## 🔍 Health Check

Verificar que el servidor está corriendo:

```bash
curl http://localhost:8080/health
```

**Respuesta:**
```json
{"status": "ok"}
```

---

## 📊 Obtener Métricas de Rendimiento

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "google_gemma-4-26B-A4B-it-IQ2_XXS",
    "messages": [{"role": "user", "content": "Hola"}],
    "max_tokens": 50
  }' | python3 -m json.tool | grep -A 20 "timings"
```

**Respuesta incluye `timings`:**
```json
"timings": {
  "prompt_n": 21,
  "prompt_ms": 396.512,
  "prompt_per_second": 52.96,
  "predicted_n": 1704,
  "predicted_ms": 61129.52,
  "predicted_per_second": 27.88
}
```

---

## ⚙️ Parámetros de la Request

### Request Body

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `model` | string | — | Nombre del modelo (informativo) |
| `messages` | array | — | Lista de mensajes con `role` y `content` |
| `max_tokens` | int | 256 | Máximo de tokens a generar |
| `temperature` | float | 0.8 | Creatividad (0 = determinista, 2 = más creativa) |
| `top_p` | float | 0.95 | Núcleo de probabilidad |
| `top_k` | int | 40 | Top-k sampling |
| `stream` | bool | false | Streaming SSE |
| `stop` | array | — | Secuencias de parada |
| `frequency_penalty` | float | 0.0 | Penalizar repetición |
| `presence_penalty` | float | 0.0 | Penalizar temas repetidos |
| `seed` | int | — | Seed para reproducibilidad |

### Roles de Mensajes

| Role | Descripción | Ejemplo |
|------|-------------|---------|
| `system` | Instrucciones del sistema | "Eres un experto en programación" |
| `user` | Mensaje del usuario | "¿Qué es un closure?" |
| `assistant` | Respuesta previa del modelo | "Un closure es..." |
| `tool` | Resultado de llamada a herramienta | '{"result": 42}' |

### Ejemplo Multi-turno

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [
      {"role": "system", "content": "Eres un asistente técnico experto."},
      {"role": "user", "content": "¿Qué diferencia hay entre Rust y C++?"},
      {"role": "assistant", "content": "Rust garantiza seguridad de memoria en compilación..."},
      {"role": "user", "content": "¿Puedes dar un ejemplo de código?"}
    ],
    "max_tokens": 1024,
    "temperature": 0.7
  }'
```

---

## 🔧 Opciones Avanzadas

### Thinking/Reasoning Content

Gemma 4 soporta reasoning interno. Para ver el contenido de razonamiento:

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [
      {"role": "user", "content": "Resuelve: 15 * 23 + 7"}
    ],
    "max_tokens": 1024
  }' | python3 -c "
import sys, json
data = json.load(sys.stdin)
msg = data['choices'][0]['message']
if 'reasoning_content' in msg:
    print('=== RAZONAMIENTO ===')
    print(msg['reasoning_content'])
    print()
print('=== RESPUESTA ===')
print(msg['content'])
"
```

### Control de Longitud

```json
{
  "max_tokens": 100,
  "min_tokens": 50
}
```

### Reproductibilidad

```json
{
  "seed": 42,
  "temperature": 0.5
}
```

### Parar en secuencia específica

```json
{
  "stop": ["\n\n", "###", "---"]
}
```

---

## 📝 Ejemplos Prácticos

### 1. Chat Interactivo con Python

```python
import requests

BASE_URL = "http://localhost:8080/v1/chat/completions"
messages = []

print("💬 Chat (escribe 'salir' para terminar)")
while True:
    user_input = input("\nTú: ")
    if user_input.lower() in ("salir", "exit", "quit"):
        break

    messages.append({"role": "user", "content": user_input})

    response = requests.post(BASE_URL, json={
        "messages": messages,
        "max_tokens": 1024,
        "temperature": 0.7,
    })

    data = response.json()
    assistant_msg = data["choices"][0]["message"]["content"]
    messages.append({"role": "assistant", "content": assistant_msg})

    print(f"\nAsistente: {assistant_msg}")
```

### 2. Generación de Código

```python
import requests

prompt = """Escribe una función en Python que:
1. Reciba una lista de números
2. Devuelva los números pares ordenados de mayor a menor
3. Use list comprehension"""

response = requests.post("http://localhost:8080/v1/chat/completions", json={
    "messages": [{"role": "user", "content": prompt}],
    "max_tokens": 512,
    "temperature": 0.3,  # Más determinista para código
})

print(response.json()["choices"][0]["message"]["content"])
```

### 3. Resumen de Texto Largo

```python
import requests

texto_largo = open("documento.txt").read()[:4000]

response = requests.post("http://localhost:8080/v1/chat/completions", json={
    "messages": [
        {"role": "system", "content": "Resume el siguiente texto en 3 puntos clave."},
        {"role": "user", "content": texto_largo},
    ],
    "max_tokens": 256,
    "temperature": 0.5,
})

print(response.json()["choices"][0]["message"]["content"])
```

### 4. Benchmark de Rendimiento

```python
import requests
import time

def benchmark(n_requests=10):
    url = "http://localhost:8080/v1/chat/completions"
    payload = {
        "messages": [{"role": "user", "content": "Di hola"}],
        "max_tokens": 50,
    }

    times = []
    tokens = []

    for i in range(n_requests):
        start = time.time()
        resp = requests.post(url, json=payload)
        elapsed = time.time() - start
        data = resp.json()

        times.append(elapsed)
        tokens.append(data["usage"]["completion_tokens"])

    avg_time = sum(times) / len(times)
    avg_tokens = sum(tokens) / len(tokens)
    avg_tps = avg_tokens / avg_time

    print(f"Requests:     {n_requests}")
    print(f"Tiempo avg:   {avg_time:.2f}s")
    print(f"Tokens avg:   {avg_tokens:.0f}")
    print(f"Tokens/seg:   {avg_tps:.1f}")

benchmark()
```

### 5. Proxy con Elysia (Backend con Auth)

```typescript
// api-proxy.ts — Proxy con Elysia que añade autenticación
import { Elysia, t } from "elysia";

const LLM_URL = "http://localhost:8080/v1/chat/completions";
const API_KEY = "mi-api-key-secreta";

const app = new Elysia()
  .post("/chat", async ({ body, error, headers }) => {
    if (headers["x-api-key"] !== API_KEY) {
      return error(401, { error: "Unauthorized" });
    }

    const response = await fetch(LLM_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        messages: body.messages,
        max_tokens: body.max_tokens ?? 1024,
        temperature: body.temperature ?? 0.7,
        stream: body.stream ?? false,
      }),
    });

    return await response.json();
  }, {
    body: t.Object({
      messages: t.Array(
        t.Object({
          role: t.String(),
          content: t.String(),
        }),
      ),
      max_tokens: t.Optional(t.Number()),
      temperature: t.Optional(t.Number()),
      stream: t.Optional(t.Boolean()),
    }),
  })
  .listen(3000);

console.log("🚀 Proxy API en http://localhost:3000");
```

Uso del proxy:

```bash
curl -X POST http://localhost:3000/chat \
  -H "Content-Type: application/json" \
  -H "x-api-key: mi-api-key-secreta" \
  -d '{
    "messages": [{"role": "user", "content": "Hola"}],
    "max_tokens": 256
  }'
```

---

## ❌ Manejo de Errores

### Error típico: Server no disponible

```python
import requests

try:
    response = requests.post(
        "http://localhost:8080/v1/chat/completions",
        json={"messages": [{"role": "user", "content": "Hola"}]},
        timeout=30,
    )
    response.raise_for_status()
    print(response.json()["choices"][0]["message"]["content"])
except requests.exceptions.ConnectionError:
    print("❌ Error: llama-server no está corriendo en :8080")
    print("Inícialo con:")
    print("  setsid env VK_ICD_FILENAMES=... ./llama-server/.../llama-server --model ... --port 8080")
except requests.exceptions.Timeout:
    print("⏰ Timeout: la request tardó demasiado")
except KeyError as e:
    print(f"❌ Respuesta inesperada: {e}")
```

### Error típico: Respuesta vacía

Si la respuesta tiene `content` vacío, verificar `finish_reason`:
- `"stop"` — completado normalmente
- `"length"` — alcanzó `max_tokens`
- `"abort"` — interrupted

---

## 🔗 Recursos Relacionados

- [OpenAI API Reference](https://platform.openai.com/docs/api-reference/chat)
- [llama.cpp Server Docs](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [Guía NATIVE-DEPLOY](./NATIVE-DEPLOY.md)
- [Guía TURBOQUANT](./TURBOQUANT.md)
- [Estado del Proyecto](./STATUS.md)
