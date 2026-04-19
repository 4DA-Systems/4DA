# @4da/mcp-server

[![npm version](https://img.shields.io/npm/v/@4da/mcp-server?color=gold)](https://www.npmjs.com/package/@4da/mcp-server)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Node](https://img.shields.io/badge/Node-%3E%3D18-brightgreen)](https://nodejs.org/)
[![MCP Tools](https://img.shields.io/badge/MCP%20Tools-35-white)](https://4da.ai)
[![smithery badge](https://smithery.ai/badge/@4da/mcp-server)](https://smithery.ai/server/@4da/mcp-server)

**35 MCP tools that give your AI coding assistant memory, context, and awareness of what you actually build.**

Your AI writes code without knowing your tech stack, your dependencies, or that the library it just recommended has a critical CVE from yesterday. This MCP server fixes that. It connects Claude Code, Cursor, Copilot, and Windsurf to a local intelligence engine that reads your `Cargo.toml`, `package.json`, `go.mod`, and `requirements.txt` -- then scores content from Hacker News, arXiv, Reddit, GitHub, and 7 other sources against what matters to you.

Everything runs locally. Nothing leaves your machine.

```
You:     "Are there any security issues in my dependencies?"
Claude:  [calls project_health, knowledge_gaps, get_actionable_signals]
         "Yes — the serde crate you use in 3 projects has a new advisory
          (RUSTSEC-2026-0012). Here's the migration path..."

You:     "Give me my morning briefing"
Claude:  [calls daily_briefing]
         "3 high-signal items today: Tokio released 1.42 with a breaking change
          in your spawn pattern, a Hacker News post on sqlite-vec hit #1 (your
          primary vector store), and the RFC you bookmarked got merged..."

You:     "Should I switch from Axios to fetch for this project?"
Claude:  [calls check_decision_alignment, tech_radar, trend_analysis]
         "Your recorded decision AD-014 chose Axios for interceptor support.
          The trend data shows native fetch adoption is accelerating, but your
          codebase has 47 interceptor usages. Recommendation: keep Axios here,
          consider fetch for new greenfield projects..."
```

## Why This Exists

Most MCP servers expose 1-5 tools that wrap a cloud API. This is different:

- **35 tools** across 8 categories -- content scoring, decision memory, knowledge gaps, agent autonomy, and more
- **Codebase-aware** -- reads your actual project files, not just what you tell it
- **Privacy-first** -- reads from a local SQLite database, zero network calls, no telemetry
- **Works offline** -- the intelligence engine runs entirely on your machine with optional Ollama for local LLM
- **MIT licensed** -- use it anywhere, fork it, integrate it, build on it

## Quick Start

### 1. Install the 4DA desktop app

The MCP server reads from a local database that the [4DA desktop app](https://4da.ai) builds. Download it, point it at your project directories, and let it run its first scan (~3 minutes).

> **[Download 4DA](https://github.com/runyourempire/4DA/releases/latest)** -- Windows, macOS, Linux

### 2. Add the MCP server

**Auto-setup** (detects your editors and writes config):
```bash
npx @4da/mcp-server --setup
```

Or add it manually to your editor:

<details>
<summary><b>Claude Code</b></summary>

```bash
claude mcp add 4da -- npx @4da/mcp-server
```
</details>

<details>
<summary><b>Cursor / Windsurf</b></summary>

Add to `~/.cursor/mcp.json` or `~/.windsurf/mcp.json`:
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
<summary><b>VS Code (Copilot)</b></summary>

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
<summary><b>Claude Desktop</b></summary>

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

### 3. Start asking

No configuration needed -- 4DA already knows your stack:

```
"What's relevant to my current project?"
"Any breaking changes in my dependencies?"
"What are my knowledge gaps?"
"Why was this article scored so high?"
"Record a decision: we're using Postgres, not MySQL"
"Export my Developer DNA"
```

### 4. Verify installation

```bash
npx @4da/mcp-server --doctor
```

## How Scoring Works

[4DA](https://4da.ai) scores every piece of content across 5 independent axes. An item needs 2+ confirming signals to pass. Typical rejection rate: **99%+**.

| Axis | Signal |
|------|--------|
| **Context** | Semantic similarity to your active codebase via local embeddings |
| **Interest** | Alignment with your declared and inferred topics |
| **ACE** | Real-time signals from your Git commits and file edits |
| **Dependency** | Direct matches against packages in your lockfiles |
| **Learned** | Behavioral patterns from your save/dismiss actions |

What survives the gate is genuinely relevant to what you build. The MCP server exposes that intelligence to any AI tool that speaks MCP.

## All 30 Tools

### Core (4 tools)

| Tool | Description |
|------|-------------|
| `get_relevant_content` | Filtered content feed -- only items that passed the 5-axis scoring gate |
| `get_context` | Your profile: tech stack, interests, affinities, ACE-detected topics |
| `explain_relevance` | Full axis breakdown of why a specific item scored the way it did |
| `record_feedback` | Teach 4DA what matters -- save, dismiss, or mark items irrelevant |

### Intelligence (9 tools)

| Tool | Description |
|------|-------------|
| `daily_briefing` | AI-generated executive summary of today's discoveries |
| `get_actionable_signals` | Classified alerts: security advisories, breaking changes, trending repos |
| `score_autopsy` | Forensic analysis of how any item's relevance score was computed |
| `trend_analysis` | Statistical patterns, anomalies, and predictions across your feed |
| `context_analysis` | Recommendations to sharpen your scoring context |
| `topic_connections` | Knowledge graph of how your content topics relate |
| `signal_chains` | Causal chains connecting related events across sources over time |
| `semantic_shifts` | Detects when topics you follow are shifting in meaning or sentiment |
| `attention_report` | Where you spend attention vs. where your codebase actually needs it |

### Decision Intelligence (3 tools)

| Tool | Description |
|------|-------------|
| `decision_memory` | Record, query, and enforce architectural decisions across sessions |
| `tech_radar` | Technology adoption signals from your decisions + content trends |
| `check_decision_alignment` | Verify if a proposed change aligns with your recorded decisions |

### Knowledge & Health (4 tools)

| Tool | Description |
|------|-------------|
| `knowledge_gaps` | Blind spots -- dependencies you use daily but never read about |
| `project_health` | Dependency freshness, security advisories, update urgency |
| `reverse_mentions` | Where your projects are being discussed across monitored sources |
| `export_context_packet` | Portable context snapshot for session or agent handoff |

### Agent Autonomy (3 tools)

| Tool | Description |
|------|-------------|
| `agent_memory` | Persistent memory that survives across sessions, agents, and editors |
| `agent_session_brief` | Tailored startup context so agents resume where you left off |
| `delegation_score` | Autonomy assessment -- should the agent proceed or ask the human? |

### Developer Identity (1 tool)

| Tool | Description |
|------|-------------|
| `developer_dna` | Your tech identity -- primary stack, dependencies, engagement patterns, blind spots |

### Intelligence Metabolism (3 tools)

| Tool | Description |
|------|-------------|
| `autophagy_status` | Self-cleaning intelligence health -- calibration accuracy, anti-patterns |
| `decision_windows` | Time-bounded opportunities that need your attention now |
| `compound_advantage` | How much intelligence leverage your decisions are generating |

### Diagnostic (3 tools)

| Tool | Description |
|------|-------------|
| `source_health` | Source fetching status and data quality diagnostics |
| `config_validator` | Configuration validation and issue detection |
| `llm_status` | LLM and Ollama provider availability check |

## Transports

**stdio** (default) -- works with all MCP hosts:
```bash
npx @4da/mcp-server
```

**Streamable HTTP** -- for remote or multi-client setups:
```bash
npx @4da/mcp-server --http --port 4840
```

## CLI Reference

```
npx @4da/mcp-server              # Start server (stdio)
npx @4da/mcp-server --http       # Start server (Streamable HTTP)
npx @4da/mcp-server --setup      # Auto-configure your editors
npx @4da/mcp-server --doctor     # Verify installation health
npx @4da/mcp-server --version    # Print version
npx @4da/mcp-server --help       # Show all options
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FOURDA_DB_PATH` | Path to 4DA's SQLite database | Auto-detected from standard install locations |

## FAQ

**Do I need the 4DA desktop app?**
No. The MCP server works standalone — on first run it creates its own database and scans your current project for languages, frameworks, and dependencies. You get immediate access to tech stack detection, dependency health, knowledge gaps, developer DNA, decision memory, and project health. Install the [desktop app](https://github.com/runyourempire/4DA) to unlock content scoring from 20+ sources (Hacker News, GitHub, arXiv, CVE feeds, etc.), daily AI briefings, and signal intelligence that compounds over time.

**Does this send my code anywhere?**
No. The MCP server reads from a local SQLite database on your filesystem. It makes zero network calls. Your codebase data, scoring profiles, and decisions never leave your machine.

**Which AI tools does this work with?**
Any tool that supports the [Model Context Protocol](https://modelcontextprotocol.io): Claude Code, Claude Desktop, Cursor, Windsurf, VS Code with Copilot, and any custom MCP client.

**Why 35 tools instead of 3?**
Because developer context is not one thing. Knowing your tech stack is different from tracking your dependencies, which is different from remembering your architectural decisions, which is different from detecting that a trending HN post is about the exact library version you pinned last week. Each tool serves a distinct intelligence function. They compose naturally -- your AI picks the right ones for each question.

**What content sources does 4DA monitor?**
Hacker News, arXiv, Reddit (customizable subreddits), GitHub Trending, GitHub Releases, RSS/Atom feeds, DevTo, Lobsters, Product Hunt, and more. All configurable.

## Build from Source

```bash
git clone https://github.com/runyourempire/4DA.git
cd 4DA/mcp-4da-server
pnpm install
pnpm build
```

## Test

```bash
pnpm test                # Contract tests (71 tests)
pnpm run inspect         # MCP Inspector (interactive browser UI)
```

## License

MIT -- use it anywhere, integrate it with anything, build on top of it.

---

Built by [4DA](https://4da.ai) -- privacy-first developer intelligence.
