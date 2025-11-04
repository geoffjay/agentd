#!/usr/bin/env bash
# Installation script for agentd
# This script provides an interactive installation experience

set -e

# Colors
BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
PREFIX="${PREFIX:-/usr/local}"
BINDIR="$PREFIX/bin"
LOGDIR="$PREFIX/var/log"
PLIST_DIR_USER="$HOME/Library/LaunchAgents"

echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     agentd Installation Script         ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo ""

# Check if running on macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo -e "${RED}Error: This script is for macOS only${NC}"
    exit 1
fi

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Rust/Cargo not found${NC}"
    echo "Please install Rust from https://rustup.rs/"
    exit 1
fi

echo -e "${GREEN}✓ Rust found: $(cargo --version)${NC}"

# Check if we're in the right directory
if [[ ! -f "Cargo.toml" ]] || [[ ! -d "crates" ]]; then
    echo -e "${RED}Error: Must be run from the agentd project root${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Project directory verified${NC}"
echo ""

# Ask user for installation type
echo -e "${YELLOW}Choose installation type:${NC}"
echo "  1) User installation (recommended, no sudo needed)"
echo "  2) System installation (requires sudo)"
echo ""
read -p "Enter choice [1]: " install_type
install_type="${install_type:-1}"

echo ""
echo -e "${BLUE}Building binaries (this may take a few minutes)...${NC}"
if cargo build --release; then
    echo -e "${GREEN}✓ Build successful${NC}"
else
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi

echo ""
echo -e "${BLUE}Installing binaries to $BINDIR...${NC}"

# Create directories
mkdir -p "$LOGDIR" 2>/dev/null || sudo mkdir -p "$LOGDIR"

# Install binaries
binaries=(
    "agent:target/release/agent"
    "agentd-notify:target/release/agentd-notify"
    "agentd-ask:target/release/agentd-ask"
    "agentd-hook:target/release/agentd-hook"
    "agentd-monitor:target/release/agentd-monitor"
    "Agent:target/release/Agent"
)

for binary_pair in "${binaries[@]}"; do
    name="${binary_pair%%:*}"
    path="${binary_pair##*:}"

    if [[ -f "$path" ]]; then
        echo "  Installing $name..."
        if install -m 755 "$path" "$BINDIR/$name" 2>/dev/null || \
           sudo install -m 755 "$path" "$BINDIR/$name"; then
            echo -e "    ${GREEN}✓${NC} $name installed"
        else
            echo -e "    ${RED}✗${NC} Failed to install $name"
            exit 1
        fi
    else
        echo -e "    ${YELLOW}⚠${NC} $name not found (may not be built yet)"
    fi
done

echo ""
echo -e "${BLUE}Installing service plist files...${NC}"

# Create plist directory
mkdir -p "$PLIST_DIR_USER"

# Install plist files
services=(
    "agentd-notify"
    "agentd-ask"
    "agentd-hook"
    "agentd-monitor"
)

for service in "${services[@]}"; do
    plist_file="contrib/plists/com.geoffjay.$service.plist"

    if [[ -f "$plist_file" ]]; then
        echo "  Installing $service..."
        cp "$plist_file" "$PLIST_DIR_USER/"
        echo -e "    ${GREEN}✓${NC} $service plist installed"
    else
        echo -e "    ${RED}✗${NC} Plist file not found: $plist_file"
    fi
done

echo ""
echo -e "${GREEN}✓ Installation complete!${NC}"
echo ""

# Ask if user wants to start services
read -p "Start services now? [Y/n]: " start_services
start_services="${start_services:-Y}"

if [[ "$start_services" =~ ^[Yy] ]]; then
    echo ""
    echo -e "${BLUE}Starting services...${NC}"

    for service in "${services[@]}"; do
        plist_path="$PLIST_DIR_USER/com.geoffjay.$service.plist"

        if [[ -f "$plist_path" ]]; then
            echo "  Starting $service..."
            if launchctl load "$plist_path" 2>/dev/null; then
                echo -e "    ${GREEN}✓${NC} $service started"
            else
                echo -e "    ${YELLOW}⚠${NC} $service may already be running or failed to start"
            fi
        fi
    done

    echo ""
    echo -e "${BLUE}Checking service status...${NC}"
    sleep 2

    for service in "${services[@]}"; do
        if launchctl list | grep -q "com.geoffjay.$service"; then
            echo -e "  $service: ${GREEN}running${NC}"
        else
            echo -e "  $service: ${RED}stopped${NC}"
        fi
    done
fi

echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Installation Summary${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "  ${GREEN}✓${NC} Binaries installed to: $BINDIR"
echo -e "  ${GREEN}✓${NC} Services installed to: $PLIST_DIR_USER"
echo -e "  ${GREEN}✓${NC} Logs will be written to: $LOGDIR"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo ""
echo -e "  Test the CLI:"
echo -e "    ${BLUE}agent notify create --title \"Test\" --message \"Hello\"${NC}"
echo -e "    ${BLUE}agent notify list${NC}"
echo ""
echo -e "  Check service health:"
echo -e "    ${BLUE}curl http://localhost:3000/health${NC}"
echo -e "    ${BLUE}curl http://localhost:3001/health${NC}"
echo ""
echo -e "  View logs:"
echo -e "    ${BLUE}tail -f $LOGDIR/agentd-notify.log${NC}"
echo ""
echo -e "  Manage services:"
echo -e "    ${BLUE}make services-status${NC}  # Check status"
echo -e "    ${BLUE}make services-stop${NC}    # Stop all services"
echo -e "    ${BLUE}make services-start${NC}   # Start all services"
echo ""
echo -e "For more information, see: ${BLUE}INSTALL.md${NC}"
echo ""
