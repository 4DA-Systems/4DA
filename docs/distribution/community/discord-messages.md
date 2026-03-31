# Discord Messages — 4DA Shadow Drop

## 1. Tauri Discord #showcase

**4DA — Developer intelligence desktop app built on Tauri 2.0**

4DA is a local-first desktop app that scores content from Hacker News, arXiv, Reddit, GitHub, and RSS feeds against your actual codebase to surface what's relevant. Built with a Rust backend, React/TypeScript frontend, and SQLite with sqlite-vec for local vector search — no data leaves your machine. The Rust side has 1,600+ tests covering source adapters, scoring, embeddings, and the full content pipeline. Tauri 2.0 made the IPC layer clean enough that the entire app runs as a single binary with zero external dependencies. More at 4da.ai.

---

## 2. MCP Community Discord

**@4da/mcp-server — 33 MCP tools for codebase-aware developer intelligence**

Most MCP servers wrap a single API or service. @4da/mcp-server is different — it exposes a full intelligence engine that connects your codebase context to the outside world. 33 tools across 8 categories: signal analysis, trend detection, knowledge gaps, tech radar, decision tracking, source health monitoring, and more. Everything runs locally against a SQLite database — zero network dependencies, no API keys required for the MCP layer itself. Install in one command:

```
npx @4da/mcp-server --setup
```

MIT licensed. Works with Claude Code, Cursor, Windsurf, or any MCP client. Details at 4da.ai.

---

## 3. MCP Contributors Discord

**Architecture of @4da/mcp-server — 33 tools, local-only, zero network deps**

Sharing the architecture behind @4da/mcp-server in case it's useful for others building non-trivial MCP servers. The server is TypeScript with a schema registry pattern — each tool declares its input schema, description, and handler in a self-contained module, and a central dispatcher routes calls. State lives in a local SQLite database via better-sqlite3 (no ORM, raw queries, WAL mode). The deliberate choice was zero network dependencies at the MCP layer — the server never makes outbound requests, it only reads from the local database that the main 4DA app populates. This means the MCP server starts instantly and works offline. Curious how others are handling tool discovery and schema validation in larger MCP servers — we went with a static registry over dynamic registration but there are tradeoffs.

```
npx @4da/mcp-server --setup
```

Source and docs at 4da.ai.
