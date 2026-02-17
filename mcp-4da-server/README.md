# @4da/mcp-server

Developer Intelligence Protocol for Claude Code, Cursor, Windsurf, or any MCP-compatible host.

4DA scores content from Hacker News, arXiv, Reddit, GitHub, and 7 more sources against your actual codebase and tech stack. This MCP server gives your AI tools access to 26 intelligence tools — from scored content feeds to decision tracking to knowledge gap detection.

## Quick Setup

**Prerequisites:** [4DA](https://github.com/runyourempire/4DA) must be installed and have run at least one analysis.

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
        "FOURDA_DB_PATH": "/path/to/4DA/data/4da.db"
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
        "FOURDA_DB_PATH": "/path/to/4DA/data/4da.db"
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
        "FOURDA_DB_PATH": "/path/to/4DA/data/4da.db"
      }
    }
  }
}
```

## Tools (26)

### Core (4)

| Tool | Description |
|------|-------------|
| `get_relevant_content` | Filtered content feed — only what passed the 5-axis scoring gate |
| `get_context` | Your tech stack, interests, learned affinities, ACE-detected topics |
| `explain_relevance` | Why a specific item was scored the way it was |
| `record_feedback` | Teach 4DA — save, dismiss, or mark items irrelevant |

### Intelligence (9)

| Tool | Description |
|------|-------------|
| `get_actionable_signals` | Classified signals: security alerts, breaking changes, new tools |
| `daily_briefing` | Executive summary of recent discoveries (AI-powered) |
| `score_autopsy` | Forensic analysis of any item's scoring breakdown |
| `trend_analysis` | Statistical patterns and anomaly detection across your feed |
| `context_analysis` | Recommendations to optimize your context for better results |
| `topic_connections` | Knowledge graph of how your content topics relate |
| `signal_chains` | Causal chains connecting related signal events over time |
| `semantic_shifts` | Narrative drift detection for tracked topics |
| `attention_report` | Attention allocation analysis vs codebase dependencies |

### Diagnostic (3)

| Tool | Description |
|------|-------------|
| `source_health` | Pipeline status for each source adapter |
| `config_validator` | Configuration validation and issue detection |
| `llm_status` | LLM provider status and Ollama availability |

### Innovation (4)

| Tool | Description |
|------|-------------|
| `export_context_packet` | Portable context snapshot for session handoff |
| `knowledge_gaps` | Cross-reference dependencies vs content to find blind spots |
| `reverse_mentions` | Find where your projects appear in monitored sources |
| `project_health` | Dependency freshness and security radar |

### Decision Intelligence (3)

| Tool | Description |
|------|-------------|
| `decision_memory` | Record, query, and align architectural decisions |
| `tech_radar` | Technology adoption signals from decisions + content |
| `decision_enforcement` | Check if proposed changes align with recorded decisions |

### Agent Autonomy (3)

| Tool | Description |
|------|-------------|
| `agent_memory` | Cross-session, cross-agent persistent memory |
| `agent_session_brief` | Tailored startup context for agent onboarding |
| `delegation_score` | Composite trust score for task delegation decisions |

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
