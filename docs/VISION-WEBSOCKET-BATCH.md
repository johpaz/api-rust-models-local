# 🔌 WebSocket & Batch Vision API

La API de visión ahora soporta **WebSocket para streaming en tiempo real** y **procesamiento por lotes (batch)** de múltiples imágenes.

## 📋 Endpoints Nuevos

| Endpoint | Método | Tipo | Descripción |
|----------|--------|------|-------------|
| `/v1/vision/analyze` | POST | HTTP | Analizar imagen individual (JSON) |
| `/v1/vision/analyze/batch` | POST | HTTP | Analizar **múltiples imágenes** en lote |
| `/v1/vision/stream/ws` | GET | WebSocket | **Streaming en tiempo real** continuo |

---

## 🔌 1. WebSocket - Streaming en Tiempo Real

### ¿Qué es WebSocket?

WebSocket es una conexión **persistente y bidireccional** entre el cliente y el servidor. Ideal para:

- ✅ **Video en tiempo real** - Enviar frames continuamente sin overhead de HTTP
- ✅ **Análisis continuo** - Recibir resultados token por token mientras se procesan
- ✅ **Baja latencia** - Sin handshake HTTP por cada frame
- ✅ **Control en tiempo real** - Cambiar configuración sin reconectar

### Cómo Funciona

```
Cliente                              Servidor
  │                                    │
  │──── Conecta WS ───────────────────▶│
  │◀─── "ready" ──────────────────────│
  │                                    │
  │──── Envía frame (base64/bin) ─────▶│
  │◀─── "processing" ─────────────────│
  │◀─── "partial" (token) ────────────│
  │◀─── "partial" (token) ────────────│
  │◀─── "complete" (resultado) ───────│
  │                                    │
  │──── Envía siguiente frame ────────▶│
  │◀─── ... ──────────────────────────│
  │                                    │
  │──── Cierra conexión ──────────────▶│
```

### Ejemplo: JavaScript (Navegador)

```javascript
class VisionWebSocket {
    constructor(apiUrl, token) {
        this.url = apiUrl;
        this.token = token;
        this.ws = null;
        this.frameCount = 0;
    }

    connect() {
        return new Promise((resolve, reject) => {
            // WebSocket URL (ws:// or wss:// for secure)
            const wsUrl = this.url.replace('http://', 'ws://').replace('https://', 'wss://');
            
            this.ws = new WebSocket(`${wsUrl}/v1/vision/stream/ws`, [
                'Authorization', `Bearer ${this.token}`
            ]);

            this.ws.onopen = () => {
                console.log('✅ WebSocket conectado');
                resolve();
            };

            this.ws.onmessage = (event) => {
                const msg = JSON.parse(event.data);
                this.handleMessage(msg);
            };

            this.ws.onerror = (error) => {
                console.error('❌ Error WebSocket:', error);
                reject(error);
            };

            this.ws.onclose = () => {
                console.log('🔌 WebSocket desconectado');
            };
        });
    }

    handleMessage(msg) {
        switch (msg.type) {
            case 'ready':
                console.log('🟢 Servidor listo');
                break;
            
            case 'config_ack':
                console.log('⚙️ Configuración actualizada:', msg.model);
                break;
            
            case 'processing':
                console.log(`⏳ Procesando frame #${msg.frame_number}...`);
                break;
            
            case 'partial':
                // Token por token en tiempo real
                process.stdout.write(msg.content);
                break;
            
            case 'complete':
                console.log(`\n✅ Frame #${msg.frame_number} completado en ${msg.processing_time_ms}ms`);
                console.log('📝 Resultado:', msg.content);
                break;
            
            case 'error':
                console.error(`❌ Error frame #${msg.frame_number}:`, msg.error);
                break;
        }
    }

    // Enviar configuración inicial
    setConfig(model, prompt) {
        this.ws.send(JSON.stringify({
            type: 'config',
            model: model,
            prompt: prompt
        }));
    }

    // Enviar frame (base64)
    sendFrame(imageBase64) {
        this.frameCount++;
        this.ws.send(JSON.stringify({
            image_base64: imageBase64,
            prompt: 'Describe esta imagen',
            max_tokens: 512
        }));
    }

    // Enviar frame binario (más eficiente)
    sendFrameBinary(imageBytes) {
        this.frameCount++;
        this.ws.send(imageBytes); // Binary message
    }

    disconnect() {
        if (this.ws) {
            this.ws.close();
        }
    }
}

// ===== USO =====

const vision = new VisionWebSocket(
    'http://localhost:9000',
    'tu-token-aqui'
);

// Conectar
await vision.connect();

// Configurar
vision.setConfig('gemma4:e4b', 'Describe lo que ves con detalle');

// Enviar frames continuamente
const video = document.getElementById('video');
const canvas = document.createElement('canvas');
const ctx = canvas.getContext('2d');

setInterval(() => {
    canvas.width = video.videoWidth;
    canvas.height = video.videoHeight;
    ctx.drawImage(video, 0, 0);
    
    // Convertir a base64 y enviar
    const base64 = canvas.toDataURL('image/jpeg', 0.7).split(',')[1];
    vision.sendFrame(base64);
}, 3000); // Cada 3 segundos
```

### Ejemplo: Python (WebSocket)

```python
import asyncio
import websockets
import json
import base64
import cv2
import time

class VisionStreamClient:
    def __init__(self, api_url, token):
        self.api_url = api_url.replace('http://', 'ws://')
        self.token = token
        self.ws = None
    
    async def connect(self):
        """Conectar al WebSocket"""
        headers = {
            'Authorization': f'Bearer {self.token}',
            'Content-Type': 'application/json'
        }
        
        self.ws = await websockets.connect(
            f'{self.api_url}/v1/vision/stream/ws',
            extra_headers=headers
        )
        
        print('✅ WebSocket conectado')
        
        # Esperar mensaje ready
        msg = await self.ws.recv()
        print('🟢', json.loads(msg))
    
    async def set_config(self, model=None, prompt=None):
        """Enviar configuración"""
        config = {
            'type': 'config',
        }
        if model:
            config['model'] = model
        if prompt:
            config['prompt'] = prompt
        
        await self.ws.send(json.dumps(config))
        print('⚙️ Configuración enviada')
    
    async def send_frame(self, image_base64, prompt=None):
        """Enviar frame para análisis"""
        msg = {
            'image_base64': image_base64,
            'max_tokens': 512
        }
        if prompt:
            msg['prompt'] = prompt
        
        await self.ws.send(json.dumps(msg))
    
    async def listen(self):
        """Escuchar respuestas del servidor"""
        async for message in self.ws:
            data = json.loads(message)
            self.handle_message(data)
    
    def handle_message(self, msg):
        """Procesar mensaje del servidor"""
        msg_type = msg.get('type')
        
        if msg_type == 'config_ack':
            print(f'⚙️ Config: {msg["model"]}')
        
        elif msg_type == 'processing':
            print(f'⏳ Procesando frame #{msg["frame_number"]}...')
        
        elif msg_type == 'partial':
            # Streaming token by token
            print(msg['content'], end='', flush=True)
        
        elif msg_type == 'complete':
            print(f'\n✅ Frame #{msg["frame_number"]} en {msg["processing_time_ms"]}ms')
            print(f'📝 Resultado: {msg["content"]}\n')
        
        elif msg_type == 'error':
            print(f'❌ Error: {msg["error"]}')
    
    async def continuous_analysis(self, camera_index=0, interval=3):
        """Análisis continuo desde cámara"""
        cap = cv2.VideoCapture(camera_index)
        
        if not cap.isOpened():
            raise Exception("No se pudo abrir la cámara")
        
        print('🎥 Iniciando análisis continuo...')
        
        while True:
            ret, frame = cap.read()
            if not ret:
                break
            
            # Codificar a JPEG
            _, buffer = cv2.imencode('.jpg', frame, [cv2.IMWRITE_JPEG_QUALITY, 75])
            image_base64 = base64.b64encode(buffer).decode('utf-8')
            
            # Enviar frame
            await self.send_frame(image_base64, 'Describe lo que ves')
            
            # Esperar intervalo
            await asyncio.sleep(interval)
        
        cap.release()

# ===== USO =====

async def main():
    client = VisionStreamClient('http://localhost:9000', 'tu-token-aqui')
    
    try:
        # Conectar
        await client.connect()
        
        # Configurar
        await client.set_config(
            model='gemma4:e4b',
            prompt='Describe detalladamente la escena'
        )
        
        # Iniciar análisis continuo
        await asyncio.gather(
            client.continuous_analysis(camera_index=0, interval=5),
            client.listen()
        )
        
    except KeyboardInterrupt:
        print('\n⏹️ Deteniendo...')
    finally:
        if client.ws:
            await client.ws.close()

asyncio.run(main())
```

### Ejemplo: Node.js (WebSocket)

```javascript
import WebSocket from 'ws';
import fs from 'fs';

class VisionWSClient {
    constructor(apiUrl, token) {
        this.url = apiUrl.replace('http://', 'ws://');
        this.token = token;
        this.ws = null;
    }

    connect() {
        return new Promise((resolve, reject) => {
            this.ws = new WebSocket(`${this.url}/v1/vision/stream/ws`, {
                headers: {
                    'Authorization': `Bearer ${this.token}`
                }
            });

            this.ws.on('open', () => {
                console.log('✅ WebSocket conectado');
                resolve();
            });

            this.ws.on('message', (data) => {
                const msg = JSON.parse(data.toString());
                this.handleMessage(msg);
            });

            this.ws.on('error', (error) => {
                console.error('❌ Error:', error);
                reject(error);
            });

            this.ws.on('close', () => {
                console.log('🔌 Desconectado');
            });
        });
    }

    handleMessage(msg) {
        switch (msg.type) {
            case 'ready':
                console.log('🟢 Servidor listo');
                break;
            case 'config_ack':
                console.log('⚙️ Config:', msg.model);
                break;
            case 'partial':
                process.stdout.write(msg.content);
                break;
            case 'complete':
                console.log(`\n✅ Frame #${msg.frame_number} en ${msg.processing_time_ms}ms`);
                break;
            case 'error':
                console.error(`❌ Error: ${msg.error}`);
                break;
        }
    }

    sendConfig(model, prompt) {
        this.ws.send(JSON.stringify({
            type: 'config',
            model,
            prompt
        }));
    }

    sendFrame(imageBase64) {
        this.ws.send(JSON.stringify({
            image_base64: imageBase64,
            max_tokens: 512
        }));
    }

    close() {
        if (this.ws) {
            this.ws.close();
        }
    }
}

// Uso
const client = new VisionWSClient('http://localhost:9000', 'tu-token');

await client.connect();
client.sendConfig('gemma4:e4b', 'Describe la escena');

// Enviar imagen
const imageBuffer = fs.readFileSync('foto.jpg');
const base64 = imageBuffer.toString('base64');
client.sendFrame(base64);

// Mantener conexión abierta
setTimeout(() => client.close(), 60000);
```

---

## 📦 2. Batch - Procesamiento por Lotes

### ¿Qué es Batch?

El endpoint batch permite enviar **múltiples imágenes en una sola request** y recibir análisis de todas ellas. Puede procesarlas en **paralelo** (más rápido) o **secuencial** (menos recursos).

### Request

```bash
POST /v1/vision/analyze/batch
```

**Body JSON:**
```json
{
  "images": [
    {
      "image_base64": "<BASE64_IMAGE_1>",
      "id": "frame-001",
      "prompt": "¿Qué hay en esta imagen?"
    },
    {
      "image_base64": "<BASE64_IMAGE_2>",
      "id": "frame-002",
      "prompt": "Describe los objetos"
    },
    {
      "image_base64": "<BASE64_IMAGE_3>",
      "id": "frame-003"
    }
  ],
  "model": "gemma4:e4b",
  "prompt": "Describe esta imagen",
  "max_tokens": 512,
  "temperature": 0.7,
  "parallel": true
}
```

**Parámetros:**

| Campo | Tipo | Requerido | Descripción |
|-------|------|-----------|-------------|
| `images` | array | ✅ Sí | Array de imágenes (máx 20) |
| `images[].image_base64` | string | ✅ Sí | Imagen en base64 |
| `images[].id` | string | ❌ No | ID personalizado para tracking |
| `images[].prompt` | string | ❌ No | Prompt específico para esta imagen |
| `model` | string | ❌ No | Modelo para todas las imágenes |
| `prompt` | string | ❌ No | Prompt default (si imagen no tiene) |
| `max_tokens` | int | ❌ No | Máximo tokens por imagen |
| `temperature` | float | ❌ No | Temperatura para todas |
| `parallel` | bool | ❌ No | Procesar en paralelo (default: false) |

### Response

```json
{
  "id": "uuid-batch-123",
  "model": "gemma4:e4b",
  "total_images": 3,
  "successful": 2,
  "failed": 1,
  "results": [
    {
      "id": "frame-001",
      "index": 0,
      "success": true,
      "content": "La imagen muestra un paisaje montañosco...",
      "error": null,
      "processing_time_ms": 2345
    },
    {
      "id": "frame-002",
      "index": 1,
      "success": true,
      "content": "Se observan varios objetos sobre una mesa...",
      "error": null,
      "processing_time_ms": 2156
    },
    {
      "id": "frame-003",
      "index": 2,
      "success": false,
      "content": null,
      "error": "Invalid image format",
      "processing_time_ms": 45
    }
  ],
  "total_processing_time_ms": 4546
}
```

### Ejemplo curl

```bash
# Codificar imágenes
IMG1=$(base64 -w 0 foto1.jpg)
IMG2=$(base64 -w 0 foto2.jpg)
IMG3=$(base64 -w 0 foto3.jpg)

# Enviar batch
curl http://localhost:9000/v1/vision/analyze/batch \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_TOKEN" \
  -d "{
    \"images\": [
      {\"image_base64\": \"$IMG1\", \"id\": \"foto-1\", \"prompt\": \"¿Qué ves?\"},
      {\"image_base64\": \"$IMG2\", \"id\": \"foto-2\"},
      {\"image_base64\": \"$IMG3\", \"id\": \"foto-3\"}
    ],
    \"model\": \"gemma4:e4b\",
    \"parallel\": true
  }"
```

### Ejemplo Python

```python
import base64
import requests
import time

API_TOKEN = "tu-token-aqui"

def analyze_batch(image_paths, parallel=True):
    """Analizar múltiples imágenes en batch"""
    
    # Preparar imágenes
    images = []
    for i, path in enumerate(image_paths):
        with open(path, "rb") as f:
            img_b64 = base64.b64encode(f.read()).decode('utf-8')
        
        images.append({
            "image_base64": img_b64,
            "id": f"image-{i:03d}",
            "prompt": "Describe esta imagen en detalle"
        })
    
    # Enviar batch
    start_time = time.time()
    
    response = requests.post(
        "http://localhost:9000/v1/vision/analyze/batch",
        headers={
            "Authorization": f"Bearer {API_TOKEN}",
            "Content-Type": "application/json"
        },
        json={
            "images": images,
            "model": "gemma4:e4b",
            "max_tokens": 512,
            "parallel": parallel
        }
    )
    
    elapsed = time.time() - start_time
    data = response.json()
    
    # Mostrar resultados
    print(f"\n📦 Batch completado en {elapsed:.2f}s")
    print(f"✅ Exitosos: {data['successful']}/{data['total_images']}")
    print(f"❌ Fallidos: {data['failed']}")
    print(f"⚡ Tiempo total API: {data['total_processing_time_ms']}ms\n")
    
    for result in data['results']:
        status = "✅" if result['success'] else "❌"
        print(f"{status} [{result['id']}] ({result['processing_time_ms']}ms)")
        
        if result['success']:
            print(f"   {result['content'][:100]}...")
        else:
            print(f"   Error: {result['error']}")
        print()
    
    return data

# Uso
image_paths = [
    "foto1.jpg",
    "foto2.jpg",
    "foto3.jpg"
]

# Procesar en paralelo (más rápido)
analyze_batch(image_paths, parallel=True)

# Procesar secuencial (menos recursos)
analyze_batch(image_paths, parallel=False)
```

### Ejemplo Node.js

```javascript
import fs from 'fs';
import fetch from 'node-fetch';

const API_TOKEN = 'tu-token-aqui';

async function analyzeBatch(imagePaths, parallel = true) {
  // Preparar imágenes
  const images = imagePaths.map((path, i) => {
    const buffer = fs.readFileSync(path);
    return {
      image_base64: buffer.toString('base64'),
      id: `image-${String(i).padStart(3, '0')}`,
      prompt: 'Describe esta imagen'
    };
  });

  // Enviar batch
  const response = await fetch('http://localhost:9000/v1/vision/analyze/batch', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${API_TOKEN}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      images,
      model: 'gemma4:e4b',
      max_tokens: 512,
      parallel
    })
  });

  const data = await response.json();
  
  console.log(`\n📦 Batch: ${data.successful}/${data.total_images} exitosos`);
  console.log(`⚡ Tiempo: ${data.total_processing_time_ms}ms\n`);
  
  for (const result of data.results) {
    console.log(`${result.success ? '✅' : '❌'} [${result.id}]`);
    if (result.success) {
      console.log(`   ${result.content.substring(0, 100)}...`);
    } else {
      console.log(`   Error: ${result.error}`);
    }
  }
  
  return data;
}

// Uso
const images = ['foto1.jpg', 'foto2.jpg', 'foto3.jpg'];
await analyzeBatch(images, true);
```

---

## 🆚 Comparación: ¿Cuál Usar?

| Característica | HTTP Simple | Batch | WebSocket |
|----------------|-------------|-------|-----------|
| **Imágenes** | 1 | Múltiples (hasta 20) | Ilimitadas (streaming) |
| **Conexión** | Request/Response | Request/Response | Persistente |
| **Velocidad** | Medio | Rápido (paralelo) | Muy rápido |
| **Uso** | Análisis puntual | Procesamiento por lotes | Video en tiempo real |
| **Overhead** | Bajo | Bajo | Muy bajo |
| **Complejidad** | Baja | Baja | Media |

### Cuándo Usar Cada Uno:

**HTTP Simple (`/vision/analyze`):**
- ✅ Analizar una imagen específica
- ✅ Integración sencilla
- ✅ Apps que no necesitan tiempo real

**Batch (`/vision/analyze/batch`):**
- ✅ Procesar múltiples fotos de una vez
- ✅ Comparar imágenes
- ✅ Reducir número de requests

**WebSocket (`/vision/stream/ws`):**
- ✅ **Video en tiempo real** desde cámara
- ✅ Análisis continuo sin reconectar
- ✅ Máxima eficiencia y menor latencia
- ✅ Apps de vigilancia, accesibilidad, etc.

---

## 🔐 Autenticación WebSocket

WebSocket usa el mismo token de autenticación:

```javascript
// JavaScript
const ws = new WebSocket('ws://localhost:9000/v1/vision/stream/ws', {
    headers: {
        'Authorization': 'Bearer TU-TOKEN'
    }
});
```

```python
# Python
async with websockets.connect(
    'ws://localhost:9000/v1/vision/stream/ws',
    extra_headers={'Authorization': 'Bearer TU-TOKEN'}
) as ws:
    # ...
```

---

## ⚡ Rate Limiting

El rate limiting aplica por request, no por frame en WebSocket:

```bash
# En .env
RATE_LIMIT_REQUESTS=100
RATE_LIMIT_SECONDS=60
```

Para WebSocket continuo:
- 1 frame cada 3s = 20 requests/min ✅
- 1 frame cada 5s = 12 requests/min ✅
- 1 frame cada 1s = 60 requests/min ⚠️ (ajustar límite)

---

## 📊 Ejemplo Completo: App de Vigilancia

```python
import asyncio
import websockets
import cv2
import base64
import json
from datetime import datetime

class SurveillanceSystem:
    def __init__(self, api_url, token, camera_urls):
        self.api_url = api_url.replace('http://', 'ws://')
        self.token = token
        self.camera_urls = camera_urls
        self.alerts = []
    
    async def monitor_camera(self, camera_url, camera_id):
        """Monitorear una cámara continuamente"""
        cap = cv2.VideoCapture(camera_url)
        
        if not cap.isOpened():
            print(f"❌ No se pudo abrir cámara {camera_id}")
            return
        
        ws = await websockets.connect(
            f'{self.api_url}/v1/vision/stream/ws',
            extra_headers={'Authorization': f'Bearer {self.token}'}
        )
        
        # Configurar
        await ws.send(json.dumps({
            'type': 'config',
            'model': 'gemma4:e4b',
            'prompt': 'Analiza la escena. ¿Hay algo sospechoso o inusual? Menciona personas, objetos y acciones.'
        }))
        
        # Esperar config_ack
        await ws.recv()
        
        print(f'🎥 Monitoreando cámara {camera_id}...')
        
        frame_count = 0
        while True:
            ret, frame = cap.read()
            if not ret:
                break
            
            # Enviar cada 10 segundos (300 frames a 30fps)
            if frame_count % 300 == 0:
                _, buffer = cv2.imencode('.jpg', frame, [cv2.IMWRITE_JPEG_QUALITY, 70])
                image_b64 = base64.b64encode(buffer).decode('utf-8')
                
                await ws.send(json.dumps({
                    'image_base64': image_b64,
                    'max_tokens': 256
                }))
                
                # Recibir análisis
                while True:
                    msg = json.loads(await ws.recv())
                    
                    if msg['type'] == 'complete':
                        timestamp = datetime.now().strftime('%H:%M:%S')
                        print(f"\n[{timestamp}] Cámara {camera_id}:")
                        print(f"📝 {msg['content']}\n")
                        
                        # Detectar alertas
                        suspicious_words = ['sospechoso', 'inusual', 'extraño', 'alerta']
                        if any(word in msg['content'].lower() for word in suspicious_words):
                            self.alerts.append({
                                'camera': camera_id,
                                'time': timestamp,
                                'analysis': msg['content']
                            })
                            print("🚨 ALERTA DETECTADA!")
                        
                        break
                    elif msg['type'] == 'error':
                        print(f"❌ Error: {msg['error']}")
                        break
            
            frame_count += 1
            await asyncio.sleep(0.033)  # 30fps
        
        cap.release()
        await ws.close()
    
    async def start(self):
        """Iniciar monitoreo de todas las cámaras"""
        tasks = [
            self.monitor_camera(url, i)
            for i, url in enumerate(self.camera_urls)
        ]
        await asyncio.gather(*tasks)

# Uso
surveillance = SurveillanceSystem(
    api_url='http://localhost:9000',
    token='tu-token',
    camera_urls=[
        'rtsp://camera1.local/stream',
        'rtsp://camera2.local/stream',
        'rtsp://camera3.local/stream'
    ]
)

asyncio.run(surveillance.start())
```

---

## 🔗 Recursos

- [Ejemplo Web Completo](examples/vision-realtime.html)
- [Documentación API](docs/API.md)
- [Guía de Consumo](docs/API-CONSUME.md)
- [Gemma 4 Multimodal](docs/GEMMA4-MULTIMODAL.md)
