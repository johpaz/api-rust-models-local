# Documentación de la API LLM: Arquitectura y Modelos Turbo Quant (GGUF)

Esta documentación describe el funcionamiento técnico de la API de inferencia implementada en Rust, optimizada para modelos cuantizados en formato GGUF (referidos internamente como "Turbo Quant" por su eficiencia).

## 🚀 Arquitectura del Sistema

La API está construida sobre un stack de alto rendimiento:
- **Framework HTTP:** [Axum](https://github.com/tokio-rs/axum) (basado en Tokio) para manejo asíncrono y escalable.
- **Motor de Inferencia:** [llama-cpp-2](https://github.com/ggerganov/llama.cpp) para ejecución nativa de modelos GGUF con soporte para instrucciones vectoriales (AVX, NEON).
- **Seguridad:** Middleware de autenticación Bearer y Rate Limiting (ventana deslizante).

## 🛠️ Funcionamiento con Modelos Turbo Quant (GGUF)

Los modelos "Turbo Quant" son archivos `.gguf` que permiten una inferencia rápida con bajo consumo de memoria RAM gracias a la cuantización (4-bit, 5-bit, etc.).

### 1. Carga y Gestión de Memoria
- **Pre-asignación de KV Cache:** Al iniciar la aplicación, se reserva el espacio necesario en memoria para el contexto (definido por `CONTEXT_SIZE`). Esto evita picos de RAM durante la generación.
- **Estado Compartido:** El modelo se carga una única vez en memoria y se gestiona a través de un `Arc<LlamaEngine>`, permitiendo que múltiples peticiones accedan a la estructura del modelo sin duplicar datos.
- **Validación de Archivos:** Se verifica la existencia y permisos del archivo `.gguf` antes de arrancar el servidor.

### 2. Motor de Inferencia ([llama.rs](file:///home/johnpaez/Documentos/llm/api%20rust%20model%20local/src/engine/llama.rs))
- **Cola de Concurrencia:** Implementamos un `Semaphore` que limita el número de inferencias simultáneas (configurable vía `MAX_CONCURRENCY`). Esto es crítico para mantener la estabilidad en hardware con recursos limitados (VPS).
- **Streaming SSE:** Los tokens se emiten en tiempo real mediante Server-Sent Events (SSE). Si el cliente se desconecta, la tarea de generación se cancela automáticamente para liberar recursos.

## 📡 Endpoints de la API

### `/v1/chat/completions` (POST)
Compatible con el estándar de OpenAI. Soporta streaming de tokens.
- **Validación de Payload:**
  - Límite de tamaño de prompt.
  - Validación de rango para `temperature` (0.0 - 2.0).
  - Límite de `max_tokens`.

### `/v1/models` (GET)
Lista el modelo actualmente cargado y su configuración básica.

### `/health` (GET)
Endpoint público para monitoreo y healthchecks de Docker.

## 🔐 Seguridad y Límites

- **Autenticación:** Todas las rutas `/v1/*` requieren el encabezado `Authorization: Bearer <token>`.
- **Rate Limiting:** Control de frecuencia de peticiones por segundo y ráfagas (burst) configurables desde el archivo `.env` o variables de entorno.
- **Apagado Gracioso (Graceful Shutdown):** El servidor espera a que las generaciones activas terminen antes de liberar la memoria del modelo y cerrarse, evitando corrupción de estado o fugas de memoria.

## 🐳 Despliegue con Docker
El [Dockerfile](file:///home/johnpaez/Documentos/llm/api%20rust%20model%20local/Dockerfile) utiliza compilación multi-etapa para generar una imagen mínima de producción con optimizaciones nativas de CPU (`target-cpu=native`), asegurando que los modelos Turbo Quant funcionen a su máxima velocidad posible.
