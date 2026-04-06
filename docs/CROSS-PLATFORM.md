# Cross-Platform Guide

This project supports **Linux**, **macOS**, and **Windows**. Each platform has its own scripts and build configuration.

## Quick Start by Platform

### Linux 🐧

```bash
# Build llama-server (Vulkan GPU)
./scripts/build-llama-server.sh

# Build Rust API
./scripts/build-api.sh

# Start server (reads .env)
./scripts/start-llama-server.sh

# Health check
curl http://localhost:8080/health
```

**GPU Backend:** Vulkan (`GGML_VULKAN=ON`)
**Variables:** `VK_ICD_FILENAMES`, `MESA_VK_WSI`

---

### macOS 🍎

```bash
# Build llama-server (Metal GPU)
./scripts/macos/build-llama-server.sh

# Build Rust API
./scripts/macos/build-api.sh

# Start server (reads .env)
./scripts/macos/start-llama-server.sh

# Health check
curl http://localhost:8080/health
```

**GPU Backend:** Metal (`GGML_METAL=ON`)
**No extra GPU variables needed** — Metal is auto-detected on macOS.

#### Requirements
- **Xcode Command Line Tools:** `xcode-select --install`
- **CMake:** `brew install cmake`
- **Rust:** `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

---

### Windows 🪟

```cmd
REM Build llama-server (Vulkan GPU)
scripts\windows\build-llama-server.bat

REM Build Rust API
scripts\windows\build-api.bat

REM Start server (reads .env)
scripts\windows\start-llama-server.bat

REM Health check
curl http://localhost:8080/health
```

**GPU Backend:** Vulkan (`GGML_VULKAN=ON`)
**Variables:** `VK_ICD_FILENAMES` (set in `.env` or defaults to Vulkan SDK paths)

#### Requirements
- **Visual Studio Build Tools** with C++ workload ([download](https://visualstudio.microsoft.com/downloads/))
- **CMake** ([download](https://cmake.org/download/) or `winget install Kitware.CMake`)
- **Git** ([download](https://git-scm.com/download/win) or `winget install Git.Git`)
- **Rust** ([rustup](https://rustup.rs/) or `winget install Rustlang.Rustup`)
- **Vulkan SDK** (for `glslc` shader compiler) — [download](https://vulkan.lunarg.com/sdk/home#windows)

---

## Build Directories

| Platform | llama-server build dir | GPU Flag |
|----------|----------------------|----------|
| Linux | `llama-server/build-native/` | `-DGGML_VULKAN=ON` |
| macOS | `llama-server/build-macos/` | `-DGGML_METAL=ON` |
| Windows | `llama-server/build-windows/` | `-DGGML_VULKAN=ON` |

---

## .env Configuration

The `.env` file at the project root is shared across all platforms. Most variables work everywhere:

```bash
PORT=8080
HOST=0.0.0.0
MODEL_NAME=Qwen3.5-9B.Q8_0.gguf
CONTEXT_SIZE=4096
GPU_LAYERS=35
LLAMA_ARG_CACHE_TYPE_K=q4_0
LLAMA_ARG_CACHE_TYPE_V=q4_0
```

### Platform-Specific Variables

| Variable | Linux | macOS | Windows | Notes |
|----------|-------|-------|---------|-------|
| `VK_ICD_FILENAMES` | ✅ Required | ❌ Not used | ✅ Optional | Vulkan ICD paths |
| `MESA_VK_WSI` | ✅ Required | ❌ Not used | ❌ Not used | Mesa Vulkan flag |

---

## Troubleshooting

### macOS: Metal not working
- Ensure you're on macOS 12.3+ (Metal FX support)
- Apple Silicon (M1/M2/M3/M4): Metal works out of the box
- Intel Mac: May need additional GPU driver setup

### Windows: Vulkan not detected
- Install Vulkan SDK from LunarG
- Ensure `VK_ICD_FILENAMES` points to correct `.json` files
- Typical path: `C:\VulkanSDK\etc\vulkan\icd.d\radeon_icd.x86_64.json`

### Linux: Vulkan not detected
- Install `mesa-vulkan-drivers` (Fedora) or `mesa-vulkan-drivers` (Ubuntu)
- Verify with: `vulkaninfo --summary`

---

## File Structure

```
scripts/
├── start-llama-server.sh     ← Linux: start server
├── build-llama-server.sh     ← Linux: build llama.cpp
├── build-api.sh              ← Linux: build Rust API
├── download-model.sh         ← Linux/macOS: download model
├── health-check.sh           ← Linux: check services
├── macos/
│   ├── start-llama-server.sh ← macOS: start server (Metal)
│   ├── build-llama-server.sh ← macOS: build llama.cpp (Metal)
│   └── build-api.sh          ← macOS: build Rust API
└── windows/
    ├── start-llama-server.bat ← Windows: start server (Vulkan)
    ├── build-llama-server.bat ← Windows: build llama.cpp (Vulkan)
    ├── build-api.bat          ← Windows: build Rust API
    └── download-model.bat     ← Windows: download model
```
