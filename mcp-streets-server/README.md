# STREETS MCP Server

MCP server for the **STREETS Developer Income Course**. Provides 9 tools for accessing course content, analyzing projects for revenue engine fit, and tracking lesson progress.

STREETS is a 7-module course that teaches developers how to build income streams using their existing skills, hardware, and software:

- **S** — Sovereign Setup (free)
- **T** — Technical Moats
- **R** — Revenue Engines
- **E1** — Execution Playbook
- **E2** — Evolving Edge
- **T2** — Tactical Automation
- **S2** — Stacking Streams

## Quick Start

```bash
pnpm install
pnpm build
pnpm start
```

## Available Tools

| Tool | Description |
|------|-------------|
| `get_module` | Retrieve a course module by ID with all parsed lessons |
| `get_template` | Retrieve a worksheet template (Sovereign Stack, Moat Map, Stream Stack) |
| `search_course` | Full-text search across all course modules |
| `get_engine` | Get details for a specific revenue engine (1-8) from Module R |
| `recommend_engines` | Analyze a project directory and recommend matching revenue engines |
| `assess_readiness` | Score a project against the Sovereign Setup checklist |
| `get_progress` | Get completion progress across all modules |
| `mark_complete` | Mark a specific lesson as complete |
| `get_next_step` | Get a recommendation for what to work on next |

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `STREETS_CONTENT_PATH` | `../docs/streets/` (relative to server) | Path to the directory containing course markdown files |

## Claude Desktop Configuration

Add to your Claude Desktop MCP config (`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "streets": {
      "command": "node",
      "args": ["D:/4DA/mcp-streets-server/dist/index.js"],
      "env": {
        "STREETS_CONTENT_PATH": "D:/4DA/docs/streets"
      }
    }
  }
}
```

## Claude Code Configuration

Add to `.claude/settings.json` or the project's MCP configuration:

```json
{
  "mcpServers": {
    "streets": {
      "command": "node",
      "args": ["D:/4DA/mcp-streets-server/dist/index.js"]
    }
  }
}
```

## MCP Inspector

Test tools interactively:

```bash
pnpm inspect
```

## Progress Storage

Lesson completion progress is stored in a local SQLite database:

- **Windows:** `%LOCALAPPDATA%/streets/progress.db`
- **macOS/Linux:** `~/.local/share/streets/progress.db`

## Development

```bash
pnpm dev    # Watch mode — recompiles on file changes
```

## License

MIT
