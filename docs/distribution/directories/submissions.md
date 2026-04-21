# @4da/mcp-server — Directory Submission Copy

Comprehensive, ready-to-paste copy for every MCP directory and listing platform.

**Package:** [@4da/mcp-server](https://www.npmjs.com/package/@4da/mcp-server) v4.1.1
**Last updated:** 2026-04-21

---

## 1. Short Description (<100 chars)

```
36 MCP tools for codebase-aware developer intelligence. Local-only, privacy-first.
```

Alternative (more specific):
```
MCP server that scores HN, arXiv, Reddit, GitHub content against your actual tech stack.
```

---

## 2. Medium Description (2-3 sentences)

```
36 MCP tools that give AI coding assistants awareness of your tech stack, dependencies, and architectural decisions. Scores content from Hacker News, arXiv, Reddit, GitHub, and 7 other sources against what you actually build — then surfaces security advisories, breaking changes, knowledge gaps, and decision conflicts. Everything runs locally. Nothing leaves your machine.
```

---

## 3. Long Description (1 paragraph)

```
@4da/mcp-server is an MCP server that connects Claude Code, Cursor, Copilot, and Windsurf to a local developer intelligence engine. It provides 36 tools across 9 categories: content scoring, developer context, intelligence analysis, decision memory, knowledge gap detection, tech radar, agent autonomy, and diagnostic health. The server reads your Cargo.toml, package.json, go.mod, and requirements.txt — then scores articles, papers, and discussions from Hacker News, arXiv, Reddit, GitHub Trending, RSS feeds, and more against your actual codebase using a 5-axis relevance algorithm (semantic similarity, interest alignment, real-time Git signals, dependency matching, and learned behavior). It also provides persistent decision memory so your AI assistant can check whether a proposed change aligns with your recorded architectural decisions, detect knowledge blind spots in dependencies you use daily but never read about, and generate daily briefings of only the content that matters to your work. All data stays on your machine — zero network calls, no telemetry, MIT licensed.
```

---

## 4. Feature Bullet Points (6-8 bullets)

```
- 36 MCP tools across 9 categories — content scoring, decision memory, knowledge gaps, tech radar, agent autonomy, and more
- Codebase-aware — auto-discovers your tech stack from package.json, Cargo.toml, go.mod, requirements.txt, and lockfiles
- 5-axis relevance scoring — semantic similarity, interest alignment, real-time Git signals, dependency matching, and learned behavior
- Decision memory — record, query, and enforce architectural decisions across AI sessions and editors
- Privacy-first — reads from a local SQLite database, zero network calls, no telemetry, no cloud dependencies
- Works with Claude Code, Cursor, Windsurf, VS Code (Copilot), and Claude Desktop
- One-command setup — `npx @4da/mcp-server --setup` auto-detects your editors and writes config
- MIT licensed — use it anywhere, fork it, integrate it, build on it
```

---

## 5. Tags / Keywords

```
mcp, mcp-server, model-context-protocol, claude, claude-code, claude-desktop, cursor, windsurf, copilot, vscode, ai-tools, ai-agent, ai-coding, developer-tools, developer-intelligence, developer-productivity, developer-context, codebase-awareness, codebase-context, code-context, code-intelligence, content-scoring, relevance-scoring, privacy-first, local-first, offline, hacker-news, arxiv, reddit, github, knowledge-gaps, tech-radar, decision-memory, dependency-tracking, security-advisories, agent-memory, sqlite, tauri, typescript, content-curation, daily-briefing, signal-chains, semantic-analysis
```

---

## 6. Platform-Specific Submission Copy

---

### 6.1 Official MCP Registry (server.json)

**Status:** The official registry uses `mcp-publisher` CLI and requires a `server.json` file or `mcpName` in `package.json`. See `official-registry.md` for step-by-step publishing instructions.

**server.json format notes:**
- The registry pulls metadata from your npm package
- `mcpName` in package.json should be `@4da/mcp-server`
- The `server.json` schema is defined by `mcp-publisher init`
- Must be published to npm first (already done)
- Auth is via GitHub OAuth

---

### 6.2 punkpeye/awesome-mcp-servers

**Status:** PR draft already prepared in `AWESOME_LIST_PR.md`. Ready to submit.

**Line to add** (Developer Tools section, alphabetical):
```markdown
- [runyourempire/4DA](https://github.com/runyourempire/4DA/tree/main/mcp-4da-server) 📇 🏠 🍎 🪟 🐧 - 36 MCP tools that score content from Hacker News, arXiv, Reddit, and GitHub against your actual codebase. Includes developer context profiling, decision memory, knowledge gap detection, tech radar, and agent session handoff. Privacy-first — everything stays local. `npx @4da/mcp-server`
```

**PR Title:**
```
Add @4da/mcp-server — developer intelligence with codebase-aware content scoring
```

**Notes:**
- Badges: 📇 (TypeScript), 🏠 (Local Service), 🍎 (macOS), 🪟 (Windows), 🐧 (Linux)
- Insert alphabetically between `rsdouglas/janee` and `ryan0204/github-repo-mcp`
- Full PR body in `mcp-4da-server/AWESOME_LIST_PR.md`

---

### 6.3 mcp.so

**Format:** GitHub issue on [mcp-get/mcp-get](https://github.com/mcp-get/mcp-get)

**Issue Title:**
```
Add @4da/mcp-server — 36 tools for codebase-aware developer intelligence
```

**Issue Body:**
```markdown
## Server Information

- **Name:** @4da/mcp-server
- **npm:** https://www.npmjs.com/package/@4da/mcp-server
- **Repository:** https://github.com/runyourempire/4DA/tree/main/mcp-4da-server
- **Website:** https://4da.ai
- **License:** MIT
- **Language:** TypeScript
- **Install:** `npx @4da/mcp-server`

## Description

36 MCP tools that give AI coding assistants awareness of your tech stack, dependencies, and architectural decisions. Scores content from Hacker News, arXiv, Reddit, GitHub, and 7 other sources against what you actually build — then surfaces security advisories, breaking changes, knowledge gaps, and decision conflicts. Everything runs locally. Nothing leaves your machine.

## Categories

- Developer Tools
- AI/ML
- Privacy

## Tool Count

36 tools across 9 categories:
- Core (4): content feed, context profile, relevance explanation, feedback
- Intelligence (9): daily briefing, actionable signals, score autopsy, trend analysis, context analysis, topic connections, signal chains, semantic shifts, attention report
- Decision Intelligence (3): decision memory, tech radar, decision alignment
- Knowledge & Health (4): knowledge gaps, project health, reverse mentions, context export
- Agent Autonomy (3): persistent memory, session briefs, delegation scoring
- Developer Identity (1): developer DNA
- Intelligence Metabolism (3): autophagy status, decision windows, compound advantage
- Diagnostic (3): source health, config validation, LLM status

## Key Features

- Codebase-aware — reads package.json, Cargo.toml, go.mod, requirements.txt
- 5-axis relevance scoring (semantic, interest, Git signals, dependency, learned behavior)
- Privacy-first — local SQLite, zero network calls, no telemetry
- Works with Claude Code, Cursor, Windsurf, Copilot, Claude Desktop
- One-command setup: `npx @4da/mcp-server --setup`
- MIT licensed
```

---

### 6.4 PulseMCP

**URL:** https://www.pulsemcp.com/submit

**Form Fields:**

| Field | Value |
|-------|-------|
| Server Name | @4da/mcp-server |
| URL | https://www.npmjs.com/package/@4da/mcp-server |
| Website | https://4da.ai |
| Short Description | 36 MCP tools for codebase-aware developer intelligence. Scores content from HN, arXiv, Reddit, GitHub against your actual tech stack. Local-only, privacy-first. |
| Category | Developer Tools |
| Language | TypeScript |
| License | MIT |
| Install Command | npx @4da/mcp-server --setup |
| GitHub | https://github.com/runyourempire/4DA/tree/main/mcp-4da-server |

---

### 6.5 mcpservers.org

**URL:** https://mcpservers.org/submit

**Form Fields:**

| Field | Value |
|-------|-------|
| Name | @4da/mcp-server |
| Description | 36 MCP tools that give AI coding assistants awareness of your tech stack, dependencies, and architectural decisions. Scores content from Hacker News, arXiv, Reddit, GitHub, and 7 other sources against what you actually build. Privacy-first — everything runs locally. |
| npm Package | @4da/mcp-server |
| GitHub Repository | https://github.com/runyourempire/4DA/tree/main/mcp-4da-server |
| Website | https://4da.ai |
| Category | Developer Tools |
| Tags | mcp, developer-tools, content-scoring, privacy-first, local-first, ai-tools, codebase-awareness, decision-memory |
| License | MIT |
| Language | TypeScript |
| Platforms | Windows, macOS, Linux |

---

### 6.6 MCPMarket

**URL:** https://mcpmarket.com/submit

**Form Fields:**

| Field | Value |
|-------|-------|
| Server Name | @4da/mcp-server |
| One-line Description | 36 MCP tools for codebase-aware developer intelligence. Local-only, privacy-first. |
| Full Description | 36 MCP tools that give AI coding assistants awareness of your tech stack, dependencies, and architectural decisions. Scores content from Hacker News, arXiv, Reddit, GitHub, and 7 other sources against what you actually build — then surfaces security advisories, breaking changes, knowledge gaps, and decision conflicts. Everything runs locally. Nothing leaves your machine. |
| npm URL | https://www.npmjs.com/package/@4da/mcp-server |
| GitHub URL | https://github.com/runyourempire/4DA/tree/main/mcp-4da-server |
| Website | https://4da.ai |
| Install Command | npx @4da/mcp-server --setup |
| Category | Developer Tools |
| Tags | developer-intelligence, content-scoring, privacy-first, codebase-awareness, decision-memory, knowledge-gaps |
| License | MIT |
| Tool Count | 36 |

---

### 6.7 Cline Marketplace

**Format:** GitHub issue on [cline/cline](https://github.com/cline/cline) — MCP Server submission template

**Important:** Requires a 400x400 PNG logo. Prepare logo at `docs/distribution/assets/4da-logo-400x400.png` before submitting.

**Issue Title:**
```
[MCP Server] @4da/mcp-server — 36 tools for codebase-aware developer intelligence
```

**Issue Body:**
```markdown
## MCP Server Submission

### Server Name
@4da/mcp-server

### Description
36 MCP tools that give AI coding assistants awareness of your tech stack, dependencies, and architectural decisions. Scores content from Hacker News, arXiv, Reddit, GitHub against what you actually build. Privacy-first — everything runs locally.

### npm Package
https://www.npmjs.com/package/@4da/mcp-server

### GitHub Repository
https://github.com/runyourempire/4DA/tree/main/mcp-4da-server

### Website
https://4da.ai

### Install Command
```
npx @4da/mcp-server --setup
```

### MCP Configuration
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

### Category
Developer Tools

### License
MIT

### Logo
[Attach 400x400 PNG logo]

### Tool Count
30

### Key Features
- Codebase-aware — reads package.json, Cargo.toml, go.mod, requirements.txt
- 5-axis relevance scoring (semantic, interest, Git signals, dependency, learned behavior)
- Decision memory — record and enforce architectural decisions across sessions
- Knowledge gap detection — finds blind spots in your dependency knowledge
- Daily briefings — AI-generated summaries of relevant discoveries
- Agent autonomy — session briefs, context packets, delegation scoring
- Privacy-first — local SQLite, zero network calls, no telemetry
- Works with Claude Code, Cursor, Windsurf, Copilot, Claude Desktop
```

---

### 6.8 LobeHub

**Format:** GitHub issue or PR on [lobehub/lobe-chat-plugins](https://github.com/lobehub/lobe-chat-plugins) or submit at https://chat-plugins.lobehub.com/submit

**Submission format (plugin manifest style):**

```json
{
  "identifier": "4da-mcp-server",
  "meta": {
    "title": "@4da/mcp-server",
    "description": "36 MCP tools for codebase-aware developer intelligence. Scores content from HN, arXiv, Reddit, GitHub against your actual tech stack. Local-only, privacy-first.",
    "tags": ["developer-tools", "content-scoring", "privacy", "codebase-awareness", "mcp"],
    "avatar": "https://4da.ai/logo.png",
    "homepage": "https://4da.ai"
  },
  "author": "4DA",
  "type": "mcp",
  "install": {
    "command": "npx",
    "args": ["@4da/mcp-server"]
  }
}
```

**Notes:**
- LobeHub may use their own plugin format; adapt the above to match their current schema
- Check https://github.com/lobehub/lobe-chat-plugins/blob/main/CONTRIBUTING.md for latest format
- Logo/avatar should be hosted at a public URL (use 4da.ai or npm CDN)

---

### 6.9 MCPServerFinder

**URL:** https://mcpserverfinder.com/submit (or similar)

**Form Fields:**

| Field | Value |
|-------|-------|
| Server Name | @4da/mcp-server |
| Short Description | 36 MCP tools for codebase-aware developer intelligence. Local-only, privacy-first. |
| Full Description | 36 MCP tools that give AI coding assistants awareness of your tech stack, dependencies, and architectural decisions. Scores content from Hacker News, arXiv, Reddit, GitHub, and 7 other sources against what you actually build — then surfaces security advisories, breaking changes, knowledge gaps, and decision conflicts. Everything runs locally. Nothing leaves your machine. |
| npm URL | https://www.npmjs.com/package/@4da/mcp-server |
| GitHub URL | https://github.com/runyourempire/4DA/tree/main/mcp-4da-server |
| Website | https://4da.ai |
| Install | npx @4da/mcp-server --setup |
| Category | Developer Tools |
| Tags | mcp, developer-intelligence, content-scoring, privacy-first, local-first, codebase-awareness |
| License | MIT |
| Platforms | Windows, macOS, Linux |

---

### 6.10 mcpserverdirectory.org

**URL:** https://mcpserverdirectory.org/submit

**Form Fields:**

| Field | Value |
|-------|-------|
| Name | @4da/mcp-server |
| Description | 36 MCP tools that give AI coding assistants awareness of your tech stack, dependencies, and architectural decisions. Scores content from Hacker News, arXiv, Reddit, GitHub, and 7 other sources against what you actually build. Privacy-first — everything runs locally, zero network calls, no telemetry. |
| npm | https://www.npmjs.com/package/@4da/mcp-server |
| Repository | https://github.com/runyourempire/4DA/tree/main/mcp-4da-server |
| Homepage | https://4da.ai |
| Install Command | npx @4da/mcp-server --setup |
| Category | Developer Tools |
| License | MIT |
| Language | TypeScript |
| Runtime | Node.js >= 18 |

---

### 6.11 mcp-servers-hub.net

**URL:** https://mcp-servers-hub.net/submit

**Form Fields:**

| Field | Value |
|-------|-------|
| Server Name | @4da/mcp-server |
| One-line Description | 36 MCP tools for codebase-aware developer intelligence. Local-only, privacy-first. |
| Description | 36 MCP tools that give AI coding assistants awareness of your tech stack, dependencies, and architectural decisions. Scores content from Hacker News, arXiv, Reddit, GitHub, and 7 other sources against what you actually build — then surfaces security advisories, breaking changes, knowledge gaps, and decision conflicts. Everything runs locally. Nothing leaves your machine. Works with Claude Code, Cursor, Windsurf, VS Code (Copilot), and Claude Desktop. |
| npm Package | @4da/mcp-server |
| GitHub | https://github.com/runyourempire/4DA/tree/main/mcp-4da-server |
| Website | https://4da.ai |
| Install | npx @4da/mcp-server --setup |
| Category | Developer Tools |
| Tags | developer-intelligence, content-scoring, privacy-first, local-first, codebase-awareness, decision-memory |
| License | MIT |
| Tool Count | 36 |

---

### 6.12 DevHunt

**URL:** https://devhunt.org/submit

**Form Fields:**

| Field | Value |
|-------|-------|
| Name | @4da/mcp-server |
| Tagline | 36 MCP tools for codebase-aware developer intelligence |
| Description | An MCP server that connects Claude Code, Cursor, Copilot, and Windsurf to a local developer intelligence engine. Provides 36 tools that score content from Hacker News, arXiv, Reddit, GitHub against your actual tech stack — surfacing security advisories, breaking changes, knowledge gaps, and decision conflicts. Everything runs locally. Nothing leaves your machine. MIT licensed. |
| Website | https://4da.ai |
| GitHub | https://github.com/runyourempire/4DA/tree/main/mcp-4da-server |
| Tags | developer-tools, mcp, ai-tools, privacy, open-source |
| Launch Date | (set to submission date) |

**Notes:**
- DevHunt works as a daily/weekly leaderboard — time your submission for maximum visibility
- Requires GitHub authentication
- Consider submitting on a weekday morning (US time) for best engagement

---

### 6.13 awesome-tauri

**Format:** PR on [tauri-apps/awesome-tauri](https://github.com/tauri-apps/awesome-tauri)

**Line to add** (under "Plugins" or "Utilities" or most relevant section):
```markdown
- [4DA](https://4da.ai) - Privacy-first developer intelligence desktop app built with Tauri 2.0. Includes an MCP server with 36 tools for codebase-aware content scoring, decision memory, and knowledge gap detection.
```

**PR Title:**
```
Add 4DA — developer intelligence with MCP server (Tauri 2.0)
```

**PR Body:**
```markdown
Adds [4DA](https://4da.ai) to the list.

4DA is a Tauri 2.0 desktop app (Rust + React + TypeScript + SQLite) that surfaces developer-relevant content from the internet — privately, locally, with zero configuration. It includes an npm-published MCP server (`@4da/mcp-server`) with 36 tools for AI coding assistants.

- **Stack:** Rust backend, React/TypeScript frontend, SQLite + sqlite-vec
- **MCP Server:** https://www.npmjs.com/package/@4da/mcp-server
- **License:** App is FSL-1.1-Apache-2.0, MCP server is MIT
- **Platforms:** Windows, macOS, Linux
```

---

### 6.14 appcypher/awesome-mcp-servers

**Format:** PR on [appcypher/awesome-mcp-servers](https://github.com/appcypher/awesome-mcp-servers)

**Line to add** (under "Developer Tools" section):
```markdown
- [@4da/mcp-server](https://www.npmjs.com/package/@4da/mcp-server) - 36 tools for codebase-aware developer intelligence. Scores HN, arXiv, Reddit, GitHub content against your tech stack. Decision memory, knowledge gaps, tech radar. Local-only, privacy-first.
```

**PR Title:**
```
Add @4da/mcp-server — codebase-aware developer intelligence (36 tools)
```

**PR Body:**
```markdown
## What

Adds [@4da/mcp-server](https://www.npmjs.com/package/@4da/mcp-server) to the Developer Tools section.

## About

36 MCP tools that give AI coding assistants awareness of your tech stack, dependencies, and architectural decisions. Reads your project files (package.json, Cargo.toml, go.mod, etc.) and scores content from 10+ sources against what you actually build.

- **npm:** https://www.npmjs.com/package/@4da/mcp-server
- **Install:** `npx @4da/mcp-server`
- **License:** MIT
- **Language:** TypeScript
- **Privacy-first:** Local SQLite, zero network calls, no telemetry
```

---

### 6.15 jamesmurdza/awesome-ai-devtools

**Format:** PR on [jamesmurdza/awesome-ai-devtools](https://github.com/jamesmurdza/awesome-ai-devtools)

**Line to add** (under "MCP Servers" or "Context Providers" or most relevant section):
```markdown
- [@4da/mcp-server](https://www.npmjs.com/package/@4da/mcp-server) - 36 MCP tools for codebase-aware developer intelligence. Scores content from HN, arXiv, Reddit, GitHub against your actual tech stack. Includes decision memory, knowledge gap detection, and agent session handoff. Privacy-first, local-only.
```

**PR Title:**
```
Add @4da/mcp-server — MCP server for codebase-aware developer intelligence
```

**PR Body:**
```markdown
## What

Adds [@4da/mcp-server](https://www.npmjs.com/package/@4da/mcp-server) — an MCP server providing 36 tools for developer intelligence.

## Key Capabilities

- **Content scoring** — Scores articles from Hacker News, arXiv, Reddit, GitHub against the user's actual project dependencies
- **Decision memory** — Records and enforces architectural decisions across AI sessions
- **Knowledge gaps** — Detects blind spots in dependency knowledge
- **Agent autonomy** — Session briefs, context packets, delegation scoring for agentic workflows
- **Privacy-first** — All data stays local, zero network calls, MIT licensed

## Links

- **npm:** https://www.npmjs.com/package/@4da/mcp-server
- **Website:** https://4da.ai
- **Install:** `npx @4da/mcp-server --setup`
```

---

## Submission Tracker

Use this to track progress across all directories:

| # | Directory | Status | URL | Notes |
|---|-----------|--------|-----|-------|
| 1 | Official MCP Registry | Not started | — | Use mcp-publisher CLI. See official-registry.md |
| 2 | punkpeye/awesome-mcp-servers | PR draft ready | — | AWESOME_LIST_PR.md has full PR copy |
| 3 | mcp.so (mcp-get) | Not started | — | GitHub issue format |
| 4 | PulseMCP | Not started | pulsemcp.com/submit | Form submission |
| 5 | mcpservers.org | Not started | mcpservers.org/submit | Form submission |
| 6 | MCPMarket | Not started | mcpmarket.com/submit | Form submission |
| 7 | Cline Marketplace | Not started | — | Needs 400x400 PNG logo |
| 8 | LobeHub | Not started | — | GitHub issue/PR or web form |
| 9 | MCPServerFinder | Not started | mcpserverfinder.com/submit | Form submission |
| 10 | mcpserverdirectory.org | Not started | mcpserverdirectory.org/submit | Form submission |
| 11 | mcp-servers-hub.net | Not started | mcp-servers-hub.net/submit | Form submission |
| 12 | DevHunt | Not started | devhunt.org/submit | Time for weekday morning |
| 13 | awesome-tauri | Not started | — | GitHub PR |
| 14 | appcypher/awesome-mcp-servers | Not started | — | GitHub PR |
| 15 | jamesmurdza/awesome-ai-devtools | Not started | — | GitHub PR |

---

## Pre-Submission Checklist

Before submitting to any directory:

- [ ] npm package is published and accessible: `npm view @4da/mcp-server`
- [ ] `npx @4da/mcp-server --setup` works on a clean machine
- [ ] `npx @4da/mcp-server --doctor` reports healthy
- [ ] README.md on npm is current and complete
- [ ] 400x400 PNG logo is ready (needed for Cline Marketplace)
- [ ] Public logo URL hosted on 4da.ai (needed for LobeHub and some directories)
- [ ] GitHub repo visibility decision made (currently private — some directories may reject or deprioritize)
