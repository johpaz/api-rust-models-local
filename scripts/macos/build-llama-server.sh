#!/bin/bash
# ═══════════════════════════════════════════════════════
# Build script for llama-server with Metal GPU support (macOS)
# ═══════════════════════════════════════════════════════
# Clones llama.cpp and compiles with Metal GPU acceleration
#
# Usage:
#   ./scripts/macos/build-llama-server.sh [install_dir]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"

INSTALL_DIR="${1:-$PROJECT_ROOT/llama-server/build-macos}"
LLAMA_DIR="$INSTALL_DIR/llama.cpp"

echo "🔧 Building llama-server with Metal GPU support (macOS)..."
echo "   Install directory: $INSTALL_DIR"
echo ""

# Detect architecture
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    echo "🍎 Apple Silicon detected (M1/M2/M3/M4)"
elif [ "$ARCH" = "x86_64" ]; then
    echo "🍎 Intel Mac detected"
else
    echo "⚠️  Unknown architecture: $ARCH"
fi

# Check dependencies
echo ""
echo "📋 Checking dependencies..."

if ! command -v cmake &> /dev/null; then
    echo "❌ cmake not found. Install with: brew install cmake"
    exit 1
fi

if ! command -v g++ &> /dev/null; then
    echo "❌ g++ not found. Install with: xcode-select --install"
    exit 1
fi

if ! command -v git &> /dev/null; then
    echo "❌ git not found. Install with: xcode-select --install"
    exit 1
fi

echo "✅ Dependencies OK"
echo ""

# Create build directory
mkdir -p "$INSTALL_DIR"

# Clone llama.cpp if not exists
if [ ! -d "$LLAMA_DIR" ]; then
    echo "📦 Cloning llama.cpp..."
    git clone --depth 1 --branch b5336 https://github.com/ggerganov/llama.cpp "$LLAMA_DIR"
    echo "✅ llama.cpp cloned"
else
    echo "✅ llama.cpp already exists at $LLAMA_DIR"
fi

cd "$LLAMA_DIR"

# Build with Metal GPU support
echo ""
echo "🔨 Compiling llama-server with Metal..."
echo "   Configuration:"
echo "   - GPU Backend: Metal"
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
    -DGGML_METAL=ON \
    -DBUILD_SHARED_LIBS=OFF

cmake --build build --target llama-server -j$(sysctl -n hw.ncpu)

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
    echo ""
    echo "🚀 To start server:"
    echo "   ./scripts/macos/start-llama-server.sh"
else
    echo "❌ Build failed. Binary not found at: $BINARY"
    exit 1
fi
