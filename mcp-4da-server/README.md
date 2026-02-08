# 4DA MCP Server

MCP (Model Context Protocol) server that exposes 4DA's personalized content filtering to AI agents like Claude Code, Cursor, and other MCP-compatible hosts.

## Overview

4DA (4 Dimensional Autonomy) is an ambient intelligence layer that monitors your local context and filters external content (Hacker News, arXiv, Reddit) to deliver only what matters to you. This MCP server allows AI agents to:

- Access your personalized content feed
- Understand your context (tech stack, interests, current work)
- Get explanations for why content was considered relevant
- Record feedback to help 4DA learn your preferences

## Installation

```bash
cd mcp-4da-server
npm install
npm run build
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FOURDA_DB_PATH` | Path to 4DA SQLite database | `data/4da.db` |

### Claude Code Integration

Add to your `.mcp.json` in the 4DA project root:

```json
{
  "mcpServers": {
    "4da": {
      "command": "node",
      "args": ["/mnt/d/4da-v3/mcp-4da-server/dist/index.js"],
      "env": {
        "FOURDA_DB_PATH": "/mnt/d/4da-v3/data/4da.db"
      }
    }
  }
}
```

### Claude Desktop Integration

Add to `~/.config/claude/claude_desktop_config.json` (Linux) or `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS):

```json
{
  "mcpServers": {
    "4da": {
      "command": "node",
      "args": ["/absolute/path/to/mcp-4da-server/dist/index.js"],
      "env": {
        "FOURDA_DB_PATH": "/absolute/path/to/4da-v3/data/4da.db"
      }
    }
  }
}
```

### Cursor Integration

Add to `.cursor/mcp.json` in your project:

```json
{
  "mcpServers": {
    "4da": {
      "command": "node",
      "args": ["/absolute/path/to/mcp-4da-server/dist/index.js"],
      "env": {
        "FOURDA_DB_PATH": "/absolute/path/to/4da-v3/data/4da.db"
      }
    }
  }
}
```

## Available Tools

### get_relevant_content

Get filtered relevant content from 4DA's personalized feed.

**Parameters:**
- `min_score` (number, optional): Minimum relevance score (0.0-1.0). Default: 0.35
- `source_type` (string, optional): Filter by source ("hackernews", "arxiv", "reddit")
- `limit` (number, optional): Maximum items to return. Default: 20
- `since_hours` (number, optional): Only items from last N hours. Default: 24

**Example:**
```json
{
  "min_score": 0.4,
  "source_type": "hackernews",
  "limit": 10,
  "since_hours": 12
}
```

**Response:**
```json
[
  {
    "id": 123,
    "source_type": "hackernews",
    "source_id": "38901234",
    "url": "https://example.com/article",
    "title": "Interesting Article About Rust",
    "content": "Article summary...",
    "relevance_score": 0.72,
    "created_at": "2026-01-21 14:30:00",
    "discovered_ago": "3 hours ago"
  }
]
```

### get_context

Get the user's context - what 4DA knows about them.

**Parameters:**
- `include_ace` (boolean, optional): Include ACE-detected context. Default: true
- `include_learned` (boolean, optional): Include learned preferences. Default: true

**Example:**
```json
{
  "include_ace": true,
  "include_learned": true
}
```

**Response:**
```json
{
  "role": "Backend Developer",
  "tech_stack": ["Rust", "TypeScript", "Python"],
  "domains": ["distributed systems", "ML infrastructure"],
  "interests": [
    {"id": 1, "topic": "WebAssembly", "weight": 1.0, "source": "explicit"}
  ],
  "exclusions": ["cryptocurrency", "blockchain"],
  "ace": {
    "detected_tech": [
      {"name": "Tauri", "category": "framework", "confidence": 0.9, "source": "manifest"}
    ],
    "active_topics": [
      {"topic": "MCP servers", "weight": 0.8, "confidence": 0.7, "source": "file_content", "last_seen": "2026-01-21 14:00:00"}
    ]
  },
  "learned": {
    "topic_affinities": [
      {"topic": "systems programming", "affinity_score": 0.6, "confidence": 0.8, "positive_signals": 15, "negative_signals": 2, "total_exposures": 20}
    ],
    "anti_topics": [
      {"topic": "web3", "rejection_count": 5, "confidence": 0.7, "auto_detected": true, "user_confirmed": false}
    ]
  }
}
```

### explain_relevance

Explain why an item was considered relevant.

**Parameters:**
- `item_id` (number, required): The database ID of the item
- `source_type` (string, required): The source type ("hackernews", "arxiv", "reddit")

**Example:**
```json
{
  "item_id": 123,
  "source_type": "hackernews"
}
```

**Response:**
```json
{
  "item_id": 123,
  "source_type": "hackernews",
  "title": "Interesting Article About Rust",
  "score_breakdown": {
    "embedding_similarity": null,
    "static_match_score": 0.35,
    "ace_match_score": 0.15,
    "learned_affinity_score": 0.12,
    "anti_penalty": 0,
    "final_score": 0.62
  },
  "matching_context": {
    "matching_interests": ["Rust", "systems programming"],
    "matching_tech": ["Rust"],
    "matching_topics": ["performance"],
    "matching_affinities": ["low-level programming"]
  },
  "explanation": "Matches your interests: Rust, systems programming. Related to your tech stack: Rust. Relevant to recent work: performance."
}
```

### record_feedback

Record user feedback on an item for learning.

**Parameters:**
- `item_id` (number, required): The database ID of the item
- `source_type` (string, required): The source type
- `action` (string, required): The feedback action
  - `"click"`: User opened the item (moderate positive signal)
  - `"save"`: User bookmarked the item (strong positive signal)
  - `"dismiss"`: User dismissed the item (weak negative signal)
  - `"mark_irrelevant"`: User marked as not relevant (strong negative signal)

**Example:**
```json
{
  "item_id": 123,
  "source_type": "hackernews",
  "action": "click"
}
```

**Response:**
```json
{
  "success": true,
  "message": "Recorded click feedback for item 123",
  "interaction_id": 456
}
```

## Development

```bash
# Build
npm run build

# Watch mode
npm run dev

# Test with MCP Inspector
npm run inspect
```

## Testing

You can test the server using the MCP Inspector:

```bash
npx @modelcontextprotocol/inspector node dist/index.js
```

Or by sending JSON-RPC messages directly:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | node dist/index.js
```

## Architecture

```
mcp-4da-server/
├── src/
│   ├── index.ts           # MCP server entry point
│   ├── db.ts              # SQLite database accessor
│   ├── types.ts           # TypeScript type definitions
│   └── tools/
│       ├── index.ts       # Tool exports
│       ├── get-relevant-content.ts
│       ├── get-context.ts
│       ├── explain-relevance.ts
│       └── record-feedback.ts
├── package.json
├── tsconfig.json
└── README.md
```

## Dependencies

- `@modelcontextprotocol/sdk` - MCP SDK for server implementation
- `better-sqlite3` - SQLite database access
- `typescript` - TypeScript compiler

## License

MIT
