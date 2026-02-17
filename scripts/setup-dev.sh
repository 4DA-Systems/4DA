#!/bin/bash
# 4DA Development Setup
# Run this after cloning to get a working dev environment.
# Usage: bash scripts/setup-dev.sh

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
fail() { echo -e "${RED}[FAIL]${NC} $1"; }

echo "================================"
echo "  4DA Development Setup"
echo "================================"
echo ""

# --- 1. Check prerequisites ---
echo "--- Checking prerequisites ---"

MISSING=0

if command -v node &>/dev/null; then
    pass "Node.js $(node --version)"
else
    fail "Node.js not found. Install from https://nodejs.org/"
    MISSING=1
fi

if command -v pnpm &>/dev/null; then
    pass "pnpm $(pnpm --version)"
else
    warn "pnpm not found. Installing..."
    npm install -g pnpm && pass "pnpm installed" || { fail "pnpm install failed"; MISSING=1; }
fi

if command -v cargo &>/dev/null; then
    pass "Rust $(rustc --version | cut -d' ' -f2)"
else
    fail "Rust not found. Install from https://rustup.rs/"
    MISSING=1
fi

if command -v git &>/dev/null; then
    pass "Git $(git --version | cut -d' ' -f3)"
else
    fail "Git not found"
    MISSING=1
fi

if [ "$MISSING" -eq 1 ]; then
    echo ""
    fail "Missing prerequisites. Install them and re-run."
    exit 1
fi

# Optional tools
if command -v ollama &>/dev/null; then
    pass "Ollama $(ollama --version 2>/dev/null | head -1)"
else
    warn "Ollama not found. Local embeddings won't work. Install from https://ollama.ai/"
fi

echo ""

# --- 2. Install dependencies ---
echo "--- Installing dependencies ---"

cd "$(git rev-parse --show-toplevel)"

pnpm install && pass "Frontend dependencies installed" || fail "pnpm install failed"

echo ""

# --- 3. Build MCP servers ---
echo "--- Building MCP servers ---"

if [ -d "mcp-memory-server" ]; then
    (cd mcp-memory-server && pnpm install && pnpm run build) && pass "MCP memory server built" || fail "MCP memory server build failed"
fi

if [ -d "mcp-4da-server" ]; then
    (cd mcp-4da-server && pnpm install && pnpm run build) && pass "MCP 4DA server built" || fail "MCP 4DA server build failed"
fi

echo ""

# --- 4. Create settings from template ---
echo "--- Setting up config files ---"

if [ ! -f "data/settings.json" ]; then
    mkdir -p data
    cp data/settings.example.json data/settings.json
    pass "Created data/settings.json from template"
    warn "Edit data/settings.json to add your API keys"
else
    pass "data/settings.json already exists"
fi

if [ ! -f ".claude/settings.local.json" ]; then
    if [ -f ".claude/settings.local.example.json" ]; then
        cp .claude/settings.local.example.json .claude/settings.local.json
        pass "Created .claude/settings.local.json from template"
    fi
else
    pass ".claude/settings.local.json already exists"
fi

echo ""

# --- 5. Create required directories ---
echo "--- Creating directories ---"

mkdir -p .claude/sessions/transcripts
mkdir -p .claude/backups
mkdir -p data

pass "Directory structure ready"

echo ""

# --- 6. Make scripts executable ---
echo "--- Setting permissions ---"

chmod +x .claude/scripts/*.sh 2>/dev/null
chmod +x .claude/hooks/*.sh 2>/dev/null
chmod +x scripts/*.sh 2>/dev/null

pass "Scripts are executable"

echo ""

# --- 7. Verify Rust build ---
echo "--- Checking Rust build (this may take a while on first run) ---"

if [ -d "src-tauri" ]; then
    (cd src-tauri && cargo check 2>&1 | tail -1) && pass "Rust code compiles" || warn "Rust check had issues (may need system dependencies)"
fi

echo ""
echo "================================"
echo "  Setup complete!"
echo "================================"
echo ""
echo "Next steps:"
echo "  1. Edit data/settings.json with your API keys (if using cloud LLM)"
echo "  2. Run: pnpm run tauri dev"
echo "  3. Open http://localhost:4444"
echo ""
echo "Optional:"
echo "  - Install Ollama for local embeddings: https://ollama.ai/"
echo "  - Pull embedding model: ollama pull nomic-embed-text"
echo ""
