# Awesome List PR Drafts

## 1. tauri-apps/awesome-tauri

**Target repo:** https://github.com/tauri-apps/awesome-tauri
**Section:** Applications (or Productivity, depending on current categories)

**Line to add:**
```markdown
- [4DA](https://4da.ai) - Developer intelligence desktop app that scores content from HN, arXiv, Reddit, and GitHub against your codebase. Rust backend with SQLite + sqlite-vec, React frontend, 1,600+ tests.
```

**PR Title:**
```
Add 4DA — developer intelligence desktop app
```

**PR Body:**
```markdown
## What

Adds [4DA](https://4da.ai) to the Applications section.

## About

4DA is a developer intelligence desktop app built with Tauri 2.0. It surfaces developer-relevant content from the internet, scored against the user's actual codebase.

**Tech stack:**
- Rust backend (54 modules, 1,600+ tests)
- React + TypeScript frontend (880+ tests)
- SQLite + sqlite-vec for local vector search
- Local embeddings via Ollama
- MCP server with 30 tools (MIT, on npm)

**Privacy-first:** All data stays on the user's machine. No cloud dependencies, no telemetry.

- Website: https://4da.ai
- MCP Server: https://www.npmjs.com/package/@4da/mcp-server
```

---

## 2. appcypher/awesome-mcp-servers

**Target repo:** https://github.com/appcypher/awesome-mcp-servers
**Section:** Developer Tools

**Line to add:**
```markdown
- [@4da/mcp-server](https://www.npmjs.com/package/@4da/mcp-server) - 30 MCP tools for codebase-aware developer intelligence. Scores HN, arXiv, Reddit, GitHub content against your tech stack. Local-only, privacy-first. `npx @4da/mcp-server`
```

**PR Title:**
```
Add @4da/mcp-server — 30-tool developer intelligence server
```

---

## 3. jamesmurdza/awesome-ai-devtools

**Target repo:** https://github.com/jamesmurdza/awesome-ai-devtools
**Section:** AI-Powered Developer Tools (or equivalent)

**Line to add:**
```markdown
- [4DA](https://4da.ai) - Desktop app + [MCP server](https://www.npmjs.com/package/@4da/mcp-server) that gives AI coding assistants awareness of your tech stack, dependencies, and decisions. Scores content from HN, arXiv, Reddit, GitHub against your codebase. Local-only.
```

**PR Title:**
```
Add 4DA — codebase-aware developer intelligence
```

---

## 4. punkpeye/awesome-mcp-servers

Already drafted in `mcp-4da-server/AWESOME_LIST_PR.md`. Ready to submit.

---

## Submission Order

1. punkpeye/awesome-mcp-servers (highest stars, auto-syncs to Glama)
2. appcypher/awesome-mcp-servers (accepts PRs, active)
3. tauri-apps/awesome-tauri (niche but high-quality audience)
4. jamesmurdza/awesome-ai-devtools (broader AI tools audience)
