#!/usr/bin/env bash
set -e

# ==============================================================================
# AGY Suite - Universal Single-Line Installer for Linux
# Repository: https://github.com/simhaonline/agy-agent.git
# Usage: curl -fsSL https://raw.githubusercontent.com/simhaonline/agy-agent/main/install.sh | bash
# ==============================================================================

REPO="simhaonline/agy-agent"
BRANCH="main"
RAW_BASE="https://raw.githubusercontent.com/${REPO}/${BRANCH}"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

printf "${BLUE}${BOLD}"
cat << 'EOF'
     ___       _______   ____    ____ 
    /   \     /  _____|  \   \  /   / 
   /  ^  \   |  |  __     \   \/   /  
  /  /_\  \  |  | |_ |     \_    _/   
 /  _____  \ |  |__| |       |  |     
/__/     \__\ \______|       |__|     
EOF
printf "${NC}\n"
echo -e "${BOLD}Installing AGY Suite (Standalone Rust Agent & Binary Patcher)${NC}"
echo "------------------------------------------------------------------"

# Detect OS & Architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

if [ "$OS" != "Linux" ]; then
    echo -e "${RED}[ERROR] Operating system '$OS' is not directly supported by this installer.${NC}"
    echo "Precompiled binaries are built for Linux (MUSL static). On macOS or BSD, build from source using:"
    echo "  git clone https://github.com/${REPO}.git && cd agy-agent && cargo build --release"
    exit 1
fi

case "$ARCH" in
    x86_64|amd64)
        DIST_ARCH="linux-x64"
        ;;
    i386|i686)
        DIST_ARCH="linux-x86"
        ;;
    *)
        echo -e "${RED}[ERROR] Unsupported CPU architecture: $ARCH${NC}"
        echo "AGY Suite provides precompiled binaries for x86_64 and i686."
        echo "For ARM/aarch64, compile from source: cargo build --release --workspace"
        exit 1
        ;;
esac

# Target install directory
if [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
    NEED_PATH_UPDATE=0
else
    INSTALL_DIR="$HOME/.agy/bin"
    NEED_PATH_UPDATE=1
fi

mkdir -p "$INSTALL_DIR"
echo -e "${BLUE}[*] Target directory:${NC} $INSTALL_DIR"
echo -e "${BLUE}[*] Detected architecture:${NC} $ARCH ($DIST_ARCH)"

# Download Binaries
download_binary() {
    local bin_name="$1"
    local dest="$INSTALL_DIR/$bin_name"
    local url="${RAW_BASE}/dist/${DIST_ARCH}/${bin_name}"
    
    echo -e "${BLUE}[*] Downloading ${bin_name}...${NC}"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$dest"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$dest" "$url"
    else
        echo -e "${RED}[ERROR] Neither curl nor wget was found. Please install either tool.${NC}"
        exit 1
    fi
    chmod +x "$dest"
}

download_binary "agy-agent"
download_binary "agy-patcher"

# PATH configuration for non-root install
if [ "$NEED_PATH_UPDATE" -eq 1 ]; then
    SHELL_PROFILE=""
    if [ -n "$ZSH_VERSION" ] || [ -f "$HOME/.zshrc" ]; then
        SHELL_PROFILE="$HOME/.zshrc"
    elif [ -f "$HOME/.bashrc" ]; then
        SHELL_PROFILE="$HOME/.bashrc"
    elif [ -f "$HOME/.profile" ]; then
        SHELL_PROFILE="$HOME/.profile"
    fi

    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        if [ -n "$SHELL_PROFILE" ]; then
            echo "" >> "$SHELL_PROFILE"
            echo "# Added by AGY Suite installer" >> "$SHELL_PROFILE"
            echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$SHELL_PROFILE"
            echo -e "${YELLOW}[!] Added $INSTALL_DIR to $SHELL_PROFILE${NC}"
            echo -e "${YELLOW}[!] Run 'source $SHELL_PROFILE' or restart your terminal to update PATH.${NC}"
        fi
        export PATH="$INSTALL_DIR:$PATH"
    fi
fi

echo "------------------------------------------------------------------"
echo -e "${GREEN}${BOLD}[SUCCESS] AGY Suite installed successfully!${NC}"
echo ""
"$INSTALL_DIR/agy-agent" --version || true
"$INSTALL_DIR/agy-patcher" --version || true
echo ""
echo -e "${BOLD}Quick Start:${NC}"
echo "  agy-agent chat                     # Launch interactive AI assistant"
echo "  agy-agent run \"<objective>\"        # Execute autonomous coding goal"
echo "  agy-agent preset list              # View security and behavior presets"
echo "  agy-patcher inspect <binary>       # Inspect binary PE structure"
echo "------------------------------------------------------------------"
