# @4da/mcp-server

Developer Intelligence Protocol for Claude Code, Cursor, VS Code, and any MCP-compatible host.

4DA scores content from Hacker News, arXiv, Reddit, GitHub, and 7 more sources against your actual codebase and tech stack. This MCP server gives your AI tools access to 27 intelligence tools — from scored content feeds to decision tracking to knowledge gap detection.

## Install

**Prerequisites:** [4DA desktop app](https://github.com/runyourempire/4DA) must be installed and have run at least one analysis.

### One-command setup (recommended)

```bash
npx @4da/mcp-server --setup
```

Auto-detects Claude Code, Cursor, and VS Code — writes the correct MCP config for each.

### Manual: Claude Code

```bash
claude mcp add 4da -- npx @4da/mcp-server
```

### Manual: Cursor / Windsurf

Add to `~/.cursor/mcp.json`:
```json
{
  "mcpServers": {
    "4da": {
      "command": "npx",
      "args": ["@4da/mcp-server"]
    }
  }
}
```

### Manual: VS Code (Copilot)

Add to `~/.vscode/mcp.json`:
```json
{
  "servers": {
    "4da": {
      "type": "stdio",
      "command": "npx",
      "args": ["@4da/mcp-server"]
    }
  }
}
```

### Manual: Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "4da": {
      "command": "npx",
      "args": ["@4da/mcp-server"]
    }
  }
}
```

## Transports

**stdio** (default) — works with all MCP hosts:
```bash
npx @4da/mcp-server
```

**Streamable HTTP** — for remote or multi-client setups:
```bash
npx @4da/mcp-server --http --port 4840
```

## Tools (27)

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

### Developer DNA (1)

| Tool | Description |
|------|-------------|
| `developer_dna` | Export your tech identity — stack, dependencies, engagement, blind spots |

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FOURDA_DB_PATH` | Path to 4DA's SQLite database | Auto-detected from standard locations |

## Build from Source

```bash
cd mcp-4da-server
pnpm install
pnpm build
```

## Test

```bash
pnpm test                # Run contract tests (38 tests)
pnpm run inspect         # MCP Inspector (interactive)
```

## Example Usage

Once configured, ask your AI assistant:

- "What did 4DA find for me today?"
- "Are there any security alerts in my feed?"
- "Give me a briefing on what's relevant to my current project"
- "Why was this item scored highly?"
- "What knowledge gaps do I have in my dependencies?"
- "Export my Developer DNA"

The AI will use the MCP tools automatically to answer from your personalized feed.

## License

MIT
