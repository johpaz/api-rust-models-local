# 📡 Documentación de la API

## 🔗 Información General

| Propiedad | Valor |
|-----------|-------|
| **URL base** | `http://localhost:9000` |
| **Formato** | OpenAI-compatible |
| **Auth** | Requerida (Bearer Token) |
| **Multi-Modelo** | ✅ Sí, selección por request |

### Características Principales

- ✅ **Descubrimiento automático** de modelos en el directorio `models/`
- ✅ **Selección de modelo** por request vía parámetro `model`
- ✅ **Streaming SSE** para generación en tiempo real
- ✅ **Rate limiting** configurable
- ✅ **Compatible** con SDKs de OpenAI

---

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
  "context_size": 4096
}
```

---

### `GET /v1/models`

Lista **todos los modelos disponibles** en el directorio `models/`.

```bash
curl http://localhost:9000/v1/models \
  -H "Authorization: Bearer $API_TOKEN"
```

**Respuesta:**
```json
{
  "object": "list",
  "data": [
    {
      "id": "google_gemma-4-E4B-it-Q4_K_M.gguf",
      "object": "model",
      "created": 1712300000,
      "owned_by": "local",
      "name": "google_gemma-4-E4B-it-Q4_K_M.gguf",
      "path": "/home/user/project/models/google_gemma-4-E4B-it-Q4_K_M.gguf",
      "size_bytes": 5100000000
    },
    {
      "id": "Qwen3.5-9B.Q8_0.gguf",
      "object": "model",
      "created": 1712300000,
      "owned_by": "local",
      "name": "Qwen3.5-9B.Q8_0.gguf",
      "path": "/home/user/project/models/Qwen3.5-9B.Q8_0.gguf",
      "size_bytes": 9000000000
    }
  ]
}
```

**Campos de cada modelo:**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| `id` | string | Identificador único (nombre del archivo) |
| `name` | string | Nombre del modelo |
| `path` | string | Ruta completa al archivo `.gguf` |
| `size_bytes` | int | Tamaño del archivo en bytes |

---

### `POST /v1/chat/completions`

Generación de texto con selección de modelo. Compatible con OpenAI.

#### Cómo Seleccionar un Modelo

1. **Obtén la lista de modelos disponibles:**
   ```bash
   curl http://localhost:9000/v1/models -H "Authorization: Bearer $API_TOKEN"
   ```

2. **Usa el campo `model` en tu request:**
   ```bash
   curl http://localhost:9000/v1/chat/completions \
     -H "Authorization: Bearer $API_TOKEN" \
     -d '{
       "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
       "messages": [...]
     }'
   ```

#### Parámetros

| Campo | Tipo | Default | Descripción |
|-------|------|---------|-------------|
| `model` | string | **Opcional** | Nombre del modelo a usar (debe existir en `models/`) |
| `messages` | array | **Requerido** | Historial de conversación |
| `messages[].role` | string | **Requerido** | `system`, `user`, `assistant` |
| `messages[].content` | string | **Requerido** | Contenido del mensaje |
| `temperature` | float | `0.7` | Creatividad (0.0 - 2.0) |
| `max_tokens` | int | `1024` | Máximo tokens a generar |
| `stream` | bool | `true` | Habilitar streaming SSE |
| `stop` | string[] | `[]` | Secuencias de parada |

#### Comportamiento del Parámetro `model`

- ✅ **Si el modelo existe**: Se usa el modelo especificado
- ⚠️ **Si el modelo no existe**: Se usa el modelo por defecto y se loguea un warning
- 🔄 **Si no se especifica**: Se usa el modelo actualmente cargado en llama-server

#### Ejemplo: Usar Modelo Específico

```bash
curl http://localhost:9000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
    "messages": [
      {"role": "system", "content": "Eres un asistente útil."},
      {"role": "user", "content": "Explícame qué es Rust en 3 frases."}
    ],
    "max_tokens": 256,
    "temperature": 0.7,
    "stream": false
  }'
```

#### Ejemplo: Streaming con Modelo Específico

```bash
curl -N http://localhost:9000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "model": "Qwen3.5-9B.Q8_0.gguf",
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

# Listar modelos disponibles
models = client.models.list()
print("Modelos disponibles:")
for model in models.data:
    print(f"  - {model.id}")

# Usar un modelo específico
response = client.chat.completions.create(
    model="google_gemma-4-E4B-it-Q4_K_M.gguf",
    messages=[
        {"role": "user", "content": "¿Qué es la inteligencia artificial?"}
    ],
    max_tokens=512,
    temperature=0.7,
    stream=True
)

print("Generando respuesta...\n")
for chunk in response:
    if chunk.choices[0].delta.content:
        print(chunk.choices[0].delta.content, end="", flush=True)
```

#### Node.js con fetch

```javascript
// Listar modelos
const modelsResponse = await fetch("http://localhost:9000/v1/models", {
  headers: { "Authorization": "Bearer mi-token-seguro" }
});
const models = await modelsResponse.json();
console.log("Modelos disponibles:", models.data.map(m => m.name));

// Usar modelo específico
const response = await fetch("http://localhost:9000/v1/chat/completions", {
  method: "POST",
  headers: {
    "Content-Type": "application/json",
    "Authorization": "Bearer mi-token-seguro"
  },
  body: JSON.stringify({
    model: "google_gemma-4-E4B-it-Q4_K_M.gguf",
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

Generación de imágenes (requiere modelo dedicado).

> ⚠️ **Nota:** llama.cpp no soporta generación de imágenes nativamente. Este endpoint requiere un backend separado como FLUX o Stable Diffusion.

#### Parámetros

| Campo | Tipo | Default | Descripción |
|-------|------|---------|-------------|
| `model` | string | **Requerido** | Modelo de generación de imágenes |
| `prompt` | string | **Requerido** | Descripción de la imagen |
| `size` | string | `"1024x1024"` | Tamaño de la imagen |
| `n` | int | `1` | Número de imágenes a generar |
| `response_format` | string | `"url"` | Formato de respuesta (`url` o `b64_json`) |

#### Ejemplo

```bash
curl http://localhost:9000/v1/images/generations \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "model": "flux-schnell",
    "prompt": "Un gato astronauta en el espacio",
    "size": "1024x1024",
    "n": 1
  }'
```

#### Modelos de Imágenes Soportados

| Modelo | Tipo | Descripción |
|--------|------|-------------|
| `flux-schnell` | FLUX | Generación rápida de imágenes |
| `stable-diffusion-xl` | SDXL | Alta calidad de imagen |
| `dall-e-3` | DALL-E | OpenAI DALL-E 3 |

---

### `POST /v1/audio/speech`

Text-to-Speech: Convierte texto a audio.

> ⚠️ **Nota:** Requiere modelo TTS dedicado (VITS, Bark, etc.).

#### Parámetros

| Campo | Tipo | Default | Descripción |
|-------|------|---------|-------------|
| `model` | string | **Requerido** | Modelo TTS a usar |
| `input` | string | **Requerido** | Texto a convertir |
| `voice` | string | `null` | Voz específica (ej: `alloy`, `echo`, `fable`) |
| `response_format` | string | `"mp3"` | Formato de audio (`mp3`, `opus`, `aac`, `flac`) |
| `speed` | float | `1.0` | Velocidad de habla (0.25 - 4.0) |

#### Ejemplo

```bash
curl http://localhost:9000/v1/audio/speech \
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

#### Modelos TTS Soportados

| Modelo | Tipo | Voces | Idiomas |
|--------|------|-------|---------|
| `tts-1` | OpenAI | 6 voces | Multi-idioma |
| `tts-1-hd` | OpenAI HD | 4 voces | Multi-idioma |
| `piper-voice` | Piper | Varias | 30+ idiomas |

---

### `POST /v1/audio/transcriptions`

Transcripción de audio a texto usando modelo Whisper GGUF.

> ✅ **Soportado:** llama.cpp soporta modelos Whisper en formato GGUF.

#### Parámetros (multipart/form-data)

| Campo | Tipo | Default | Descripción |
|-------|------|---------|-------------|
| `file` | file | **Requerido** | Archivo de audio (mp3, wav, ogg, etc.) |
| `model` | string | **Requerido** | Modelo Whisper GGUF |
| `language` | string | `null` | Idioma del audio (ej: `es`, `en`, `fr`) |
| `prompt` | string | `null` | Texto adicional para contexto |
| `response_format` | string | `"json"` | Formato (`json`, `text`, `verbose_json`) |
| `temperature` | float | `0.0` | Temperatura (0 = determinista) |

#### Ejemplo: Transcripción Básica

```bash
curl http://localhost:9000/v1/audio/transcriptions \
  -H "Authorization: Bearer $API_TOKEN" \
  -F "file=@mi-audio.mp3" \
  -F "model=whisper-large-v3-turbo.gguf" \
  -F "language=es"
```

#### Ejemplo: Transcripción con Contexto

```bash
curl http://localhost:9000/v1/audio/transcriptions \
  -H "Authorization: Bearer $API_TOKEN" \
  -F "file=@reunion.mp3" \
  -F "model=whisper-large-v3.gguf" \
  -F "language=es" \
  -F "prompt=Reunión de equipo sobre proyecto API" \
  -F "response_format=json"
```

#### Ejemplo: Python con Whisper

```python
import requests

API_TOKEN = "tu-token-aqui"

with open("audio.mp3", "rb") as f:
    response = requests.post(
        "http://localhost:9000/v1/audio/transcriptions",
        headers={"Authorization": f"Bearer {API_TOKEN}"},
        files={"file": f},
        data={
            "model": "whisper-large-v3.gguf",
            "language": "es",
            "response_format": "json"
        }
    )

print(response.json()["text"])
```

#### Modelos Whisper Soportados

| Modelo | Tamaño | Params | Idiomas | RAM |
|--------|--------|--------|---------|-----|
| `whisper-tiny.gguf` | 75 MB | 39M | Multi | ~1 GB |
| `whisper-base.gguf` | 142 MB | 74M | Multi | ~1.5 GB |
| `whisper-small.gguf` | 466 MB | 244M | Multi | ~2 GB |
| `whisper-medium.gguf` | 1.5 GB | 769M | Multi | ~4 GB |
| `whisper-large-v3.gguf` | 3 GB | 1550M | Multi | ~8 GB |
| `whisper-large-v3-turbo.gguf` | 1.6 GB | 809M | Multi | ~5 GB |

#### Respuesta

```json
{
  "text": "Hola, bienvenidos a la reunión de hoy. Vamos a discutir el progreso del proyecto.",
  "language": "es",
  "duration": 45.2
}
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

## 📂 Configuración de Modelos

### Variable de Entorno

| Variable | Default | Descripción |
|----------|---------|-------------|
| `MODELS_DIR` | `./models/` | Directorio donde se escanean los modelos `.gguf` |

### Cómo Agregar un Nuevo Modelo

1. **Descarga el modelo al directorio `models/`:**
   ```bash
   ./scripts/download-model.sh bartowski/Qwen3.5-9B-GGUF Qwen3.5-9B.Q8_0.gguf
   ```

2. **Reinicia el API server** para que detecte el nuevo modelo:
   ```bash
   # El API escanea el directorio al iniciar
   ```

3. **Verifica que el modelo está disponible:**
   ```bash
   curl http://localhost:9000/v1/models -H "Authorization: Bearer $API_TOKEN"
   ```

### Modelos Soportados

Todos los modelos en formato **GGUF** son compatibles:

| Modelo | Params | Tamaño | Uso Ideal |
|--------|--------|--------|-----------|
| Gemma 4 E4B | 7.5B | ~4.5 GB | Chat, resumen rápido |
| Gemma 4 31B | 30.7B | ~19 GB | Documentos largos |
| Nemotron Cascade 2 | 30B | ~12 GB | Código, razonamiento |
| Qwen 3.5 35B | 35B | ~20 GB | Multi-idioma, agentic |

## 🔄 Flujo Completo de Selección de Modelo

### Paso 1: Listar Modelos Disponibles

```bash
#!/bin/bash
# Listar modelos
curl -s http://localhost:9000/v1/models \
  -H "Authorization: Bearer $API_TOKEN" | jq -r '.data[].name'
```

**Salida esperada:**
```
google_gemma-4-E4B-it-Q4_K_M.gguf
Qwen3.5-9B.Q8_0.gguf
nvidia_Nemotron-Cascade-2-30B-A3B-IQ2_M.gguf
```

### Paso 2: Seleccionar y Usar un Modelo

```bash
#!/bin/bash
# Usar un modelo específico
MODEL="google_gemma-4-E4B-it-Q4_K_M.gguf"

curl http://localhost:9000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d "{
    \"model\": \"$MODEL\",
    \"messages\": [
      {\"role\": \"user\", \"content\": \"Hola, ¿cómo estás?\"}
    ],
    \"max_tokens\": 256,
    \"temperature\": 0.7
  }"
```

### Paso 3: Cambiar de Modelo en Siguiente Request

```bash
#!/bin/bash
# Ahora usar otro modelo
MODEL="Qwen3.5-9B.Q8_0.gguf"

curl http://localhost:9000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d "{
    \"model\": \"$MODEL\",
    \"messages\": [
      {\"role\": \"user\", \"content\": \"Traduce esto al inglés: Hola mundo\"}
    ],
    \"max_tokens\": 128
  }"
```

## 📊 Ejemplo Completo en Python

```python
#!/usr/bin/env python3
"""
Ejemplo completo: Listar modelos y usar cada uno
"""
import requests
import json

BASE_URL = "http://localhost:9000/v1"
API_TOKEN = "tu-token-aqui"

headers = {
    "Authorization": f"Bearer {API_TOKEN}",
    "Content-Type": "application/json"
}

# 1. Listar modelos disponibles
print("📋 Modelos disponibles:")
models_response = requests.get(f"{BASE_URL}/models", headers=headers)
models = models_response.json()

for model in models["data"]:
    size_gb = model["size_bytes"] / 1e9 if model["size_bytes"] else 0
    print(f"  • {model['name']} ({size_gb:.2f} GB)")

# 2. Usar cada modelo para una tarea diferente
tasks = [
    ("google_gemma-4-E4B-it-Q4_K_M.gguf", "¿Qué es Rust?"),
    ("Qwen3.5-9B.Q8_0.gguf", "¿Qué es Python?"),
]

print("\n🚀 Probando modelos:")
for model_name, question in tasks:
    print(f"\n--- Usando: {model_name} ---")
    
    response = requests.post(
        f"{BASE_URL}/chat/completions",
        headers=headers,
        json={
            "model": model_name,
            "messages": [{"role": "user", "content": question}],
            "max_tokens": 128,
            "temperature": 0.7
        }
    )
    
    if response.status_code == 200:
        data = response.json()
        print(f"Respuesta: {data['choices'][0]['message']['content']}")
    else:
        print(f"❌ Error: {response.status_code} - {response.text}")
```
