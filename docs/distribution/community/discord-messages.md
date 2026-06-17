# Discord Messages — 4DA Shadow Drop

## 1. Tauri Discord #showcase

**4DA — Developer intelligence desktop app built on Tauri 2.0**

4DA is a local-first desktop app that scores content from Hacker News, arXiv, Reddit, GitHub, and RSS feeds against your actual codebase to surface what's relevant. Built with a Rust backend, React/TypeScript frontend, and SQLite with sqlite-vec for local vector search. No 4DA server; content sources and any cloud AI you enable are contacted directly from your machine. The Rust side has 1,600+ tests covering source adapters, scoring, embeddings, and the full content pipeline. Tauri 2.0 made the IPC layer clean enough that the entire app runs as a single binary with zero external dependencies. More at 4da.ai.

---

## 2. MCP Community Discord

**@4da/mcp-server — 14 MCP tools for codebase-aware developer intelligence**

Most MCP servers wrap a single API or service. @4da/mcp-server is different — it exposes a full intelligence engine that connects your codebase context to the outside world. 14 tools across 9 categories: signal analysis, trend detection, knowledge gaps, tech radar, decision tracking, source health monitoring, and more. The intelligence tools read a local SQLite database; the live tools query public sources (OSV.dev, package registries) with only package names and versions — no API keys required for the MCP layer, and `FOURDA_OFFLINE=true` disables network access. Install in one command:

```
npx @4da/mcp-server --setup
```

Apache-2.0 licensed. Works with Claude Code, Cursor, Windsurf, or any MCP client. Details at 4da.ai.

---

## 3. MCP Contributors Discord

**Architecture of @4da/mcp-server — 14 tools, local-only, zero network deps**

Sharing the architecture behind @4da/mcp-server in case it's useful for others building non-trivial MCP servers. The server is TypeScript with a schema registry pattern — each tool declares its input schema, description, and handler in a self-contained module, and a central dispatcher routes calls. State lives in a local SQLite database via better-sqlite3 (no ORM, raw queries, WAL mode). The DB-backed tools read only from that local database; the live tools (vulnerability scanning, dependency health, ecosystem news) fetch from public sources like OSV.dev and the package registries, sending only package names and versions, with a token-bucket rate limiter and on-disk caching — and `FOURDA_OFFLINE=true` disables all outbound calls for a fully local run. Curious how others are handling tool discovery and schema validation in larger MCP servers — we went with a static registry over dynamic registration but there are tradeoffs.

```
npx @4da/mcp-server --setup
```

Source and docs at 4da.ai.
