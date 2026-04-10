# 👁️ API de Visión en Tiempo Real

La API ahora soporta **análisis de visión en tiempo real** para aplicaciones externas. Cualquier app (web, móvil, desktop) puede enviar frames de video y recibir análisis de Gemma 4.

## 🔗 Endpoints de Visión

| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/v1/vision/analyze` | POST | Analizar una imagen (respuesta JSON) |
| `/v1/vision/analyze/stream` | POST | Analizar imagen con streaming SSE |

## 📋 1. Análisis de Imagen Individual

### Request

```bash
POST /v1/vision/analyze
```

**Body JSON:**
```json
{
  "image_base64": "<BASE64_ENCODED_IMAGE>",
  "model": "gemma4:e4b",
  "prompt": "Describe esta imagen en detalle",
  "max_tokens": 1024,
  "temperature": 0.7
}
```

**Parámetros:**

| Campo | Tipo | Requerido | Descripción |
|-------|------|-----------|-------------|
| `image_base64` | string | ✅ Sí | Imagen en base64 (JPEG/PNG) |
| `model` | string | ❌ No | Modelo a usar (default: modelo actual) |
| `prompt` | string | ❌ No | Prompt personalizado |
| `max_tokens` | int | ❌ No | Máximo tokens a generar (default: 1024) |
| `temperature` | float | ❌ No | Creatividad 0-2 (default: 0.7) |

### Response

```json
{
  "id": "uuid-1234-5678",
  "model": "gemma4:e4b",
  "content": "La imagen muestra un paisaje montañosco al atardecer...",
  "created": 1712300000,
  "processing_time_ms": 3456
}
```

### Ejemplo curl

```bash
# Codificar imagen a base64
IMG_B64=$(base64 -w 0 foto.jpg)

# Enviar a la API
curl http://localhost:9000/v1/vision/analyze \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d "{
    \"image_base64\": \"$IMG_B64\",
    \"model\": \"gemma4:e4b\",
    \"prompt\": \"¿Qué ves en esta imagen?\",
    \"max_tokens\": 512
  }"
```

### Ejemplo Python

```python
import base64
import requests

API_TOKEN = "tu-token-aqui"

def analyze_image(image_path: str, prompt: str = None):
    """Analiza una imagen usando la API de visión"""
    
    # Codificar imagen a base64
    with open(image_path, "rb") as f:
        img_b64 = base64.b64encode(f.read()).decode('utf-8')
    
    # Enviar request
    response = requests.post(
        "http://localhost:9000/v1/vision/analyze",
        headers={
            "Authorization": f"Bearer {API_TOKEN}",
            "Content-Type": "application/json"
        },
        json={
            "image_base64": img_b64,
            "model": "gemma4:e4b",
            "prompt": prompt or "Describe esta imagen en detalle",
            "max_tokens": 1024,
            "temperature": 0.7
        }
    )
    
    return response.json()

# Uso
resultado = analyze_image("foto.jpg", "¿Qué objetos hay en esta imagen?")
print(f"Análisis: {resultado['content']}")
print(f"Tiempo: {resultado['processing_time_ms']}ms")
```

### Ejemplo Node.js

```javascript
import fs from 'fs';
import fetch from 'node-fetch';

const API_TOKEN = 'tu-token-aqui';

async function analyzeImage(imagePath) {
  // Leer y codificar imagen
  const imageBuffer = fs.readFileSync(imagePath);
  const imageBase64 = imageBuffer.toString('base64');
  
  // Enviar request
  const response = await fetch('http://localhost:9000/v1/vision/analyze', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${API_TOKEN}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      image_base64: imageBase64,
      model: 'gemma4:e4b',
      prompt: 'Describe esta imagen',
      max_tokens: 1024
    })
  });
  
  const data = await response.json();
  console.log('Análisis:', data.content);
  console.log('Tiempo:', data.processing_time_ms, 'ms');
  return data;
}

// Uso
await analyzeImage('foto.jpg');
```

---

## 🎥 2. Análisis con Streaming (SSE)

Para recibir la respuesta token por token en tiempo real:

### Request

```bash
POST /v1/vision/analyze/stream
```

Mismo body que el endpoint normal, pero la respuesta viene en formato SSE.

### Ejemplo curl

```bash
IMG_B64=$(base64 -w 0 foto.jpg)

curl -N http://localhost:9000/v1/vision/analyze/stream \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d "{
    \"image_base64\": \"$IMG_B64\",
    \"model\": \"gemma4:e4b\",
    \"prompt\": \"Describe esta imagen\"
  }"
```

### Ejemplo Python (Streaming)

```python
import base64
import requests
import json

def analyze_image_stream(image_path: str):
    """Analiza imagen con streaming SSE"""
    
    with open(image_path, "rb") as f:
        img_b64 = base64.b64encode(f.read()).decode('utf-8')
    
    response = requests.post(
        "http://localhost:9000/v1/vision/analyze/stream",
        headers={
            "Authorization": f"Bearer {API_TOKEN}",
            "Content-Type": "application/json"
        },
        json={
            "image_base64": img_b64,
            "model": "gemma4:e4b",
            "prompt": "Describe esta imagen"
        },
        stream=True
    )
    
    print("Analizando...\n")
    for line in response.iter_lines():
        if line:
            line = line.decode("utf-8")
            if line.startswith("data: ") and line != "data: [DONE]":
                chunk = json.loads(line[6:])
                print(chunk['content'], end="", flush=True)
    print()

# Uso
analyze_image_stream("foto.jpg")
```

---

## 📹 3. Implementación de Visión en Tiempo Real (Cámara)

### Aplicación Web Incluida

El proyecto incluye **`examples/vision-realtime.html`**, una aplicación web completa que:

- ✅ Captura frames de la cámara en tiempo real
- ✅ Envía frames automáticamente a la API
- ✅ Muestra análisis continuos
- ✅ Historial de análisis
- ✅ Configuración de modelo, intervalo, resolución

**Uso:**

```bash
# Abrir el archivo HTML directamente
firefox examples/vision-realtime.html

# O servir con Python
cd examples
python -m http.server 8080
# Abrir http://localhost:8080/vision-realtime.html
```

### Implementación en Tu Aplicación

#### JavaScript (Navegador - Captura de Cámara)

```javascript
class VisionAnalyzer {
    constructor(apiUrl, apiToken) {
        this.apiUrl = apiUrl;
        this.apiToken = apiToken;
        this.videoStream = null;
        this.analysisInterval = null;
        this.isAnalyzing = false;
    }

    async startCamera() {
        // Iniciar cámara
        this.videoStream = await navigator.mediaDevices.getUserMedia({
            video: { width: 1280, height: 720 },
            audio: false
        });
        
        // Mostrar en elemento <video>
        const videoElement = document.getElementById('video');
        videoElement.srcObject = this.videoStream;
    }

    captureFrame() {
        // Capturar frame actual
        const canvas = document.createElement('canvas');
        const video = document.getElementById('video');
        
        canvas.width = video.videoWidth;
        canvas.height = video.videoHeight;
        
        const ctx = canvas.getContext('2d');
        ctx.drawImage(video, 0, 0);
        
        // Convertir a base64
        return canvas.toDataURL('image/jpeg', 0.8).split(',')[1];
    }

    async analyzeFrame(prompt = null) {
        const imageBase64 = this.captureFrame();
        
        const response = await fetch(`${this.apiUrl}/v1/vision/analyze`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${this.apiToken}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                image_base64: imageBase64,
                model: 'gemma4:e4b',
                prompt: prompt || 'Describe lo que ves',
                max_tokens: 512
            })
        });
        
        const data = await response.json();
        return data;
    }

    startContinuousAnalysis(intervalSeconds = 5, prompt = null) {
        this.isAnalyzing = true;
        
        // Analizar inmediatamente
        this.analyzeFrame(prompt).then(result => {
            console.log('Análisis:', result.content);
        });
        
        // Continuar cada X segundos
        this.analysisInterval = setInterval(async () => {
            if (this.isAnalyzing) {
                const result = await this.analyzeFrame(prompt);
                console.log('Análisis:', result.content);
            }
        }, intervalSeconds * 1000);
    }

    stopAnalysis() {
        this.isAnalyzing = false;
        if (this.analysisInterval) {
            clearInterval(this.analysisInterval);
        }
    }
}

// Uso
const analyzer = new VisionAnalyzer(
    'http://localhost:9000',
    'tu-token-aqui'
);

await analyzer.startCamera();
analyzer.startContinuousAnalysis(5, '¿Qué hay en esta imagen?');
```

#### Python (Cámara + Análisis Continuo)

```python
import cv2
import base64
import requests
import time
import threading

class ContinuousVisionAnalyzer:
    def __init__(self, api_url, api_token):
        self.api_url = api_url
        self.api_token = api_token
        self.cap = None
        self.is_analyzing = False
        self.analysis_thread = None
    
    def start_camera(self):
        """Iniciar captura de cámara"""
        self.cap = cv2.VideoCapture(0)
        self.cap.set(cv2.CAP_PROP_FRAME_WIDTH, 1280)
        self.cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 720)
        
        if not self.cap.isOpened():
            raise Exception("No se pudo abrir la cámara")
    
    def capture_frame_base64(self):
        """Capturar frame y convertir a base64"""
        ret, frame = self.cap.read()
        if not ret:
            return None
        
        # Codificar a JPEG
        _, buffer = cv2.imencode('.jpg', frame, [cv2.IMWRITE_JPEG_QUALITY, 80])
        
        # Convertir a base64
        return base64.b64encode(buffer).decode('utf-8')
    
    def analyze_frame(self, prompt=None):
        """Analizar un frame con la API"""
        image_base64 = self.capture_frame_base64()
        if not image_base64:
            return None
        
        response = requests.post(
            f"{self.api_url}/v1/vision/analyze",
            headers={
                "Authorization": f"Bearer {self.api_token}",
                "Content-Type": "application/json"
            },
            json={
                "image_base64": image_base64,
                "model": "gemma4:e4b",
                "prompt": prompt or "Describe lo que ves",
                "max_tokens": 512
            }
        )
        
        return response.json()
    
    def _analysis_loop(self, interval_seconds, prompt):
        """Loop de análisis continuo"""
        while self.is_analyzing:
            try:
                result = self.analyze_frame(prompt)
                if result:
                    print(f"\n[{time.strftime('%H:%M:%S')}] {result['content']}")
                    print(f"Tiempo: {result['processing_time_ms']}ms\n")
                
                time.sleep(interval_seconds)
            except Exception as e:
                print(f"Error en análisis: {e}")
    
    def start_continuous_analysis(self, interval_seconds=5, prompt=None):
        """Iniciar análisis continuo en thread separado"""
        self.is_analyzing = True
        self.analysis_thread = threading.Thread(
            target=self._analysis_loop,
            args=(interval_seconds, prompt),
            daemon=True
        )
        self.analysis_thread.start()
    
    def stop_analysis(self):
        """Detener análisis continuo"""
        self.is_analyzing = False
        if self.analysis_thread:
            self.analysis_thread.join(timeout=2)
    
    def cleanup(self):
        """Limpiar recursos"""
        self.stop_analysis()
        if self.cap:
            self.cap.release()

# Uso
analyzer = ContinuousVisionAnalyzer(
    "http://localhost:9000",
    "tu-token-aqui"
)

try:
    analyzer.start_camera()
    print("🎥 Cámara iniciada. Presiona Ctrl+C para detener.")
    
    analyzer.start_continuous_analysis(
        interval_seconds=5,
        prompt="Describe detalladamente lo que ves en esta imagen"
    )
    
    # Mantener ejecución
    while True:
        time.sleep(1)
        
except KeyboardInterrupt:
    print("\n⏹️ Deteniendo...")
finally:
    analyzer.cleanup()
```

---

## 🚀 Casos de Uso

### 1. OCR en Tiempo Real

```python
analyzer = ContinuousVisionAnalyzer(API_URL, API_TOKEN)
analyzer.start_camera()
analyzer.start_continuous_analysis(
    interval_seconds=3,
    prompt="Extrae TODO el texto visible en esta imagen. Mantén el formato original."
)
```

### 2. Detección de Objetos

```python
analyzer.start_continuous_analysis(
    interval_seconds=5,
    prompt="Lista todos los objetos, personas y elementos visibles en esta imagen."
)
```

### 3. Accesibilidad (Descripción para Personas Ciegas)

```python
analyzer.start_continuous_analysis(
    interval_seconds=10,
    prompt="Describe el entorno actual de forma clara y concisa. Menciona obstáculos, personas y objetos relevantes."
)
```

### 4. Seguridad y Vigilancia

```python
analyzer.start_continuous_analysis(
    interval_seconds=15,
    prompt="Analiza la escena. ¿Hay algo inusual o sospechoso? Describe personas, acciones y ubicación."
)
```

### 5. Asistente de Cocina

```python
analyzer.start_continuous_analysis(
    interval_seconds=8,
    prompt="¿Qué ingredientes y utensilios ves? ¿Hay algo que necesite atención (quemándose, derramándose)?"
)
```

---

## 📊 Arquitectura

```
┌─────────────────────────────────────────────────────────────────┐
│                    Aplicación Externa                           │
│  (Web App / Mobile App / Desktop App / Python Script)          │
│                                                                 │
│  1. Captura frame de cámara                                    │
│  2. Convierte a base64                                         │
│  3. Envía POST a /v1/vision/analyze                            │
│  4. Recibe análisis de Gemma 4                                 │
└─────────────────────────────────────────────────────────────────┘
                           │
                           │ HTTP POST (JSON con image_base64)
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                     API Rust (:9000)                            │
│                                                                 │
│  Endpoint: POST /v1/vision/analyze                             │
│  - Valida auth token                                           │
│  - Decodifica base64                                           │
│  - Formatea prompt con <|image|>                               │
│  - Envía a llama-server                                        │
└─────────────────────────────────────────────────────────────────┘
                           │
                           │ HTTP POST (completion + image_data)
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                  llama-server (:8080)                           │
│                                                                 │
│  Modelo: gemma-4-e4b-it-Q4_K_M.gguf                            │
│  - Procesa imagen con vision encoder                           │
│  - Genera descripción en texto                                 │
│  - Responde con análisis                                       │
└─────────────────────────────────────────────────────────────────┘
```

---

## ⚙️ Configuración Requerida

### 1. Ollama (para desarrollo local)

```bash
# Descargar Gemma 4 E4B
ollama pull gemma4:e4b

# Iniciar Ollama
ollama serve
```

### 2. llama-server (para producción)

```bash
# Iniciar con Gemma 4 multimodal
./llama-server \
  --model gemma-4-e4b-it-Q4_K_M.gguf \
  --host 0.0.0.0 \
  --port 8080 \
  --ctx-size 8192 \
  --n-gpu-layers 35 \
  --flash-attn
```

### 3. API Rust

```bash
# Configurar .env
API_TOKEN=tu-token-seguro
LLAMA_SERVER_URL=http://localhost:8080

# Iniciar API
cd api && cargo run --release
```

---

## 🔐 Autenticación

Todos los endpoints requieren el header:

```
Authorization: Bearer <tu-token>
```

Sin token: `401 Unauthorized`

---

## ⚡ Rate Limiting

Configurable en `.env`:

```bash
RATE_LIMIT_REQUESTS=100   # 100 requests
RATE_LIMIT_SECONDS=60     # por ventana de 60 segundos
```

Para análisis continuo, configura un límite apropiado:
- **5 segundos** entre frames = 12 requests/minuto
- **10 segundos** entre frames = 6 requests/minuto

---

## 📝 Notas Importantes

### Formatos de Imagen Soportados

- ✅ **JPEG** (recomendado, menor tamaño)
- ✅ **PNG** (mayor calidad, mayor tamaño)
- ✅ **WebP** (buena compresión)

### Tamaño Recomendido

| Resolución | Velocidad | Calidad | Uso |
|------------|-----------|---------|-----|
| 640x480 | Muy rápido | Media | Análisis rápido |
| 1280x720 | Balanceado | Alta | **Recomendado** |
| 1920x1080 | Lento | Máxima | OCR/Detalle |

### Modelos Recomendados

| Modelo | Visión | Audio | Velocidad | Uso |
|--------|--------|-------|-----------|-----|
| `gemma4:e4b` | ✅ | ✅ | Rápido | **Recomendado** |
| `gemma4:e2b` | ✅ | ✅ | Muy rápido | Edge/dispositivos |
| `gemma4:31b` | ✅ | ❌ | Lento | Máxima precisión |

---

## 🔗 Recursos

- [Ejemplo Web Completo](examples/vision-realtime.html)
- [Documentación Gemma 4 Multimodal](docs/GEMMA4-MULTIMODAL.md)
- [Documentación API](docs/API.md)
- [Guía de Consumo](docs/API-CONSUME.md)
