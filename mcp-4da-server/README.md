# @4da/mcp-server

[![npm version](https://img.shields.io/npm/v/@4da/mcp-server?color=gold)](https://www.npmjs.com/package/@4da/mcp-server)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Node](https://img.shields.io/badge/Node-%3E%3D18-brightgreen)](https://nodejs.org/)
[![Tools](https://img.shields.io/badge/MCP%20Tools-30-white)](https://4da.ai)

Your AI coding assistant doesn't know what you're working on. It doesn't know your tech stack, your dependencies, or that the library you're debating has a critical CVE from yesterday. It writes code in a vacuum.

This MCP server fixes that. It connects your AI tools to a local intelligence engine that scans your actual codebase — your `Cargo.toml`, `package.json`, `go.mod` — and continuously scores content from 11 sources (Hacker News, arXiv, Reddit, GitHub, and more) against what you actually build with. 30 tools. Everything stays on your machine.

```
You:     "Are there any security issues in my dependencies?"
Claude:  [calls knowledge_gaps, project_health, get_actionable_signals]
         "Yes — the `serde` crate you use in 3 projects has a new advisory
          (RUSTSEC-2026-0012). Here's the migration path..."
```

## How It Works

[4DA](https://4da.ai) is a desktop app that runs quietly in the background. It scans your projects, watches your Git activity, and scores every piece of incoming content across 5 independent axes:

| Axis | Signal |
|------|--------|
| **Context** | Semantic similarity to your active codebase |
| **Interest** | Alignment with your declared and learned topics |
| **ACE** | Real-time signals from your Git commits and file edits |
| **Dependency** | Direct matches against your installed packages |
| **Learned** | Behavioral patterns from your save/dismiss feedback |

An item needs 2+ independent signals to pass the confirmation gate. Typical rejection rate: **99%+**. What survives is genuinely relevant to you.

This MCP server exposes that intelligence to any AI tool that speaks MCP.

## Setup

### 1. Install the intelligence engine

Download [4DA](https://4da.ai) for your platform (Windows, macOS, Linux). Open it, point it at your project directories, and let it run its first scan. Takes about 3 minutes.

### 2. Add the MCP server to your editor

**One command** (auto-detects Claude Code, Cursor, VS Code):
```bash
npx @4da/mcp-server --setup
```

Or manually:

<details>
<summary>Claude Code</summary>

```bash
claude mcp add 4da -- npx @4da/mcp-server
```
</details>

<details>
<summary>Cursor / Windsurf</summary>

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
</details>

<details>
<summary>VS Code (Copilot)</summary>

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
</details>

<details>
<summary>Claude Desktop</summary>

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):
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
</details>

### 3. Ask your AI anything

```
"What's relevant to my current project?"
"Any breaking changes in my dependencies?"
"Give me today's briefing"
"Why was this Rust async article scored so high for me?"
"What knowledge gaps do I have?"
"Export my Developer DNA"
```

Your AI calls the right tools automatically. No configuration needed — 4DA already knows your stack.

## Tools (30)

### Core

| Tool | What it does |
|------|-------------|
| `get_relevant_content` | Your filtered feed — only items that passed the 5-axis scoring gate |
| `get_context` | What 4DA knows about you: stack, interests, learned affinities, ACE-detected topics |
| `explain_relevance` | Why a specific item scored the way it did — full axis breakdown |
| `record_feedback` | Teach 4DA what matters — save, dismiss, or mark items irrelevant |

### Intelligence

| Tool | What it does |
|------|-------------|
| `daily_briefing` | AI-generated executive summary of your discoveries |
| `get_actionable_signals` | Classified alerts: security advisories, breaking changes, new tools, trending repos |
| `score_autopsy` | Deep forensic analysis of how any item's score was computed |
| `trend_analysis` | Statistical patterns, anomalies, and predictions across your feed |
| `context_analysis` | Recommendations to sharpen your context for better scoring |
| `topic_connections` | Knowledge graph showing how your content topics relate |
| `signal_chains` | Causal chains connecting related events across sources over time |
| `semantic_shifts` | Detects when topics you follow are changing in meaning or sentiment |
| `attention_report` | Where you spend attention vs. where your codebase needs it |

### Diagnostic

| Tool | What it does |
|------|-------------|
| `source_health` | Diagnose source fetching and data quality issues |
| `config_validator` | Validate configuration and detect issues |
| `llm_status` | Check LLM/Ollama configuration and availability |

### Knowledge & Health

| Tool | What it does |
|------|-------------|
| `knowledge_gaps` | Blind spots — dependencies you use but never read about |
| `project_health` | Dependency freshness, security advisories, update urgency |
| `reverse_mentions` | Where your projects are being discussed across monitored sources |
| `export_context_packet` | Portable snapshot of your context for session handoff |

### Decision Intelligence

| Tool | What it does |
|------|-------------|
| `decision_memory` | Record, query, and enforce architectural decisions across sessions |
| `tech_radar` | Technology adoption signals derived from your decisions + content trends |
| `check_decision_alignment` | Check if a proposed change aligns with your recorded decisions |

### Agent Autonomy

| Tool | What it does |
|------|-------------|
| `agent_memory` | Persistent memory that survives across sessions and agents |
| `agent_session_brief` | Tailored startup context so agents don't start cold |
| `delegation_score` | Should the agent proceed autonomously or ask you? |

### Developer DNA

| Tool | What it does |
|------|-------------|
| `developer_dna` | Your tech identity — primary stack, dependencies, engagement patterns, blind spots |

### Intelligence Metabolism

| Tool | What it does |
|------|-------------|
| `autophagy_status` | Intelligence metabolism status — autophagy cycles, calibration accuracy, anti-patterns |
| `decision_windows` | Time-bounded decision opportunities requiring your attention |
| `compound_advantage` | Compound advantage score — measures intelligence leverage for decisions |

## Transports

**stdio** (default) — works with all MCP hosts:
```bash
npx @4da/mcp-server
```

**Streamable HTTP** — for remote or multi-client setups:
```bash
npx @4da/mcp-server --http --port 4840
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FOURDA_DB_PATH` | Path to 4DA's SQLite database | Auto-detected from standard install locations |

## What Makes This Different

Most MCP servers connect your AI to a cloud API. This one connects it to **you** — your local codebase, your dependencies, your Git history, your architectural decisions. Nothing leaves your machine. The AI gets smarter about your work without any data going anywhere.

30 tools is not typical. Most MCP servers expose 1-5 endpoints. This is a full intelligence layer — from raw content scoring to decision enforcement to knowledge gap detection. It's not a wrapper around someone else's API. It's a read layer on top of a scoring engine that rejects 99% of everything it sees, so what your AI gets is what actually matters to you.

## Build from Source

```bash
cd mcp-4da-server
pnpm install
pnpm build
```

## Test

```bash
pnpm test                # Contract tests (71 tests)
pnpm run inspect         # MCP Inspector (interactive browser UI)
```

## Versioning

v4.0.0 is the first real release. Earlier versions on npm were pre-release stubs. We couldn't reclaim v1.0.0 due to npm's immutability policy, so we jumped past them.

## License

MIT — use it anywhere, integrate it with anything.

---

Built by [4DA](https://4da.ai) — privacy-first developer intelligence. All signal. No feed.
