#!/bin/bash
# ═══════════════════════════════════════════════════════
# Build script for llama-server with Vulkan GPU support
# ═══════════════════════════════════════════════════════
# Clones llama.cpp and compiles with Vulkan GPU acceleration
#
# Usage:
#   ./scripts/build-llama-server.sh [install_dir]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Installation directory
INSTALL_DIR="${1:-$PROJECT_ROOT/llama-server/build-native}"
LLAMA_DIR="$INSTALL_DIR/llama.cpp"

echo "🔧 Building llama-server with Vulkan GPU support..."
echo "   Install directory: $INSTALL_DIR"
echo ""

# Check dependencies
echo "📋 Checking dependencies..."

if ! command -v cmake &> /dev/null; then
    echo "❌ cmake not found. Install with: sudo dnf install cmake"
    exit 1
fi

if ! command -v g++ &> /dev/null; then
    echo "❌ g++ not found. Install with: sudo dnf install gcc-c++"
    exit 1
fi

if ! command -v git &> /dev/null; then
    echo "❌ git not found. Install with: sudo dnf install git"
    exit 1
fi

if ! command -v glslc &> /dev/null; then
    echo "❌ glslc (Vulkan shader compiler) not found."
    echo "Install with: sudo dnf install glslc"
    exit 1
fi

echo "✅ Dependencies OK"
echo ""

# Create build directory
mkdir -p "$INSTALL_DIR"

# Clone llama.cpp if not exists
if [ ! -d "$LLAMA_DIR" ]; then
    echo "📦 Cloning llama.cpp..."
    # Use a known stable commit
    git clone --depth 1 --branch b5336 https://github.com/ggerganov/llama.cpp "$LLAMA_DIR"
    echo "✅ llama.cpp cloned"
else
    echo "✅ llama.cpp already exists at $LLAMA_DIR"
fi

cd "$LLAMA_DIR"

# Build with Vulkan GPU support
echo ""
echo "🔨 Compiling llama-server..."
echo "   Configuration:"
echo "   - GPU Backend: Vulkan"
echo "   - Build Type: Release"
echo "   - Server: ON"
echo "   - Tests: OFF"
echo "   - Examples: OFF"
echo ""

cmake -B build \
    -DCMAKE_BUILD_TYPE=Release \
    -DLLAMA_BUILD_TESTS=OFF \
    -DLLAMA_BUILD_EXAMPLES=OFF \
    -DLLAMA_BUILD_SERVER=ON \
    -DGGML_VULKAN=ON \
    -DBUILD_SHARED_LIBS=OFF

cmake --build build --target llama-server -j$(nproc)

# Verify build
BINARY="$LLAMA_DIR/build/bin/llama-server"
if [ -f "$BINARY" ]; then
    BINARY_SIZE=$(du -h "$BINARY" | cut -f1)
    echo ""
    echo "✅ Build successful!"
    echo "   Binary: $BINARY"
    echo "   Size: $BINARY_SIZE"
    echo ""
    echo "📋 To test:"
    echo "   $BINARY --help"
else
    echo "❌ Build failed. Binary not found at: $BINARY"
    exit 1
fi
