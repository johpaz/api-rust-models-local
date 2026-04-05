#!/bin/bash
# Apply TurboQuant patches to llama.cpp
# Now uses Python script for idempotent, reliable patching

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Determine llama.cpp root directory
# Try: 1) current directory, 2) ../build-native/llama.cpp
if [ -f "ggml/include/ggml.h" ]; then
    LLAMA_ROOT="$(pwd)"
elif [ -d "$SCRIPT_DIR/../build-native/llama.cpp" ]; then
    LLAMA_ROOT="$SCRIPT_DIR/../build-native/llama.cpp"
else
    echo "❌ Cannot find llama.cpp root directory"
    echo "Run this script from llama.cpp root or ensure build-native/llama.cpp exists"
    exit 1
fi

echo "🔧 Applying TurboQuant patches to llama.cpp..."
echo "   Root: $LLAMA_ROOT"
echo ""

# Check for Python
if command -v python3 &> /dev/null; then
    PYTHON=python3
elif command -v python &> /dev/null; then
    PYTHON=python
else
    echo "❌ Python not found. Required for applying patches."
    exit 1
fi

# Run the Python patch script
cd "$LLAMA_ROOT"
$PYTHON "$SCRIPT_DIR/apply-turboquant.py"

echo ""
echo "📋 To compile with TurboQuant + Vulkan:"
echo "   cmake -B build -DGGML_VULKAN=ON -DLLAMA_BUILD_SERVER=ON"
echo "   cmake --build build --target llama-server -j\$(nproc)"
