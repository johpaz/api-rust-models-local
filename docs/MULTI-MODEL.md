# 🔄 Multi-Model Selection

The API now supports selecting from multiple available models at request time, rather than being limited to a single configured model.

## How It Works

1. **Automatic Model Discovery**: The API scans the `models/` directory for all `.gguf` files
2. **List Available Models**: Use the `/v1/models` endpoint to see all available models
3. **Select Per Request**: Specify which model to use in each chat completion request

## API Usage

### List All Available Models

```bash
curl http://localhost:9000/v1/models \
  -H "Authorization: Bearer your-api-token"
```

**Response:**
```json
{
  "object": "list",
  "data": [
    {
      "id": "google_gemma-4-E4B-it-Q4_K_M.gguf",
      "object": "model",
      "created": 1712217600,
      "owned_by": "local",
      "name": "google_gemma-4-E4B-it-Q4_K_M.gguf",
      "path": "/path/to/models/google_gemma-4-E4B-it-Q4_K_M.gguf",
      "size_bytes": 5100000000
    },
    {
      "id": "Qwen3.5-9B.Q8_0.gguf",
      "object": "model",
      "created": 1712217600,
      "owned_by": "local",
      "name": "Qwen3.5-9B.Q8_0.gguf",
      "path": "/path/to/models/Qwen3.5-9B.Q8_0.gguf",
      "size_bytes": 9000000000
    }
  ]
}
```

### Use a Specific Model

Include the `model` field in your chat completion request:

```bash
curl http://localhost:9000/v1/chat/completions \
  -X POST \
  -H "Authorization: Bearer your-api-token" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "google_gemma-4-E4B-it-Q4_K_M.gguf",
    "messages": [
      {
        "role": "user",
        "content": "Hello, how are you?"
      }
    ],
    "temperature": 0.7,
    "max_tokens": 1024
  }'
```

### Fallback Behavior

- If the requested model doesn't exist, the API will log a warning and use the default model
- If no model is specified in the request, the currently loaded model in llama-server will be used

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MODELS_DIR` | `./models/` | Directory to scan for available `.gguf` models |

### Example `.env`

```bash
# Optional: Specify a custom models directory
MODELS_DIR=/home/user/my-models

# The rest of your configuration...
API_TOKEN=your-token
LLAMA_SERVER_URL=http://localhost:8080
```

## Adding New Models

Simply place `.gguf` files in the `models/` directory (or your custom `MODELS_DIR`):

```bash
# Download a new model
./scripts/download-model.sh bartowski/Qwen3.5-9B-GGUF Qwen3.5-9B.Q8_0.gguf

# The API will automatically detect it on next restart
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      API Server                             │
│                                                             │
│  1. Scans models/ directory at startup                     │
│  2. Returns all available models via /v1/models            │
│  3. Accepts model selection in chat requests               │
│  4. Passes model name to llama-server                      │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    models/ Directory                        │
│                                                             │
│  ├── google_gemma-4-E4B-it-Q4_K_M.gguf                     │
│  ├── Qwen3.5-9B.Q8_0.gguf                                  │
│  ├── nvidia_Nemotron-Cascade-2-30B-A3B-IQ2_M.gguf          │
│  └── ... (any other .gguf files)                           │
└─────────────────────────────────────────────────────────────┘
```

## Notes

- The API scans the models directory at startup. If you add new models, restart the API server.
- Model files must have the `.gguf` extension to be detected
- The llama-server must support the `model` parameter in the `/completion` endpoint for per-request model selection

## Audio Models (Whisper)

### How to Use Whisper Models

Whisper models are used for **audio transcription** (speech-to-text).

1. **Download a Whisper model:**
   ```bash
   # Small model (466MB)
   wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin -O models/whisper-small.gguf
   
   # Large v3 Turbo (1.6GB, recommended)
   wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin -O models/whisper-large-v3-turbo.gguf
   ```

2. **Transcribe audio with model selection:**
   ```bash
   curl http://localhost:9000/v1/audio/transcriptions \
     -H "Authorization: Bearer $API_TOKEN" \
     -F "file=@audio.mp3" \
     -F "model=whisper-large-v3-turbo.gguf" \
     -F "language=es"
   ```

### Available Whisper Models

| Model | Size | Quality | Speed | Use Case |
|-------|------|---------|-------|----------|
| `whisper-tiny.gguf` | 75 MB | Basic | Very Fast | Quick tests |
| `whisper-base.gguf` | 142 MB | Low | Fast | Simple transcriptions |
| `whisper-small.gguf` | 466 MB | Medium | Medium | Good balance |
| `whisper-medium.gguf` | 1.5 GB | High | Slow | Better accuracy |
| `whisper-large-v3.gguf` | 3 GB | Very High | Very Slow | Maximum accuracy |
| `whisper-large-v3-turbo.gguf` | 1.6 GB | High | Fast | **Recommended** |

## Text-to-Speech Models

TTS models convert text to audio. These require a separate TTS backend.

### Example TTS Request

```bash
curl http://localhost:9000/v1/audio/speech \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "model": "tts-1",
    "input": "Hola, bienvenido",
    "voice": "alloy",
    "response_format": "mp3"
  }' \
  --output audio.mp3
```

## Image Generation Models

Image generation requires a dedicated backend (FLUX, Stable Diffusion).

### Example Image Generation Request

```bash
curl http://localhost:9000/v1/images/generations \
  -H "Authorization: Bearer $API_TOKEN" \
  -d '{
    "model": "flux-schnell",
    "prompt": "A cat astronaut in space",
    "size": "1024x1024",
    "n": 1
  }'
```
