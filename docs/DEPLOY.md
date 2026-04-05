# 🐳 Guía de Deploy

## Requisitos

- Docker + Docker Compose
- 6+ GB RAM (modelos ≤7.5B)
- 22+ GB RAM (modelos ≤30B)
- Espacio en disco para modelos (4-20 GB)

## Configuración

### 1. Variables de Entorno

```bash
cp .env.example .env
nano .env
```

#### Variables Clave

| Variable | Descripción | Ejemplo |
|----------|-------------|---------|
| `MODEL_NAME` | Nombre del archivo GGUF en `models/` | `google_gemma-4-E4B-it-Q4_K_M.gguf` |
| `CONTEXT_SIZE` | Tokens de contexto | `8192` |
| `CACHE_TYPE_K` | Tipo de cuantización cache K | `turbo3`, `q4_0`, `q8_0`, `f16` |
| `CACHE_TYPE_V` | Tipo de cuantización cache V | `turbo3`, `q4_0`, `q8_0`, `f16` |
| `API_TOKEN` | Token de autenticación | `mi-token-seguro` |
| `API_PORT` | Puerto externo | `9000` |

### 2. Descargar Modelo

```bash
./scripts/download-model.sh bartowski/google_gemma-4-E4B-it-GGUF \
    google_gemma-4-E4B-it-Q4_K_M.gguf
```

### 3. Configurar .env

```bash
MODEL_NAME=google_gemma-4-E4B-it-Q4_K_M.gguf
CACHE_TYPE_K=turbo3
CACHE_TYPE_V=turbo3
API_TOKEN=mi-token-seguro
API_PORT=9000
```

## Comandos

### Iniciar

```bash
docker compose up -d --build
```

### Ver logs

```bash
# Logs en tiempo real
docker compose logs -f

# Solo llama-server
docker compose logs -f llama-server

# Solo API
docker compose logs -f api
```

### Verificar

```bash
./scripts/health-check.sh
```

### Detener

```bash
docker compose down
```

### Reiniciar con otro modelo

```bash
# Cambiar en .env
MODEL_NAME=google_gemma-4-31B-it-Q4_K_M.gguf

# Reconstruir y reiniciar
docker compose up -d --build
```

## Troubleshooting

### **El modelo no carga**

```bash
# Verificar que el archivo existe
ls -lh models/

# Ver logs de llama-server
docker compose logs llama-server
```

### **Puerto en uso**

```bash
# Cambiar puerto en .env
API_PORT=9001

# O matar proceso existente
lsof -i :9000
kill <PID>
```

### **Falta de memoria**

```bash
# Reducir contexto
CONTEXT_SIZE=4096

# O usar modelo más pequeño
MODEL_NAME=google_gemma-4-E4B-it-Q4_K_M.gguf
```

### **API no responde**

```bash
# Verificar que llama-server está healthy
docker compose ps

# Si llama-server no está healthy, la API no iniciará
docker compose logs llama-server
```
