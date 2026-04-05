# 🚀 Native Deployment Guide (Binarios + systemd)

Guía para desplegar el LLM API Server como **binarios nativos** con servicios systemd, sin Docker.

## ¿Por qué nativo?

| Ventaja | Descripción |
|---------|-------------|
| **Máximo rendimiento** | Sin overhead de virtualización |
| **GPU completa** | Acceso directo a Vulkan sin problemas de contenedores |
| **Menos memoria** | Sin Docker daemon (~100-200MB ahorrados) |
| **Boot instantáneo** | Servicios systemd inician en <2s |
| **Debugging fácil** | Logs directos con `journalctl` |
| **Auto-start** | systemd maneja reinicios automáticos |

## 📋 Requisitos

### Hardware
- **CPU**: x86_64 con soporte AVX2 (recomendado)
- **RAM**: 6GB+ (modelos 7B), 22GB+ (modelos 30B)
- **GPU**: Compatible con Vulkan (Intel, AMD, NVIDIA)
- **Disco**: 10GB+ para binarios + modelos

### Software
- **OS**: Linux con systemd (Ubuntu 20.04+, Debian 11+, Fedora 35+)
- **Kernel**: 5.10+ (recomendado para Vulkan)
- **Root**: Acceso sudo para instalación

## 🏗️ Arquitectura

```
┌─────────────────────────────────────────────────────────┐
│                    Sistema Host                         │
│                                                         │
│  ┌──────────────────┐         ┌──────────────────┐    │
│  │  llama-server    │────────▶│  GPU (Vulkan)    │    │
│  │  (systemd)       │         │  /dev/dri        │    │
│  │  Port 8080       │         └──────────────────┘    │
│  └────────┬─────────┘                                  │
│           │ HTTP                                       │
│           ▼                                            │
│  ┌──────────────────┐         ┌──────────────────┐    │
│  │  Rust API        │────────▶│  Clientes        │    │
│  │  (systemd)       │         │  Port 9000       │    │
│  │  Port 3000       │         └──────────────────┘    │
│  └──────────────────┘                                  │
│                                                         │
│  Config: /etc/llm-api/.env                            │
│  Binaries: /opt/llm-api/bin/                          │
│  Logs: journalctl -u <service>                        │
└─────────────────────────────────────────────────────────┘
```

## 🔧 Instalación Automática

### 1. Clonar repositorio

```bash
git clone <tu-repo>
cd api-rust-model-local
```

### 2. Ejecutar instalador

```bash
sudo ./scripts/install-native.sh
```

Esto hará:
- ✅ Instala dependencias (Rust, Vulkan libs, cmake)
- ✅ Crea usuario del sistema `llm-api`
- ✅ Compila llama-server con Vulkan GPU
- ✅ Compila Rust API con optimizaciones
- ✅ Instala binarios en `/opt/llm-api/bin/`
- ✅ Configura servicios systemd
- ✅ Habilita auto-start en boot

### 3. Configurar entorno

```bash
sudo nano /etc/llm-api/.env
```

**Variables críticas a modificar:**

```bash
# Ruta a tu modelo GGUF
MODEL_PATH=/home/tu-usuario/Documentos/llm/api rust model local/models/google_gemma-4-E4B-it-Q4_K_M.gguf

# Token de seguridad (¡CAMBIA ESTO!)
API_TOKEN=tu-token-seguro-generado-aqui

# Capas GPU (ajusta según tu hardware)
GPU_LAYERS=35

# Tamaño de contexto
CONTEXT_SIZE=8192
```

### 4. Descargar modelo

```bash
# Desde el directorio del proyecto
./scripts/download-model.sh bartowski/google_gemma-4-E4B-it-GGUF \
    google_gemma-4-E4B-it-Q4_K_M.gguf
```

### 5. Iniciar servicios

```bash
# Iniciar llama-server (inference engine)
sudo systemctl start llama-server

# Ver logs en tiempo real
sudo journalctl -u llama-server -f

# Iniciar API REST
sudo systemctl start llm-api

# Ver logs API
sudo journalctl -u llm-api -f
```

## 📊 Gestión de Servicios

### Comandos básicos

```bash
# Ver estado
sudo systemctl status llama-server llm-api

# Detener servicios
sudo systemctl stop llama-server llm-api
# Nota: llm-api se detiene automáticamente cuando llama-server para

# Reiniciar servicios
sudo systemctl restart llama-server
sudo systemctl restart llm-api

# Ver logs completos
sudo journalctl -u llama-server -n 100
sudo journalctl -u llm-api -n 100

# Seguir logs en tiempo real
sudo journalctl -u llama-server -f
```

### Logs avanzados

```bash
# Logs de las últimas 2 horas
sudo journalctl -u llama-server --since "2 hours ago"

# Logs con formato JSON
sudo journalctl -u llama-server -o json

# Exportar logs a archivo
sudo journalctl -u llama-server > llama-server.log
```

## 🔍 Verificación

### 1. Verificar GPU

```bash
# Verificar que Vulkan detecta tu GPU
vulkaninfo --summary | grep -i device

# Verificar dispositivo DRM
ls -la /dev/dri/
```

### 2. Verificar llama-server

```bash
# Ver logs de inicio
sudo journalctl -u llama-server -n 50

# Deberías ver algo como:
# "llama_init_from_params: using Vulkan GPU"
# "offloading 35 layers to GPU"
```

### 3. Verificar API

```bash
# Health check
curl http://localhost:9000/health

# Response esperada:
# {"status":"ok","services":{"api":"running","llama_server":"healthy"}}

# Listar modelos
curl -H "Authorization: Bearer tu-token" http://localhost:9000/v1/models
```

### 4. Test de inferencia

```bash
curl -X POST http://localhost:9000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer tu-token" \
  -d '{
    "model": "gemma-4",
    "messages": [{"role": "user", "content": "Hola, ¿cómo estás?"}],
    "max_tokens": 100
  }'
```

## ⚙️ Configuración Avanzada

### Ajustar capas GPU

Edita `/etc/llm-api/.env`:

```bash
# Para GPU con 8GB VRAM
GPU_LAYERS=25

# Para GPU con 12GB VRAM
GPU_LAYERS=45

# Para GPU con 24GB VRAM (todos los layers)
GPU_LAYERS=999
```

### Cambiar puertos

```bash
# En /etc/llm-api/.env
PORT=3000              # Puerto interno de la API
API_PORT=9000          # Puerto externo (mapeado por nginx/reverse proxy)
```

### Límites de recursos

Los servicios systemd ya incluyen límites:

```ini
# llama-server.service
MemoryMax=30G          # Ajustar según modelo

# llm-api.service
MemoryMax=512M         # Suficiente para la API
```

Para modificar:
```bash
sudo systemctl edit llama-server
# Agregar:
# [Service]
# MemoryMax=40G
```

### Variables Vulkan

Si tienes problemas con Vulkan, las siguientes variables están configuradas en el service file:

```bash
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/intel_icd.x86_64.json:/usr/share/vulkan/icd.d/radeon_icd.x86_64.json
MESA_VK_WSI=1
GGML_VK_VISIBLE_DEVICES=0
```

Para NVIDIA, cambia a:
```bash
sudo systemctl edit llama-server
# Agregar:
# [Service]
# Environment=GGML_VK_VISIBLE_DEVICES=0
```

## 🔄 Actualización

### Actualizar código

```bash
# Pull nuevos cambios
git pull

# Reconstruir binarios
./scripts/build-api.sh
./scripts/build-llama-server.sh /opt/llm-api/build

# Copiar binarios
sudo cp api/target/release/rust_llm_api /opt/llm-api/bin/
sudo cp /opt/llm-api/build/llama.cpp/build/bin/llama-server /opt/llm-api/bin/

# Reiniciar servicios
sudo systemctl restart llama-server llm-api
```

### Actualizar dependencias

```bash
sudo apt update
sudo apt upgrade

# Reinstalar si es necesario
sudo ./scripts/install-native.sh
```

## 🐛 Troubleshooting

### llama-server no inicia

```bash
# Ver logs detallados
sudo journalctl -u llama-server -n 200 --no-pager

# Problemas comunes:
# 1. MODEL_PATH incorrecto → Verificar ruta en .env
# 2. Modelo no existe → ls -lh $MODEL_PATH
# 3. Vulkan no detectado → vulkaninfo --summary
```

### GPU no detectada

```bash
# Verificar Vulkan
vulkaninfo --summary

# Verificar permisos /dev/dri
ls -la /dev/dri/
# Deberías tener acceso de lectura/escritura

# Agregar usuario al grupo render
sudo usermod -aG render llm-api
sudo usermod -aG video llm-api

# Reiniciar servicio
sudo systemctl restart llama-server
```

### API no conecta con llama-server

```bash
# Verificar que llama-server está corriendo
sudo systemctl status llama-server

# Verificar puerto
sudo ss -tlnp | grep 8080

# Test directo
curl http://localhost:8080/health

# Ver logs de API
sudo journalctl -u llm-api -n 50
```

### Memoria insuficiente

```bash
# Ver uso de memoria
sudo systemctl status llama-server

# Reducir contexto en /etc/llm-api/.env
CONTEXT_SIZE=4096

# Reducir capas GPU (usa más CPU)
GPU_LAYERS=20

# Reiniciar
sudo systemctl restart llama-server
```

### Service fails to start

```bash
# Verificar sintaxis de service files
systemd-analyze verify /etc/systemd/system/llama-server.service

# Recargar systemd
sudo systemctl daemon-reload

# Ver logs completos
sudo journalctl -xe
```

## 📁 Estructura de Archivos

```
/opt/llm-api/
├── bin/
│   ├── llama-server          # Binario llama.cpp
│   └── rust_llm_api          # Binario API Rust
├── logs/                     # Logs de aplicación
├── models -> /path/to/models # Symlink a modelos
└── build/                    # Directorio de build (puedes eliminar)

/etc/llm-api/
└── .env                      # Configuración

/etc/systemd/system/
├── llama-server.service      # Service inference engine
└── llm-api.service           # Service API HTTP
```

## 🔐 Seguridad

El usuario `llm-api` está configurado con:
- ✅ Sin login shell (`/usr/sbin/nologin`)
- ✅ Sin home directory
- ✅ `NoNewPrivileges=true`
- ✅ `ProtectSystem=strict`
- ✅ `PrivateTmp=true`
- ✅ `ProtectHome=true`

### Firewall

```bash
# UFW (Ubuntu/Debian)
sudo ufw allow 9000/tcp

# firewalld (Fedora/RHEL)
sudo firewall-cmd --permanent --add-port=9000/tcp
sudo firewall-cmd --reload
```

### Reverse Proxy (Opcional)

Para producción, usa nginx como reverse proxy:

```nginx
server {
    listen 443 ssl;
    server_name api.tudominio.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://localhost:9000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## 📊 Monitoreo

### systemd metrics

```bash
# Uso de memoria
sudo systemctl show llama-server -p MemoryCurrent
sudo systemctl show llm-api -p MemoryCurrent

# CPU usage
sudo systemctl show llama-server -p CPUUsageNSec
```

### Logs estructurados

```bash
# Logs en JSON para análisis
sudo journalctl -u llama-server -o json | jq .

# Contar errores
sudo journalctl -u llama-server -p err --no-pager | wc -l
```

## 🆚 Docker vs Nativo

| Característica | Docker | Nativo |
|----------------|--------|--------|
| **GPU Access** | Configuración compleja | Directo |
| **Memory Overhead** | ~100-200MB | ~0MB |
| **Boot Time** | 10-30s | <2s |
| **Debugging** | docker logs | journalctl |
| **Updates** | Rebuild image | Recompile binary |
| **Isolation** | Alta | Media (systemd) |
| **Portability** | Alta | Baja |

## 📚 Recursos

- [systemd.service man page](https://www.freedesktop.org/software/systemd/man/systemd.service.html)
- [llama.cpp documentation](https://github.com/ggerganov/llama.cpp)
- [Vulkan SDK](https://vulkan.lunarg.com/)
- [Rust performance](https://github.com/johnthagen/min-sized-rust)

## 🤝 Soporte

Para issues específicos:
1. Revisar logs: `sudo journalctl -u llama-server -n 100`
2. Verificar GPU: `vulkaninfo --summary`
3. Testear API: `curl http://localhost:9000/health`
