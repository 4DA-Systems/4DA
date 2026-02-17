#!/bin/bash
# 4DA Setup Validation / Health Check
# Verifies all components of the dev environment are working.
# Usage: bash scripts/validate-setup.sh

set -o pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

ERRORS=0
WARNINGS=0

pass() { echo -e "  ${GREEN}OK${NC}  $1"; }
warn() { echo -e "  ${YELLOW}!!${NC}  $1"; WARNINGS=$((WARNINGS + 1)); }
fail() { echo -e "  ${RED}XX${NC}  $1"; ERRORS=$((ERRORS + 1)); }

cd "$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "Not in a git repo"; exit 1; }

echo "=== 4DA Health Check ==="
echo ""

# --- Prerequisites ---
echo "[Prerequisites]"
command -v node &>/dev/null   && pass "Node.js $(node --version)" || fail "Node.js missing"
command -v pnpm &>/dev/null   && pass "pnpm"                      || fail "pnpm missing"
command -v cargo &>/dev/null  && pass "Rust/Cargo"                 || fail "Rust missing"
command -v git &>/dev/null    && pass "Git"                        || fail "Git missing"
command -v ollama &>/dev/null && pass "Ollama"                     || warn "Ollama not installed (local embeddings disabled)"
echo ""

# --- Node Dependencies ---
echo "[Dependencies]"
[ -d "node_modules" ]                  && pass "node_modules exists"          || fail "node_modules missing (run pnpm install)"
[ -d "mcp-memory-server/node_modules" ] && pass "MCP memory server deps"     || warn "MCP memory server deps missing"
[ -d "mcp-4da-server/node_modules" ]    && pass "MCP 4DA server deps"        || warn "MCP 4DA server deps missing"
echo ""

# --- MCP Server Builds ---
echo "[MCP Servers]"
[ -f "mcp-memory-server/dist/index.js" ] && pass "Memory server built"  || fail "Memory server not built (cd mcp-memory-server && pnpm run build)"
[ -f "mcp-4da-server/dist/index.js" ]    && pass "4DA server built"     || fail "4DA server not built (cd mcp-4da-server && pnpm run build)"
echo ""

# --- Config Files ---
echo "[Configuration]"
[ -f "data/settings.json" ]          && pass "data/settings.json"              || fail "data/settings.json missing (cp data/settings.example.json data/settings.json)"
[ -f "data/settings.example.json" ]  && pass "data/settings.example.json"      || warn "Settings template missing"
[ -f ".mcp.json" ]                   && pass ".mcp.json"                       || fail ".mcp.json missing"
[ -f "CLAUDE.md" ]                   && pass "CLAUDE.md"                       || warn "CLAUDE.md missing"
echo ""

# --- Claude Code Config ---
echo "[Claude Code Setup]"
[ -f ".claude/settings.json" ]       && pass "Project hooks config"            || warn "No project hooks configured"
[ -d ".claude/agents" ]              && pass "Project agents ($(ls .claude/agents/*.md 2>/dev/null | wc -l) files)" || warn "No project agents"
[ -d ".claude/commands" ]            && pass "Slash commands ($(ls .claude/commands/*.md 2>/dev/null | wc -l) files)" || warn "No slash commands"
[ -d ".claude/scripts" ]             && pass "Automation scripts"              || warn "No automation scripts"
echo ""

# --- Databases ---
echo "[Databases]"
[ -f "data/4da.db" ]     && pass "4DA database ($(du -h data/4da.db | cut -f1))"       || warn "4DA database not created yet (created on first run)"
[ -f ".claude/memory.db" ] && pass "Memory database ($(du -h .claude/memory.db | cut -f1))" || warn "Memory database not created yet"
echo ""

# --- Script Permissions ---
echo "[Script Permissions]"
for script in .claude/scripts/*.sh .claude/hooks/*.sh; do
    if [ -f "$script" ]; then
        [ -x "$script" ] && pass "$script" || warn "$script not executable"
    fi
done
echo ""

# --- Path Validation ---
echo "[Path Checks]"
if [ -f ".mcp.json" ]; then
    # Check if .mcp.json references paths that exist
    MCP_PATHS=$(grep -oP '"[^"]*index\.js"' .mcp.json | tr -d '"')
    for p in $MCP_PATHS; do
        [ -f "$p" ] && pass "MCP path: $p" || fail "MCP path missing: $p"
    done
fi
echo ""

# --- Disk Usage ---
echo "[Disk Usage]"
SESSIONS=$(find .claude/sessions/transcripts -name "*.jsonl" 2>/dev/null | wc -l | tr -d ' ')
if [ "$SESSIONS" -gt 50 ]; then
    warn "$SESSIONS session transcripts (consider: bash .claude/scripts/prune-sessions.sh)"
elif [ "$SESSIONS" -gt 0 ]; then
    pass "$SESSIONS session transcripts"
else
    pass "No session transcripts (clean)"
fi
echo ""

# --- .ai/ Documentation ---
echo "[Project Docs]"
for doc in .ai/INVARIANTS.md .ai/DECISIONS.md .ai/ARCHITECTURE.md .ai/FAILURE_MODES.md; do
    [ -f "$doc" ] && pass "$doc" || warn "$doc missing"
done
echo ""

# --- Summary ---
echo "==========================="
if [ "$ERRORS" -eq 0 ] && [ "$WARNINGS" -eq 0 ]; then
    echo -e "${GREEN}All checks passed!${NC}"
elif [ "$ERRORS" -eq 0 ]; then
    echo -e "${YELLOW}Passed with ${WARNINGS} warning(s)${NC}"
else
    echo -e "${RED}${ERRORS} error(s), ${WARNINGS} warning(s)${NC}"
    echo "Fix errors above and re-run: bash scripts/validate-setup.sh"
fi
echo "==========================="
exit $ERRORS
