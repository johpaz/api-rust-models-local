# 📡 Guía para Consumir la API

Guía práctica para interactuar con el LLM API Server con soporte multi-modelo.

## 🔗 Información General

| Propiedad | Valor |
|-----------|-------|
| **URL base (API Rust)** | `http://localhost:9000` |
| **URL base (llama-server directo)** | `http://localhost:8080` |
| **Formato** | OpenAI-compatible |
| **Auth (API Rust)** | Requerida (Bearer Token) |
| **Auth (llama-server)** | No requerida |
| **Multi-Modelo** | ✅ Sí, selección por request |

### Características Multi-Modelo

El API ahora soporta **múltiples modelos** simultáneamente:

1. **Descubrimiento automático**: Escanea el directorio `models/` al iniciar
2. **Listar modelos**: Endpoint `/v1/models` retorna todos los disponibles
3. **Selección por request**: Especifica cuál modelo usar en cada petición
4. **Fallback inteligente**: Si el modelo no existe, usa el default

### Formato de request con modelo

```json
{
  "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
  "messages": [{"role": "user", "content": "Tu pregunta aquí"}],
  "max_tokens": 1024
}
```

**Nota:** El campo `model` es **opcional**. Si no se especifica, se usa el modelo por defecto.

---

## 🚀 Cómo Seleccionar un Modelo

### Paso 1: Listar Modelos Disponibles

#### curl
```bash
curl http://localhost:9000/v1/models \
  -H "Authorization: Bearer $API_TOKEN"
```

#### Python
```python
import requests

response = requests.get(
    "http://localhost:9000/v1/models",
    headers={"Authorization": f"Bearer {API_TOKEN}"}
)

models = response.json()
for model in models["data"]:
    size_gb = model["size_bytes"] / 1e9 if model["size_bytes"] else 0
    print(f"• {model['name']} ({size_gb:.2f} GB)")
```

#### Node.js
```javascript
const response = await fetch("http://localhost:9000/v1/models", {
  headers: { "Authorization": `Bearer ${API_TOKEN}` }
});

const models = await response.json();
models.data.forEach(model => {
  const sizeGB = (model.size_bytes / 1e9).toFixed(2);
  console.log(`• ${model.name} (${sizeGB} GB)`);
});
```

### Paso 2: Usar un Modelo Específico

Una vez que conoces los modelos disponibles, usa el campo `model`:

```bash
curl -X POST http://localhost:9000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
    "messages": [
      {"role": "user", "content": "¿Qué es Rust?"}
    ],
    "max_tokens": 1024
  }'
```

---

## 🚀 Chat Completions

### API Rust (Puerto 9000 - Con Auth y Multi-Modelo)

#### curl (modelo específico)

```bash
curl -X POST http://localhost:9000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
    "messages": [
      {"role": "user", "content": "¿Qué es Rust?"}
    ],
    "max_tokens": 1024,
    "temperature": 0.7
  }'
```

#### curl (sin especificar modelo - usa default)

```bash
curl -X POST http://localhost:9000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "messages": [
      {"role": "user", "content": "¿Qué es Rust?"}
    ],
    "max_tokens": 1024
  }'
```

### llama-server Directo (Puerto 8080 - Sin Auth)

> **Nota:** Al acceder directamente al llama-server, el campo `model` es informativo pero no cambia el modelo cargado.

#### curl (mínimo)

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

#### curl (completo con opciones)

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

# Con modelo específico
response = requests.post(
    "http://localhost:9000/v1/chat/completions",
    headers={"Authorization": f"Bearer {API_TOKEN}"},
    json={
        "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
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
    base_url="http://localhost:9000/v1",
    api_key="mi-token-seguro",
)

# Listar modelos disponibles
print("📋 Modelos disponibles:")
models = client.models.list()
for model in models.data:
    print(f"  • {model.id}")

# Usar un modelo específico
response = client.chat.completions.create(
    model="google_gemma-4-E4B-it-Q4_K_M.gguf",
    messages=[{"role": "user", "content": "¿Qué es Rust?"}],
    max_tokens=1024,
)

print(response.choices[0].message.content)
```

### JavaScript/Node.js (fetch)

```javascript
// Con modelo específico
const response = await fetch("http://localhost:9000/v1/chat/completions", {
  method: "POST",
  headers: { 
    "Content-Type": "application/json",
    "Authorization": `Bearer ${API_TOKEN}`
  },
  body: JSON.stringify({
    model: "google_gemma-4-E4B-it-Q4_K_M.gguf",
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
  baseURL: "http://localhost:9000/v1",
  apiKey: "mi-token-seguro",
});

// Listar modelos
const models = await openai.models.list();
console.log("Modelos disponibles:");
for (const model of models.data) {
  console.log(`  • ${model.id}`);
}

// Usar modelo específico
const response = await openai.chat.completions.create({
  model: "google_gemma-4-E4B-it-Q4_K_M.gguf",
  messages: [{ role: "user", content: "¿Qué es Rust?" }],
  max_tokens: 1024,
});

console.log(response.choices[0].message.content);
```

### Bun (fetch directo)

```typescript
// cliente.ts
const API_TOKEN = "tu-token-aqui";

const response = await fetch(
  "http://localhost:9000/v1/chat/completions",
  {
    method: "POST",
    headers: { 
      "Content-Type": "application/json",
      "Authorization": `Bearer ${API_TOKEN}`
    },
    body: JSON.stringify({
      model: "google_gemma-4-E4B-it-Q4_K_M.gguf",
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
2. URL: `http://localhost:9000/v1/chat/completions`
3. Headers: 
   - `Content-Type: application/json`
   - `Authorization: Bearer tu-token-aqui`
4. Body (JSON):

```json
{
  "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
  "messages": [
    { "role": "user", "content": "¿Qué es Rust?" }
  ],
  "max_tokens": 512,
  "temperature": 0.7
}
```

**Tip:** En Postman, puedes crear una colección con diferentes modelos para probar rápidamente cada uno.

---

## 📋 Streaming (SSE)

Respuesta token por token en tiempo real.

### curl (con modelo específico)

```bash
curl -X POST http://localhost:9000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
    "messages": [
      {"role": "user", "content": "Explica async/await en Rust"}
    ],
    "stream": true,
    "max_tokens": 1024
  }'
```

### Python (streaming con modelo específico)

```python
import requests
import json

response = requests.post(
    "http://localhost:9000/v1/chat/completions",
    headers={"Authorization": f"Bearer {API_TOKEN}"},
    json={
        "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
        "messages": [{"role": "user", "content": "Explica async/await"}],
        "stream": True,
        "max_tokens": 1024,
    },
    stream=True,
)

print("Generando respuesta...\n")
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

client = OpenAI(
    base_url="http://localhost:9000/v1", 
    api_key="mi-token-seguro"
)

stream = client.chat.completions.create(
    model="google_gemma-4-E4B-it-Q4_K_M.gguf",
    messages=[{"role": "user", "content": "Explica async/await"}],
    max_tokens=1024,
    stream=True,
)

print("Generando...\n")
for chunk in stream:
    token = chunk.choices[0].delta.content or ""
    print(token, end="", flush=True)
print()
```

### JavaScript/Node.js (streaming)

```javascript
const response = await fetch("http://localhost:9000/v1/chat/completions", {
  method: "POST",
  headers: { 
    "Content-Type": "application/json",
    "Authorization": `Bearer ${API_TOKEN}`
  },
  body: JSON.stringify({
    model: "google_gemma-4-E4B-it-Q4_K_M.gguf",
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
  baseURL: "http://localhost:9000/v1",
  apiKey: "mi-token-seguro",
});

const stream = await openai.chat.completions.create({
  model: "google_gemma-4-E4B-it-Q4_K_M.gguf",
  messages: [{ role: "user", content: "Explica async/await" }],
  stream: true,
});

for await (const chunk of stream) {
  process.stdout.write(chunk.choices[0]?.delta?.content || "");
}
console.log();
```

---

## 🖼️ Generación de Imágenes

> ⚠️ **Nota:** Este endpoint requiere un backend de imágenes dedicado (FLUX, Stable Diffusion).

### curl

```bash
curl -X POST http://localhost:9000/v1/images/generations \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "model": "flux-schnell",
    "prompt": "Un paisaje montañosco al atardecer con un río cristalino",
    "size": "1024x1024",
    "n": 1
  }'
```

### Python

```python
import requests

response = requests.post(
    "http://localhost:9000/v1/images/generations",
    headers={"Authorization": f"Bearer {API_TOKEN}"},
    json={
        "model": "flux-schnell",
        "prompt": "Un gato astronauta flotando en el espacio",
        "size": "1024x1024",
        "n": 1
    }
)

data = response.json()
print(f"Imagen generada en: {data['data'][0]['url']}")
```

---

## 🎤 Audio: Transcripción (Whisper)

### curl

```bash
curl -X POST http://localhost:9000/v1/audio/transcriptions \
  -H "Authorization: Bearer $API_TOKEN" \
  -F "file=@mi-audio.mp3" \
  -F "model=whisper-large-v3-turbo.gguf" \
  -F "language=es"
```

### Python (requests)

```python
import requests

API_TOKEN = "tu-token-aqui"

# Transcribir archivo de audio
with open("audio.mp3", "rb") as f:
    response = requests.post(
        "http://localhost:9000/v1/audio/transcriptions",
        headers={"Authorization": f"Bearer {API_TOKEN}"},
        files={"file": f},
        data={
            "model": "whisper-large-v3-turbo.gguf",
            "language": "es"
        }
    )

data = response.json()
print(f"Transcripción: {data['text']}")
print(f"Idioma: {data.get('language', 'desconocido')}")
if data.get('duration'):
    print(f"Duración: {data['duration']}s")
```

### Python (Seleccionar Modelo Whisper)

```python
import requests
import os

API_TOKEN = "tu-token-aqui"

# Modelos Whisper disponibles
WHISPER_MODELS = {
    "rapido": "whisper-large-v3-turbo.gguf",    # 1.6GB, rápido
    "preciso": "whisper-large-v3.gguf",         # 3GB, más preciso
    "ligero": "whisper-small.gguf",             # 466MB, básico
}

def transcribir_audio(archivo, modelo_key="rapido", idioma="es"):
    """Transcribe audio con el modelo seleccionado"""
    modelo = WHISPER_MODELS.get(modelo_key, WHISPER_MODELS["rapido"])
    
    with open(archivo, "rb") as f:
        response = requests.post(
            "http://localhost:9000/v1/audio/transcriptions",
            headers={"Authorization": f"Bearer {API_TOKEN}"},
            files={"file": f},
            data={
                "model": modelo,
                "language": idioma,
                "response_format": "json"
            }
        )
    
    return response.json()

# Ejemplo: Transcripción rápida
print("🎤 Transcripción rápida:")
resultado = transcribir_audio("reunion.mp3", "rapido")
print(f"Texto: {resultado['text'][:200]}...")

# Ejemplo: Transcripción precisa
print("\n🎤 Transcripción precisa:")
resultado = transcribir_audio("conferencia.mp3", "preciso")
print(f"Texto: {resultado['text'][:200]}...")
```

### Node.js (fetch con form-data)

```javascript
import fs from 'fs';
import fetch from 'node-fetch';
import FormData from 'form-data';

const form = new FormData();
form.append('file', fs.createReadStream('audio.mp3'));
form.append('model', 'whisper-large-v3.gguf');
form.append('language', 'es');

const response = await fetch('http://localhost:9000/v1/audio/transcriptions', {
  method: 'POST',
  headers: {
    'Authorization': `Bearer ${API_TOKEN}`,
  },
  body: form
});

const data = await response.json();
console.log('Transcripción:', data.text);
```

---

## 🔊 Audio: Text-to-Speech

### curl

```bash
curl -X POST http://localhost:9000/v1/audio/speech \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "model": "tts-1",
    "input": "Hola, bienvenido a nuestra aplicación",
    "voice": "alloy",
    "response_format": "mp3",
    "speed": 1.0
  }' \
  --output audio.mp3
```

### Python

```python
import requests

API_TOKEN = "tu-token-aqui"

response = requests.post(
    "http://localhost:9000/v1/audio/speech",
    headers={"Authorization": f"Bearer {API_TOKEN}"},
    json={
        "model": "tts-1",
        "input": "Hola, este es un ejemplo de texto a voz",
        "voice": "alloy",
        "response_format": "mp3",
        "speed": 1.0
    }
)

# Guardar archivo de audio
with open("output.mp3", "wb") as f:
    f.write(response.content)

print("✅ Audio guardado en output.mp3")
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

### Chat Completions

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `model` | string | — | Nombre del modelo (selecciona de los disponibles) |
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

### Generación de Imágenes

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `model` | string | — | Modelo de generación (flux, sd, dall-e) |
| `prompt` | string | — | Descripción de la imagen |
| `size` | string | `1024x1024` | Tamaño (`256x256`, `512x512`, `1024x1024`) |
| `n` | int | 1 | Número de imágenes |
| `response_format` | string | `url` | `url` o `b64_json` |

### Audio: Transcripción (Whisper)

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `file` | file | — | Archivo de audio (mp3, wav, ogg, flac) |
| `model` | string | — | Modelo Whisper GGUF |
| `language` | string | auto | Idioma del audio (`es`, `en`, `fr`, etc.) |
| `prompt` | string | — | Texto de contexto adicional |
| `response_format` | string | `json` | `json`, `text`, `verbose_json` |
| `temperature` | float | 0.0 | Temperatura (0 = determinista) |

### Audio: Text-to-Speech

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `model` | string | — | Modelo TTS |
| `input` | string | — | Texto a convertir |
| `voice` | string | — | Voz (`alloy`, `echo`, `fable`, etc.) |
| `response_format` | string | `mp3` | `mp3`, `opus`, `aac`, `flac` |
| `speed` | float | 1.0 | Velocidad (0.25 - 4.0) |

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

## 🔄 Ejemplos Avanzados Multi-Modelo

### 1. Comparar Respuestas de Múltiples Modelos

```python
#!/usr/bin/env python3
"""
Comparar respuestas de diferentes modelos para la misma pregunta
"""
import requests
import json

API_TOKEN = "tu-token-aqui"
BASE_URL = "http://localhost:9000/v1"

headers = {
    "Authorization": f"Bearer {API_TOKEN}",
    "Content-Type": "application/json"
}

# Obtener lista de modelos
models_response = requests.get(f"{BASE_URL}/models", headers=headers)
available_models = [m["name"] for m in models_response.json()["data"]]

print(f"📋 Modelos disponibles: {len(available_models)}\n")

# Pregunta para comparar
question = "¿Qué es la programación funcional?"

print(f"Pregunta: {question}\n")
print("=" * 60)

# Probar cada modelo
for model_name in available_models:
    print(f"\n🤖 Usando: {model_name}")
    print("-" * 60)
    
    response = requests.post(
        f"{BASE_URL}/chat/completions",
        headers=headers,
        json={
            "model": model_name,
            "messages": [{"role": "user", "content": question}],
            "max_tokens": 256,
            "temperature": 0.7
        }
    )
    
    if response.status_code == 200:
        data = response.json()
        content = data["choices"][0]["message"]["content"]
        print(content[:300] + "..." if len(content) > 300 else content)
    else:
        print(f"❌ Error: {response.status_code}")
    
    print("\n")
```

### 2. Router Inteligente por Tarea

```python
#!/usr/bin/env python3
"""
Router inteligente: selecciona el mejor modelo según la tarea
"""
import requests

API_TOKEN = "tu-token-aqui"
BASE_URL = "http://localhost:9000/v1"

headers = {"Authorization": f"Bearer {API_TOKEN}"}

# Configuración de modelos
MODEL_CONFIG = {
    "rapido": "google_gemma-4-E4B-it-Q4_K_M.gguf",      # 7.5B - rápido
    "calidad": "Qwen3.5-9B.Q8_0.gguf",                  # 9B - buena calidad
    "complejo": "nvidia_Nemotron-Cascade-2-30B-A3B-IQ2_M.gguf",  # 30B - tareas complejas
}

def select_model(task_type: str) -> str:
    """Selecciona el mejor modelo según el tipo de tarea"""
    return MODEL_CONFIG.get(task_type, MODEL_CONFIG["rapido"])

def chat(message: str, task_type: str = "rapido"):
    """Envía mensaje con el modelo apropiado"""
    model = select_model(task_type)
    
    response = requests.post(
        f"{BASE_URL}/chat/completions",
        headers=headers,
        json={
            "model": model,
            "messages": [{"role": "user", "content": message}],
            "max_tokens": 512,
            "temperature": 0.7
        }
    )
    
    return response.json()["choices"][0]["message"]["content"]

# Ejemplos de uso
print("💬 Tarea rápida (modelo ligero):")
print(chat("¿Cuánto es 2+2?", "rapido"))

print("\n💬 Tarea de calidad media:")
print(chat("Explica qué es una API REST", "calidad"))

print("\n💬 Tarea compleja (modelo grande):")
print(chat("Explica el teorema de Bayes con ejemplos", "complejo"))
```

### 3. Chat Interactivo con Selección de Modelo

```python
#!/usr/bin/env python3
"""
Chat interactivo que permite cambiar de modelo en cualquier momento
"""
import requests

API_TOKEN = "tu-token-aqui"
BASE_URL = "http://localhost:9000/v1"

headers = {"Authorization": f"Bearer {API_TOKEN}"}

# Obtener modelos disponibles
models_response = requests.get(f"{BASE_URL}/models", headers=headers)
models = [m["name"] for m in models_response.json()["data"]]

print("🤖 Chat Multi-Modelo")
print("=" * 60)
print("Modelos disponibles:")
for i, model in enumerate(models, 1):
    print(f"  {i}. {model}")
print(f"\nComandos especiales:")
print(f"  /model <número> - Cambiar modelo")
print(f"  /models - Listar modelos")
print(f"  /salir - Terminar")
print("=" * 60)

# Modelo actual
current_model_idx = 0
messages = []

while True:
    current_model = models[current_model_idx]
    print(f"\n[{current_model}]")
    
    user_input = input("\nTú: ").strip()
    
    if not user_input:
        continue
    
    # Comandos especiales
    if user_input.lower() in ["/salir", "/exit", "/quit"]:
        print("👋 ¡Hasta luego!")
        break
    
    if user_input.lower() == "/models":
        print("\n📋 Modelos disponibles:")
        for i, model in enumerate(models, 1):
            marker = "← actual" if i-1 == current_model_idx else ""
            print(f"  {i}. {model} {marker}")
        continue
    
    if user_input.startswith("/model "):
        try:
            new_idx = int(user_input.split()[1]) - 1
            if 0 <= new_idx < len(models):
                current_model_idx = new_idx
                print(f"✅ Modelo cambiado a: {models[current_model_idx]}")
                messages = []  # Limpiar historial
            else:
                print(f"❌ Número inválido (1-{len(models)})")
        except (ValueError, IndexError):
            print(f"❌ Uso: /model <1-{len(models)}>")
        continue
    
    # Agregar mensaje
    messages.append({"role": "user", "content": user_input})
    
    # Enviar request
    try:
        response = requests.post(
            f"{BASE_URL}/chat/completions",
            headers=headers,
            json={
                "model": models[current_model_idx],
                "messages": messages,
                "max_tokens": 1024,
                "temperature": 0.7
            },
            timeout=120
        )
        
        if response.status_code == 200:
            data = response.json()
            assistant_msg = data["choices"][0]["message"]["content"]
            messages.append({"role": "assistant", "content": assistant_msg})
            print(f"\nAsistente: {assistant_msg}")
        else:
            print(f"\n❌ Error: {response.status_code}")
            
    except requests.exceptions.Timeout:
        print("\n⏰ Timeout: el modelo tardó demasiado")
    except requests.exceptions.ConnectionError:
        print("\n❌ Error: no se puede conectar al servidor")
```

### 4. Benchmark de Múltiples Modelos

```python
#!/usr/bin/env python3
"""
Benchmark: comparar velocidad y calidad de diferentes modelos
"""
import requests
import time
import statistics

API_TOKEN = "tu-token-aqui"
BASE_URL = "http://localhost:9000/v1"

headers = {"Authorization": f"Bearer {API_TOKEN}"}

# Obtener modelos
models_response = requests.get(f"{BASE_URL}/models", headers=headers)
models = [m["name"] for m in models_response.json()["data"]]

print("🏁 Benchmark Multi-Modelo")
print("=" * 60)

# Test prompt
test_prompts = [
    "Explica qué es Rust en 3 frases.",
    "¿Cuál es la diferencia entre async/await y promesas?",
    "Resume las ventajas de los sistemas de tipos estáticos.",
]

results = []

for model in models:
    print(f"\n🧪 Probando: {model}")
    times = []
    tokens = []
    
    for prompt in test_prompts:
        start = time.time()
        
        response = requests.post(
            f"{BASE_URL}/chat/completions",
            headers=headers,
            json={
                "model": model,
                "messages": [{"role": "user", "content": prompt}],
                "max_tokens": 256,
                "temperature": 0.7
            },
            timeout=120
        )
        
        elapsed = time.time() - start
        
        if response.status_code == 200:
            data = response.json()
            completion_tokens = data.get("usage", {}).get("completion_tokens", 0)
            times.append(elapsed)
            tokens.append(completion_tokens)
            print(f"  ✓ {elapsed:.2f}s ({completion_tokens} tokens)")
        else:
            print(f"  ✗ Error {response.status_code}")
    
    if times:
        results.append({
            "model": model,
            "avg_time": statistics.mean(times),
            "avg_tokens": statistics.mean(tokens),
            "avg_tps": statistics.mean([t/p for t, p in zip(times, tokens)])
        })

# Mostrar resultados
print("\n" + "=" * 60)
print("📊 Resultados:")
print("=" * 60)

for result in sorted(results, key=lambda x: x["avg_time"]):
    print(f"\nModelo: {result['model']}")
    print(f"  Tiempo promedio: {result['avg_time']:.2f}s")
    print(f"  Tokens promedio: {result['avg_tokens']:.0f}")
    print(f"  Tokens/seg: {result['avg_tps']:.1f}")
```

### 5. Cola de Procesamiento con Múltiples Modelos

```python
#!/usr/bin/env python3
"""
Procesar múltiples prompts en cola usando diferentes modelos
"""
import requests
import json
from concurrent.futures import ThreadPoolExecutor, as_completed

API_TOKEN = "tu-token-aqui"
BASE_URL = "http://localhost:9000/v1"

headers = {"Authorization": f"Bearer {API_TOKEN}"}

# Obtener modelos
models_response = requests.get(f"{BASE_URL}/models", headers=headers)
models = [m["name"] for m in models_response.json()["data"]]

# Cola de tareas
tasks = [
    {"model": models[0], "prompt": "¿Qué es Python?"},
    {"model": models[1] if len(models) > 1 else models[0], "prompt": "¿Qué es Rust?"},
    {"model": models[0], "prompt": "¿Qué es JavaScript?"},
]

def process_task(task):
    """Procesa una tarea individual"""
    try:
        response = requests.post(
            f"{BASE_URL}/chat/completions",
            headers=headers,
            json={
                "model": task["model"],
                "messages": [{"role": "user", "content": task["prompt"]}],
                "max_tokens": 256,
            },
            timeout=120
        )
        
        if response.status_code == 200:
            data = response.json()
            return {
                "model": task["model"],
                "prompt": task["prompt"],
                "response": data["choices"][0]["message"]["content"],
                "status": "success"
            }
        else:
            return {
                "model": task["model"],
                "prompt": task["prompt"],
                "error": f"Status {response.status_code}",
                "status": "error"
            }
    except Exception as e:
        return {
            "model": task["model"],
            "prompt": task["prompt"],
            "error": str(e),
            "status": "error"
        }

# Procesar en paralelo
print("🚀 Procesando cola de tareas...")
with ThreadPoolExecutor(max_workers=2) as executor:
    futures = [executor.submit(process_task, task) for task in tasks]
    
    for future in as_completed(futures):
        result = future.result()
        
        print(f"\n{'='*60}")
        print(f"Modelo: {result['model']}")
        print(f"Pregunta: {result['prompt']}")
        
        if result["status"] == "success":
            print(f"Respuesta: {result['response'][:200]}...")
        else:
            print(f"❌ Error: {result['error']}")
```

---

## ❌ Manejo de Errores

### Error: Server no disponible

```python
import requests

try:
    response = requests.post(
        "http://localhost:9000/v1/chat/completions",
        headers={"Authorization": f"Bearer {API_TOKEN}"},
        json={
            "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
            "messages": [{"role": "user", "content": "Hola"}]
        },
        timeout=30,
    )
    response.raise_for_status()
    print(response.json()["choices"][0]["message"]["content"])
except requests.exceptions.ConnectionError:
    print("❌ Error: API server no está corriendo en :9000")
    print("Inícialo con:")
    print("  cd api && cargo run")
except requests.exceptions.Timeout:
    print("⏰ Timeout: la request tardó demasiado")
except KeyError as e:
    print(f"❌ Respuesta inesperada: {e}")
```

### Error: Modelo no encontrado

```python
import requests

response = requests.post(
    "http://localhost:9000/v1/chat/completions",
    headers={"Authorization": f"Bearer {API_TOKEN}"},
    json={
        "model": "modelo-que-no-existe.gguf",
        "messages": [{"role": "user", "content": "Hola"}]
    }
)

if response.status_code == 200:
    # El API usa fallback al modelo default
    print("⚠️ Modelo no encontrado, usó el default")
    print(response.json())
else:
    print(f"❌ Error: {response.status_code}")
    print(response.text)
```

### Error: Token inválido

```python
import requests

response = requests.post(
    "http://localhost:9000/v1/chat/completions",
    headers={"Authorization": "Bearer token-invalido"},
    json={
        "messages": [{"role": "user", "content": "Hola"}]
    }
)

if response.status_code == 401:
    print("❌ No autorizado: token inválido o faltante")
    print("Obtén tu token del archivo .env: API_TOKEN=...")
```

### Error: Rate Limit Excedido

```python
import requests

response = requests.post(
    "http://localhost:9000/v1/chat/completions",
    headers={"Authorization": f"Bearer {API_TOKEN}"},
    json={
        "messages": [{"role": "user", "content": "Hola"}]
    }
)

if response.status_code == 429:
    print("⚠️ Demasiadas requests - rate limit excedido")
    print("Espera antes de intentar nuevamente")
    print(f"Configuración actual:")
    print(f"  RATE_LIMIT_REQUESTS=100")
    print(f"  RATE_LIMIT_SECONDS=60")
```

### Error: Respuesta vacía

Si la respuesta tiene `content` vacío, verificar `finish_reason`:
- `"stop"` — completado normalmente
- `"length"` — alcanzó `max_tokens` (aumenta el límite)
- `"abort"` — interrumpido

```python
response = requests.post(...)
data = response.json()

finish_reason = data["choices"][0].get("finish_reason")
content = data["choices"][0]["message"]["content"]

if not content:
    print(f"⚠️ Respuesta vacía - finish_reason: {finish_reason}")
    
    if finish_reason == "length":
        print("💡 Solución: aumenta max_tokens")
    elif finish_reason == "abort":
        print("💡 Solución: el servidor fue interrumpido")
```

---

## 🔗 Recursos Relacionados

- **[API Documentation](./API.md)** — Documentación completa de endpoints
- **[Multi-Model Guide](./MULTI-MODEL.md)** — Guía detallada de selección de modelos
- **[OpenAI API Reference](https://platform.openai.com/docs/api-reference/chat)**
- **[llama.cpp Server Docs](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)**
- **[Guía NATIVE-DEPLOY](./NATIVE-DEPLOY.md)**
- **[Guía TURBOQUANT](./TURBOQUANT.md)**
- **[Estado del Proyecto](./STATUS.md)**
- **[Modelos Soportados](./MODELS.md)**
