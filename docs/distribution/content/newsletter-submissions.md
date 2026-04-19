# Newsletter Submission Templates

## Console.dev
Submit at: https://console.dev
Selection criteria: https://console.dev/selection-criteria

**What it does:**
@4da/mcp-server is a 30-tool MCP server that connects AI coding assistants (Claude Code, Cursor, Copilot, Windsurf) to a developer's local codebase. It reads dependency files (Cargo.toml, package.json, go.mod, requirements.txt, pyproject.toml), builds a context profile, then scores content from Hacker News, arXiv, Reddit, GitHub, and RSS feeds against what the developer actually builds. Everything runs locally via SQLite. No cloud, no telemetry, no account required.

**Install:**
```
npx @4da/mcp-server --setup
```

**Links:**
- npm: https://www.npmjs.com/package/@4da/mcp-server
- Website: https://4da.ai
- License: MIT

---

## This Week in Rust
Submit via PR to: https://github.com/rust-lang/this-week-in-rust
Section: "Project/Tooling Updates"

**Suggested line:**
[4DA](https://4da.ai) — A Tauri 2.0 desktop application built in Rust that surfaces developer-relevant content from HN, arXiv, Reddit, and GitHub. Uses SQLite + sqlite-vec for local vector search, ocrs for PDF extraction, and ships with a 30-tool MCP server. 1,600+ Rust tests.

---

## Rust Bytes (Substack)
https://weeklyrust.substack.com/

**Pitch:**
4DA is a Tauri 2.0 desktop app written in Rust (~54 modules, 1,600+ tests) that scores internet content against a developer's actual codebase. It uses sqlite-vec for vector search, ocrs for PDF text extraction, and ships with an MIT-licensed MCP server (35 tools) on npm. The Rust backend handles content ingestion from 7 source types, local embeddings via Ollama, and a confidence-weighted scoring algorithm. Privacy-first: no data leaves the machine.

---

## TLDR Newsletter
Submit at: https://tldr.tech (varies by edition)

**For TLDR AI:**
@4da/mcp-server: 35 MCP tools that give AI coding assistants awareness of your tech stack. Reads your dependency files, scores HN/arXiv/Reddit/GitHub content against what you build, and provides decision memory, knowledge gap detection, and tech radar. Local-only, MIT licensed. `npx @4da/mcp-server --setup`

**For TLDR Web Dev:**
@4da/mcp-server: An MCP server with 35 tools that connects Claude Code, Cursor, and Copilot to your local codebase. It reads Cargo.toml, package.json, go.mod — then surfaces relevant content from HN, arXiv, and GitHub. No cloud dependencies. `npx @4da/mcp-server --setup`

**For TLDR DevOps:**
@4da/mcp-server: 30 developer intelligence tools via MCP. Monitors dependency security advisories, scores technical content against your stack, tracks knowledge gaps. Runs entirely local via SQLite, zero network dependencies. MIT licensed. `npx @4da/mcp-server --setup`

---

## Changelog News
Submit at: https://changelog.com/news/submit

**Title:** @4da/mcp-server — 35 MCP tools for codebase-aware developer intelligence

**Description:**
Most MCP servers wrap a single cloud API. This one ships 35 tools across 8 categories — content scoring, developer context profiling, decision memory, knowledge gap detection, tech radar, trend analysis, and agent session handoff. It reads your local dependency files, builds a context profile of your tech stack, then scores content from HN, arXiv, Reddit, and GitHub against what you actually build. Everything runs locally via SQLite. No cloud, no telemetry, MIT licensed.

**Install:** `npx @4da/mcp-server --setup`
**npm:** https://www.npmjs.com/package/@4da/mcp-server
