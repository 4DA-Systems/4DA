#!/bin/bash
# Sync AI coding tool rules from single source of truth (.ai/RULES.md)
# Usage: bash scripts/sync-ai-rules.sh
#
# Generates config files for:
#   - Claude Code (CLAUDE.md)
#   - Cursor (.cursor/rules/project.mdc)
#   - GitHub Copilot (.github/copilot-instructions.md)
#   - Windsurf (.windsurfrules)
#   - Cline (.clinerules)
#   - Aider (CONVENTIONS.md)

set -e
cd "$(git rev-parse --show-toplevel)"

SOURCE=".ai/RULES.md"

if [ ! -f "$SOURCE" ]; then
    echo "Error: $SOURCE not found"
    exit 1
fi

# Strip the header comment (lines starting with >) from the source
CONTENT=$(sed '/^>/d' "$SOURCE")

# --- Claude Code ---
# Claude Code gets the full rules plus Claude-specific additions
cat > CLAUDE.md << 'CLAUDE_EOF'
# 4DA — Claude Code Instructions

CLAUDE_EOF
# Append shared rules (skip the first heading line since we wrote our own)
tail -n +2 "$SOURCE" | sed '/^>/d' >> CLAUDE.md
cat >> CLAUDE.md << 'CLAUDE_EXTRA'

## Claude-Specific

- Agent definitions: `.claude/agents/` (4DA-specific agents for source debugging, trend analysis, etc.)
- Slash commands: `.claude/commands/` (project-specific commands)
- MCP servers: memory (persistent decisions/learnings) and 4da (21 intelligence tools)
- Hooks: git hygiene monitor, prompt analyzer, session archiver
- Subagent rules: spawn for 3+ file changes, searching, debugging, testing, reviewing
CLAUDE_EXTRA
echo "  CLAUDE.md"

# --- Cursor ---
mkdir -p .cursor/rules
cat > .cursor/rules/project.mdc << MDC_EOF
---
description: 4DA project rules and conventions
globs:
alwaysApply: true
---

MDC_EOF
sed '/^>/d' "$SOURCE" >> .cursor/rules/project.mdc
echo "  .cursor/rules/project.mdc"

# --- GitHub Copilot ---
mkdir -p .github
{
    echo "# 4DA — Copilot Instructions"
    echo ""
    tail -n +2 "$SOURCE" | sed '/^>/d'
} > .github/copilot-instructions.md
echo "  .github/copilot-instructions.md"

# --- Windsurf ---
{
    echo "# 4DA — Project Rules"
    echo ""
    tail -n +2 "$SOURCE" | sed '/^>/d'
} > .windsurfrules
echo "  .windsurfrules"

# --- Cline ---
{
    echo "# 4DA — Project Rules"
    echo ""
    tail -n +2 "$SOURCE" | sed '/^>/d'
} > .clinerules
echo "  .clinerules"

# --- Aider ---
{
    echo "# 4DA — Conventions"
    echo ""
    tail -n +2 "$SOURCE" | sed '/^>/d'
} > CONVENTIONS.md
echo "  CONVENTIONS.md"

echo ""
echo "All AI tool configs synced from $SOURCE"
echo "Commit the generated files to share with your team."
