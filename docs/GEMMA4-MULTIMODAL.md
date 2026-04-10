# 👁️🎤 Implementación de Multimodalidad con Gemma 4

## Resumen de Capacidades de Gemma 4

Gemma 4 es un modelo **multimodal nativo** que soporta:

| Variante | Texto | Imágenes | Video | Audio | Contexto |
|----------|-------|----------|-------|-------|----------|
| **E2B** (2.3B) | ✅ | ✅ | ✅ | ✅ | 128K |
| **E4B** (4.5B) | ✅ | ✅ | ✅ | ✅ | 128K |
| **26B A4B** (MoE) | ✅ | ✅ | ❌ | ❌ | 256K |
| **31B** (Dense) | ✅ | ✅ | ❌ | ❌ | 256K |

### Notas Importantes

- ✅ **Todas las variantes** soportan entrada de **imágenes** con relación de aspecto variable
- ✅ **E2B y E4B** soportan **audio nativo** (transcripción y comprensión)
- ❌ **Gemma 4 NO genera imágenes** - solo las entiende/analiza
- ❌ **Gemma 4 NO genera audio** - solo lo entiende/transcribe
- 🎯 **Es un modelo de comprensión, no de generación multimedia**

---

## 🖼️ Visión: Cómo Funciona

### Tokens Especiales

Gemma 4 usa tokens placeholder para indicar dónde van las imágenes:

```
<|image|>  → Marcador de posición para imagen
```

### Configuración de Tokens de Visión

Gemma 4 tiene presupuestos configurables de tokens para el codificador visual:

| Tokens | Uso | Velocidad | Detalle |
|--------|-----|-----------|---------|
| 70 | Clasificación/captions | Muy rápido | Bajo detalle |
| 140 | Descripciones generales | Rápido | Medio |
| 280 | Análisis detallado | Medio | Alto |
| 560 | OCR/documents | Lento | Muy alto |
| 1120 | Máximo detalle | Muy lento | Máximo |

### Formato de Mensaje (Ollama API)

```json
{
  "model": "gemma4:e4b",
  "messages": [
    {
      "role": "user",
      "content": "Describe esta imagen: <|image|>",
      "images": ["<BASE64_IMAGE_DATA>"]
    }
  ],
  "stream": false
}
```

### Ejemplo curl con Imagen

```bash
# Codificar imagen a base64 (Linux: -w 0 elimina saltos de línea)
IMG_B64=$(base64 -w 0 imagen.jpg)

curl http://localhost:11434/api/chat -d "{
  \"model\": \"gemma4:e4b\",
  \"messages\": [{
    \"role\": \"user\",
    \"content\": \"Describe esta imagen en detalle: <|image|>\",
    \"images\": [\"$IMG_B64\"]
  }],
  \"stream\": false
}"
```

### Ejemplo Python con Imagen

```python
import base64
import requests

def image_to_base64(filepath):
    """Convierte imagen a base64 sin saltos de línea"""
    with open(filepath, "rb") as f:
        return base64.b64encode(f.read()).decode('utf-8')

# Cargar imagen
img_b64 = image_to_base64("foto.jpg")

# Enviar a Ollama
response = requests.post(
    "http://localhost:11434/api/chat",
    json={
        "model": "gemma4:e4b",
        "messages": [{
            "role": "user",
            "content": "¿Qué ves en esta imagen? <|image|>",
            "images": [img_b64]
        }],
        "stream": False
    }
)

print(response.json()["message"]["content"])
```

### Múltiples Imágenes

```python
import base64
import requests

def image_to_base64(filepath):
    with open(filepath, "rb") as f:
        return base64.b64encode(f.read()).decode('utf-8')

# Cargar múltiples imágenes
img1 = image_to_base64("foto1.jpg")
img2 = image_to_base64("foto2.jpg")

response = requests.post(
    "http://localhost:11434/api/chat",
    json={
        "model": "gemma4:e4b",
        "messages": [{
            "role": "user",
            # Un <|image|> por cada imagen
            "content": "Compara estas dos imágenes: <|image|> <|image|> ¿Cuáles son las diferencias?",
            "images": [img1, img2]
        }],
        "stream": False
    }
)

print(response.json()["message"]["content"])
```

---

## 🎤 Audio: Cómo Funciona

### Disponibilidad

- ✅ **Solo en E2B y E4B** (~300M parámetros de codificador de audio)
- ❌ **No disponible en 26B o 31B**

### Token Especial

```
<|audio|>  → Marcador de posición para audio
```

### Formato de Mensaje (Ollama API)

```json
{
  "model": "gemma4:e4b",
  "messages": [
    {
      "role": "user",
      "content": "Transcribe este audio: <|audio|>",
      "images": [],
      "audio": ["<BASE64_AUDIO_DATA>"]
    }
  ],
  "stream": false
}
```

### Ejemplo curl con Audio

```bash
# Codificar audio a base64
AUD_B64=$(base64 -w 0 audio.wav)

curl http://localhost:11434/api/chat -d "{
  \"model\": \"gemma4:e4b\",
  \"messages\": [{
    \"role\": \"user\",
    \"content\": \"Transcribe este audio: <|audio|>\",
    \"audio\": [\"$AUD_B64\"]
  }],
  \"stream\": false
}"
```

### Ejemplo Python con Audio

```python
import base64
import requests

def audio_to_base64(filepath):
    """Convierte audio a base64"""
    with open(filepath, "rb") as f:
        return base64.b64encode(f.read()).decode('utf-8')

# Cargar audio
audio_b64 = audio_to_base64("grabacion.wav")

# Enviar a Ollama
response = requests.post(
    "http://localhost:11434/api/chat",
    json={
        "model": "gemma4:e4b",
        "messages": [{
            "role": "user",
            "content": "Transcribe este audio y resume los puntos clave: <|audio|>",
            "audio": [audio_b64]
        }],
        "stream": False
    }
)

print(response.json()["message"]["content"])
```

---

## 🎬 Multimodal: Imagen + Audio + Texto

### Ejemplo Combinado (solo E2B/E4B)

```python
import base64
import requests

def file_to_base64(filepath):
    with open(filepath, "rb") as f:
        return base64.b64encode(f.read()).decode('utf-8')

# Cargar multimedia
img_b64 = file_to_base64("diagrama.jpg")
audio_b64 = file_to_base64("explicacion.wav")

response = requests.post(
    "http://localhost:11434/api/chat",
    json={
        "model": "gemma4:e4b",
        "messages": [{
            "role": "user",
            "content": (
                "Analiza este diagrama: <|image|>\n"
                "Ahora escucha esta explicación y complementa el análisis: <|audio|>\n"
                "Dame un resumen completo combinando ambos."
            ),
            "images": [img_b64],
            "audio": [audio_b64]
        }],
        "stream": False
    }
)

print(response.json()["message"]["content"])
```

---

## 🔧 Configuración para llama.cpp Server

### Parámetros Importantes para Visión

```bash
# Iniciar llama-server con Gemma 4 multimodal
./llama-server \
  --model gemma-4-e4b-it-Q4_K_M.gguf \
  --host 0.0.0.0 \
  --port 8080 \
  --ctx-size 8192 \
  --n-gpu-layers 35 \
  --flash-attn \
  --cache-type-k q4_0 \
  --cache-type-v q4_0
```

### Formato de Request (llama-server /completion)

```json
{
  "prompt": "Describe esta imagen: <|image|>",
  "image_data": [
    {
      "data": "<BASE64_IMAGE>",
      "id": 1
    }
  ],
  "temperature": 0.7,
  "n_predict": 512,
  "stream": false
}
```

### Ejemplo curl para llama-server

```bash
IMG_B64=$(base64 -w 0 imagen.jpg)

curl http://localhost:8080/completion -d "{
  \"prompt\": \"Describe esta imagen en detalle: <|image|>\",
  \"image_data\": [{\"data\": \"$IMG_B64\", \"id\": 1}],
  \"temperature\": 0.7,
  \"n_predict\": 512,
  \"stream\": false
}"
```

---

## 🚀 Implementación en Nuestra API Rust

### Lo que Necesitamos Agregar

#### 1. Actualizar el Endpoint de Chat

El endpoint `/v1/chat/completions` debe aceptar:

```rust
#[derive(Debug, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub images: Option<Vec<String>>,  // Base64 encoded images
    pub audio: Option<Vec<String>>,   // Base64 encoded audio
}
```

#### 2. Formato de Prompt para Gemma 4

```rust
fn format_gemma4_multimodal_prompt(
    text: &str,
    images: &[String],
    audio: &[String],
) -> String {
    let mut prompt = text.to_string();
    
    // Agregar marcadores de imagen
    for _ in images {
        prompt = prompt.replace("<|image|>", "<|image|>", 1);
    }
    
    // Agregar marcadores de audio
    for _ in audio {
        prompt = prompt.replace("<|audio|>", "<|audio|>", 1);
    }
    
    prompt
}
```

#### 3. Enviar a llama-server

```rust
// Para llama-server, enviar image_data en el request
let request = CompletionRequest {
    prompt: formatted_prompt,
    image_data: images.iter().map(|img| ImageData {
        data: img.clone(),
        id: 1,
    }).collect(),
    // ... otros campos
};
```

---

## 📋 Ejemplos Prácticos Completos

### 1. Descripción de Imagen (Python)

```python
import base64
import requests

def describe_image(image_path: str, model: str = "gemma4:e4b") -> str:
    """Describe una imagen usando Gemma 4"""
    
    # Codificar imagen
    with open(image_path, "rb") as f:
        img_b64 = base64.b64encode(f.read()).decode('utf-8')
    
    # Enviar a Ollama
    response = requests.post(
        "http://localhost:11434/api/chat",
        json={
            "model": model,
            "messages": [{
                "role": "user",
                "content": "Describe esta imagen en detalle, incluyendo colores, objetos, texto si hay, y el contexto general: <|image|>",
                "images": [img_b64]
            }],
            "stream": False
        }
    )
    
    return response.json()["message"]["content"]

# Uso
descripcion = describe_image("foto_vacaciones.jpg")
print(descripcion)
```

### 2. OCR / Extracción de Texto de Imagen

```python
def extract_text_from_image(image_path: str) -> str:
    """Extrae texto de una imagen (OCR)"""
    
    with open(image_path, "rb") as f:
        img_b64 = base64.b64encode(f.read()).decode('utf-8')
    
    response = requests.post(
        "http://localhost:11434/api/chat",
        json={
            "model": "gemma4:e4b",
            "messages": [{
                "role": "user",
                "content": "Extrae TODO el texto visible en esta imagen. Mantén el formato original: <|image|>",
                "images": [img_b64]
            }],
            "stream": False
        }
    )
    
    return response.json()["message"]["content"]

# Uso
texto = extract_text_from_image("documento_escaneado.jpg")
print(texto)
```

### 3. Transcripción de Audio

```python
def transcribe_audio(audio_path: str) -> str:
    """Transcribe audio usando Gemma 4 E4B"""
    
    with open(audio_path, "rb") as f:
        audio_b64 = base64.b64encode(f.read()).decode('utf-8')
    
    response = requests.post(
        "http://localhost:11434/api/chat",
        json={
            "model": "gemma4:e4b",  // Debe ser E2B o E4B para audio
            "messages": [{
                "role": "user",
                "content": "Transcribe este audio textualmente: <|audio|>",
                "audio": [audio_b64]
            }],
            "stream": False
        }
    )
    
    return response.json()["message"]["content"]

# Uso
transcripcion = transcribe_audio("reunion.mp3")
print(transcripcion)
```

### 4. Análisis Multimodal (Imagen + Audio)

```python
def analyze_multimedia(image_path: str, audio_path: str) -> str:
    """Analiza imagen y audio juntos"""
    
    def to_base64(path):
        with open(path, "rb") as f:
            return base64.b64encode(f.read()).decode('utf-8')
    
    response = requests.post(
        "http://localhost:11434/api/chat",
        json={
            "model": "gemma4:e4b",
            "messages": [{
                "role": "user",
                "content": (
                    "Primero analiza esta imagen: <|image|>\n"
                    "Luego escucha este audio: <|audio|>\n"
                    "Finalmente, explica cómo se relacionan ambos."
                ),
                "images": [to_base64(image_path)],
                "audio": [to_base64(audio_path)]
            }],
            "stream": False
        }
    )
    
    return response.json()["message"]["content"]

# Uso
analisis = analyze_multimedia("diagrama.jpg", "explicacion.wav")
print(analisis)
```

---

## ⚠️ Limitaciones Importantes

### Lo que Gemma 4 **NO** puede hacer:

❌ **Generar imágenes** - No es un modelo de generación como DALL-E o Stable Diffusion  
❌ **Generar audio/TTS** - No convierte texto a voz  
❌ **Editar imágenes** - No modifica imágenes existentes  
❌ **Crear videos** - No genera contenido visual en movimiento  

### Lo que Gemma 4 **SÍ** puede hacer:

✅ **Describir imágenes** - Analiza y describe fotos  
✅ **OCR** - Extrae texto de imágenes  
✅ **Entender diagramas** - Interpreta gráficos, diagramas, charts  
✅ **Transcribir audio** - Convierte audio a texto (solo E2B/E4B)  
✅ **Análisis multimodal** - Combina visión + audio + texto  
✅ **Responder preguntas sobre imágenes/audio** - QA multimodal  

---

## 🎯 Recomendaciones de Uso

### Para Visión

| Tarea | Modelo | Tokens Visión | Temperatura |
|-------|--------|---------------|-------------|
| Clasificación rápida | E2B | 70 | 0.1 |
| Descripción general | E4B | 140 | 0.7 |
| Análisis detallado | E4B | 280 | 0.7 |
| OCR preciso | 26B/31B | 560 | 0.1 |
| Documentos complejos | 31B | 1120 | 0.3 |

### Para Audio

| Tarea | Modelo | Temperatura |
|-------|--------|-------------|
| Transcripción | E4B | 0.0 |
| Resumen de audio | E4B | 0.7 |
| Extracción de info | E4B | 0.5 |

---

## 📚 Recursos

- [Gemma 4 Official Page](https://deepmind.google/models/gemma/gemma-4/)
- [Gemma 4 Documentation](https://ai.google.dev/gemma/docs/core)
- [Gemma 4 Prompt Formatting](https://ai.google.dev/gemma/docs/core/prompt-formatting-gemma4)
- [llama.cpp Server](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [Ollama Gemma 4](https://ollama.com/library/gemma4)
