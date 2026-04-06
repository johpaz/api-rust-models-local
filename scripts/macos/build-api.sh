#!/bin/bash
# ═══════════════════════════════════════════════════════
# Build script for Rust LLM API (macOS)
# ═══════════════════════════════════════════════════════
# Compiles the Rust API with release optimizations
#
# Usage:
#   ./scripts/macos/build-api.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
API_DIR="$PROJECT_ROOT/api"

echo "🦀 Building Rust API (macOS)..."
echo "   Project: $API_DIR"
echo ""

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo not found."
    echo "Install with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

cd "$API_DIR"

# Build with release optimizations
echo "🔨 Compiling with --release..."
cargo build --release

# Show build results
BINARY="$API_DIR/target/release/rust_llm_api"
if [ -f "$BINARY" ]; then
    BINARY_SIZE=$(du -h "$BINARY" | cut -f1)
    echo ""
    echo "✅ Build successful!"
    echo "   Binary: $BINARY"
    echo "   Size: $BINARY_SIZE"
    echo ""
    echo "📋 To run manually:"
    echo "   cd $API_DIR && cargo run --release"
    echo ""
else
    echo "❌ Build failed. Binary not found at: $BINARY"
    exit 1
fi
