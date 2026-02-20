# mcp-memory-server

Persistent project memory for Claude Code sessions. Survives context compaction by storing decisions, state, learnings, and code locations in a local SQLite database.

Also provides access to archived session transcripts for referencing past conversations.

## Install

### Claude Code

```bash
claude mcp add memory -- node /path/to/mcp-memory-server/dist/index.js
```

### Cursor / Windsurf

Add to `~/.cursor/mcp.json`:
```json
{
  "mcpServers": {
    "memory": {
      "command": "node",
      "args": ["/path/to/mcp-memory-server/dist/index.js"],
      "env": {
        "MEMORY_DB_PATH": "/path/to/.claude/memory.db"
      }
    }
  }
}
```

## Tools (16)

### Memory (8)

| Tool | Description |
|------|-------------|
| `remember_decision` | Store an architectural or design decision with rationale and alternatives |
| `recall_decisions` | Retrieve stored decisions by key or search term |
| `update_state` | Track current project state (current task, blockers, last modified file) |
| `get_state` | Get current project state to understand what was being worked on |
| `remember_learning` | Store a development insight, gotcha, or pattern by topic |
| `recall_learnings` | Search stored learnings by topic or content |
| `remember_code_location` | Bookmark an important file/line for quick reference |
| `recall_code_locations` | Find remembered code locations by name, path, or purpose |

### Search (1)

| Tool | Description |
|------|-------------|
| `search_memory` | Full-text search across all stored memory (decisions, learnings, state) |

### Sessions (4)

| Tool | Description |
|------|-------------|
| `list_sessions` | List all archived session transcripts with optional indexing |
| `search_sessions` | Search through past session transcripts by keyword |
| `get_session_messages` | Read messages from a specific past session with pagination |
| `index_sessions` | Index all session transcripts for faster searching |

### Metrics (3)

| Tool | Description |
|------|-------------|
| `record_metric` | Record a quality metric (rework, iterations, gate passes, confidence) |
| `get_metrics` | Retrieve quality metrics with optional filtering by type and date |
| `get_quality_report` | Generate an aggregated quality summary report |

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `MEMORY_DB_PATH` | Path to the SQLite memory database | `~/.claude/memory.db` |
| `SESSIONS_DIR` | Path to archived session transcripts | `~/.claude/sessions/transcripts` |

## Build from Source

```bash
cd mcp-memory-server
pnpm install
pnpm build
```

## Example Usage

Once configured, your AI assistant can:

- "What decisions have we made about the database?"
- "What did we learn about sqlite-vec?"
- "Where is the scoring algorithm defined?"
- "What were we working on last session?"
- "Search past conversations for the auth discussion"

The AI uses these tools automatically to maintain continuity across sessions.

## License

MIT
