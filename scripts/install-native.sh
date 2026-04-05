#!/bin/bash
# ═══════════════════════════════════════════════════════
# Native Installation Script
# ═══════════════════════════════════════════════════════
# Installs LLM API Server as native binaries with systemd services
# Replaces Docker deployment with maximum performance
#
# Usage:
#   sudo ./scripts/install-native.sh
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Installation paths
INSTALL_PREFIX="/opt/llm-api"
CONFIG_DIR="/etc/llm-api"
SYSTEMD_DIR="/etc/systemd/system"
LOG_DIR="/opt/llm-api/logs"
MODELS_DIR="$PROJECT_ROOT/models"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║${NC}  LLM API Server - Native Installation          ${BLUE}║${NC}"
echo -e "${BLUE}║${NC}  Binaries + systemd (No Docker)                ${BLUE}║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════╝${NC}"
echo ""

# ═══════════════════════════════════════════════════════
# 1. Check root
# ═══════════════════════════════════════════════════════
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}❌ This script must be run as root (sudo)${NC}"
    exit 1
fi

# ═══════════════════════════════════════════════════════
# 2. Install dependencies
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}[1/8]${NC} Installing system dependencies..."

apt-get update -qq
apt-get install -y -qq \
    build-essential \
    cmake \
    git \
    curl \
    pkg-config \
    libssl-dev \
    libvulkan-dev \
    mesa-vulkan-drivers \
    vulkan-validationlayers \
    systemd

echo -e "${GREEN}✅ Dependencies installed${NC}"
echo ""

# ═══════════════════════════════════════════════════════
# 3. Install Rust if not present
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}[2/8]${NC} Checking Rust installation..."

if ! command -v rustc &> /dev/null; then
    echo -e "${YELLOW}⚠️  Rust not found, installing...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo -e "${GREEN}✅ Rust installed${NC}"
else
    echo -e "${GREEN}✅ Rust found: $(rustc --version)${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════
# 4. Create system user and directories
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}[3/8]${NC} Creating system user and directories..."

# Create dedicated user (no login)
if ! id -u llm-api &>/dev/null; then
    useradd --system --no-create-home --shell /usr/sbin/nologin llm-api
    echo -e "${GREEN}✅ Created user 'llm-api'${NC}"
else
    echo -e "${GREEN}✅ User 'llm-api' already exists${NC}"
fi

# Create directories
mkdir -p "$INSTALL_PREFIX/bin"
mkdir -p "$INSTALL_PREFIX/logs"
mkdir -p "$CONFIG_DIR"
mkdir -p "$LOG_DIR"

# Set permissions
chown -R llm-api:llm-api "$INSTALL_PREFIX"
chown -R llm-api:llm-api "$CONFIG_DIR"

echo -e "${GREEN}✅ Directories created${NC}"
echo ""

# ═══════════════════════════════════════════════════════
# 5. Build llama-server
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}[4/8]${NC} Building llama-server with Vulkan GPU support..."

# Run build script (without root, as current user)
su -l "$SUDO_USER" -c "bash '$SCRIPT_DIR/build-llama-server.sh' '$INSTALL_PREFIX/build'"

# Install binary
LLAMA_BINARY="$INSTALL_PREFIX/build/llama.cpp/build/bin/llama-server"
if [ -f "$LLAMA_BINARY" ]; then
    cp "$LLAMA_BINARY" "$INSTALL_PREFIX/bin/llama-server"
    chmod +x "$INSTALL_PREFIX/bin/llama-server"
    chown llm-api:llm-api "$INSTALL_PREFIX/bin/llama-server"
    echo -e "${GREEN}✅ llama-server installed${NC}"
else
    echo -e "${RED}❌ llama-server build failed${NC}"
    exit 1
fi
echo ""

# ═══════════════════════════════════════════════════════
# 6. Build Rust API
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}[5/8]${NC} Building Rust API..."

# Run build script (without root, as current user)
su -l "$SUDO_USER" -c "bash '$SCRIPT_DIR/build-api.sh'"

# Install binary
API_BINARY="$PROJECT_ROOT/api/target/release/rust_llm_api"
if [ -f "$API_BINARY" ]; then
    cp "$API_BINARY" "$INSTALL_PREFIX/bin/rust_llm_api"
    chmod +x "$INSTALL_PREFIX/bin/rust_llm_api"
    chown llm-api:llm-api "$INSTALL_PREFIX/bin/rust_llm_api"
    echo -e "${GREEN}✅ Rust API installed${NC}"
else
    echo -e "${RED}❌ Rust API build failed${NC}"
    exit 1
fi
echo ""

# ═══════════════════════════════════════════════════════
# 7. Configure environment
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}[6/8]${NC} Configuring environment..."

# Copy .env example if config doesn't exist
if [ ! -f "$CONFIG_DIR/.env" ]; then
    cp "$PROJECT_ROOT/.env.example" "$CONFIG_DIR/.env"
    echo -e "${YELLOW}⚠️  Created default config at $CONFIG_DIR/.env${NC}"
    echo -e "${YELLOW}   EDIT THIS FILE before starting services!${NC}"
else
    echo -e "${GREEN}✅ Configuration already exists at $CONFIG_DIR/.env${NC}"
fi

# Create models symlink if needed
if [ ! -L "$INSTALL_PREFIX/models" ] && [ -d "$MODELS_DIR" ]; then
    ln -sf "$MODELS_DIR" "$INSTALL_PREFIX/models"
    echo -e "${GREEN}✅ Models directory linked${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════
# 8. Install systemd services
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}[7/8]${NC} Installing systemd services..."

# Copy service files
cp "$PROJECT_ROOT/systemd/llama-server.service" "$SYSTEMD_DIR/"
cp "$PROJECT_ROOT/systemd/llm-api.service" "$SYSTEMD_DIR/"

# Reload systemd
systemctl daemon-reload

echo -e "${GREEN}✅ Services installed${NC}"
echo ""

# ═══════════════════════════════════════════════════════
# 9. Enable and start services
# ═══════════════════════════════════════════════════════
echo -e "${BLUE}[8/8]${NC} Enabling services..."

echo -e "${YELLOW}⚠️  Services will be enabled but NOT started automatically${NC}"
echo -e "${YELLOW}   Edit $CONFIG_DIR/.env first, then run:${NC}"
echo -e ""
echo -e "   sudo systemctl start llama-server"
echo -e "   sudo systemctl start llm-api"
echo -e ""

systemctl enable llama-server
systemctl enable llm-api

echo -e "${GREEN}✅ Services enabled on boot${NC}"
echo ""

# ═══════════════════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════════════════
echo -e "${GREEN}╔══════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║${NC}  ✅ Installation Complete!                      ${GREEN}║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${BLUE}📁 Installed files:${NC}"
echo -e "   Binary:     $INSTALL_PREFIX/bin/llama-server"
echo -e "   Binary:     $INSTALL_PREFIX/bin/rust_llm_api"
echo -e "   Config:     $CONFIG_DIR/.env"
echo -e "   Logs:       $INSTALL_PREFIX/logs/"
echo -e "   Services:   $SYSTEMD_DIR/llama-server.service"
echo -e "               $SYSTEMD_DIR/llm-api.service"
echo ""
echo -e "${BLUE}🚀 Next steps:${NC}"
echo -e ""
echo -e "   1. Edit configuration:"
echo -e "      sudo nano $CONFIG_DIR/.env"
echo -e ""
echo -e "   2. Download a model:"
echo -e "      ./scripts/download-model.sh bartowski/google_gemma-4-E4B-it-GGUF \\"
echo -e "          google_gemma-4-E4B-it-Q4_K_M.gguf"
echo -e ""
echo -e "   3. Start services:"
echo -e "      sudo systemctl start llama-server"
echo -e "      sudo systemctl start llm-api"
echo -e ""
echo -e "   4. Check status:"
echo -e "      sudo systemctl status llama-server llm-api"
echo -e "      journalctl -u llama-server -f"
echo -e "      journalctl -u llm-api -f"
echo -e ""
echo -e "${BLUE}📖 Documentation:${NC}"
echo -e "   See docs/NATIVE-DEPLOY.md for detailed guide"
echo ""
