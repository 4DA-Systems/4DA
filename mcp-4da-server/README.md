# @4da/mcp-server

MCP server that plugs [4DA](https://github.com/4da-dev/4da-home)'s personalized developer intelligence into Claude Code, Cursor, Windsurf, or any MCP-compatible host.

4DA monitors 11 sources (Hacker News, arXiv, Reddit, GitHub, Product Hunt, YouTube, etc.), scores everything against your codebase and tech stack, and rejects 99%+ as noise. This MCP server gives your AI tools access to what survived.

## Quick Setup

**Prerequisites:** [4DA](https://github.com/4da-dev/4da-home) must be installed and have run at least one analysis.

### Claude Code

```bash
# Add to your project's .mcp.json
claude mcp add 4da node /path/to/mcp-4da-server/dist/index.js
```

Or manually add to `.mcp.json`:
```json
{
  "mcpServers": {
    "4da": {
      "command": "node",
      "args": ["/path/to/mcp-4da-server/dist/index.js"],
      "env": {
        "FOURDA_DB_PATH": "/path/to/4da-v3/data/4da.db"
      }
    }
  }
}
```

### Cursor / Windsurf

Add to `.cursor/mcp.json` or equivalent:
```json
{
  "mcpServers": {
    "4da": {
      "command": "node",
      "args": ["/path/to/mcp-4da-server/dist/index.js"],
      "env": {
        "FOURDA_DB_PATH": "/path/to/4da-v3/data/4da.db"
      }
    }
  }
}
```

### Claude Desktop

Add to your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):
```json
{
  "mcpServers": {
    "4da": {
      "command": "node",
      "args": ["/path/to/mcp-4da-server/dist/index.js"],
      "env": {
        "FOURDA_DB_PATH": "/path/to/4da-v3/data/4da.db"
      }
    }
  }
}
```

## Tools (13)

### Core

| Tool | Description |
|------|-------------|
| `get_relevant_content` | Filtered content feed — only what passed the 5-axis scoring gate |
| `get_context` | Your tech stack, interests, learned affinities, ACE-detected topics |
| `explain_relevance` | Why a specific item was scored the way it was |
| `record_feedback` | Teach 4DA — save, dismiss, or mark items irrelevant |

### Intelligence

| Tool | Description |
|------|-------------|
| `get_actionable_signals` | Classified signals: security alerts, breaking changes, new tools |
| `daily_briefing` | Executive summary of recent discoveries (AI-powered with synthesize=true) |
| `score_autopsy` | Forensic analysis of any item's scoring breakdown |
| `trend_analysis` | Statistical patterns and anomaly detection across your feed |
| `context_analysis` | Recommendations to optimize your context for better results |
| `topic_connections` | Knowledge graph of how your content topics relate |

### Diagnostic

| Tool | Description |
|------|-------------|
| `source_health` | Pipeline status for each source adapter |
| `config_validator` | Configuration validation and issue detection |
| `llm_status` | LLM provider status and Ollama availability |

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FOURDA_DB_PATH` | Path to 4DA's SQLite database | Auto-detected from `~/.4da/4da.db` or `data/4da.db` |

## Build from Source

```bash
cd mcp-4da-server
pnpm install
pnpm build
```

## Test

```bash
# MCP Inspector (interactive)
pnpm run inspect

# Direct JSON-RPC
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | node dist/index.js
```

## Example Usage in Claude Code

Once configured, you can ask Claude:

- "What did 4DA find for me today?"
- "Are there any security alerts in my feed?"
- "Give me a briefing on what's relevant to my current project"
- "Why was this item scored highly?"
- "What are the trending topics in my domain?"

Claude will use the MCP tools automatically to answer from your personalized feed.

## License

MIT
