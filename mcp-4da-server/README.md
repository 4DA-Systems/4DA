# @4da/mcp-server

[![npm version](https://img.shields.io/npm/v/@4da/mcp-server?color=gold)](https://www.npmjs.com/package/@4da/mcp-server)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Node](https://img.shields.io/badge/Node-%3E%3D18-brightgreen)](https://nodejs.org/)
[![smithery badge](https://smithery.ai/badge/@4da/mcp-server)](https://smithery.ai/server/@4da/mcp-server)

**MCP server that scans your dependencies for known CVEs. Auto-detects your stack, queries OSV.dev, returns severity + fix versions. Zero config, privacy-first.**

```
You:     "Scan my project for vulnerabilities"
Claude:  Found 3 advisories affecting your dependencies:

         CRITICAL  openssl-sys 0.9.93     CVE-2025-4231   -> upgrade to 0.9.96
         HIGH      serde       1.0.197    RUSTSEC-2026-12 -> upgrade to 1.0.210
         MEDIUM    cookie      0.17.0     RUSTSEC-2025-81 -> upgrade to 0.18.1

         45 dependencies scanned across npm + crates.io in 1.2s
```

One command to install. No API keys. No accounts. No code leaves your machine.

## Install

```bash
claude mcp add 4da -- npx @4da/mcp-server
```

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

<details>
<summary><b>Auto-setup</b> (detects all installed editors)</summary>

```bash
npx @4da/mcp-server --setup
```
</details>

Then ask your AI: **"Scan my project for vulnerabilities"**

## How It Works

On first run, the server reads your manifest and lock files (`package.json`, `Cargo.toml`, `go.mod`, `pyproject.toml`), resolves exact dependency versions, and queries [OSV.dev](https://osv.dev) for known vulnerabilities. Results include CVE/GHSA IDs, CVSS scores, severity ratings, fix versions, and upgrade recommendations.

**What's sent over the network:** package names + versions. That's it. The same data visible in your `package.json`. No source code, no file paths, no personal data. Set `FOURDA_OFFLINE=true` to disable all network calls.

**Ecosystems supported:** npm, crates.io (Rust), PyPI (Python), Go.

## What You Can Ask

```
"Scan my project for vulnerabilities"        -> vulnerability_scan
"What's my tech stack?"                       -> get_context
"What are my knowledge gaps?"                 -> knowledge_gaps
"Any security issues in my dependencies?"     -> project_health
"Record a decision: we chose Postgres"        -> decision_memory
"Export my developer profile"                 -> developer_dna
"What should I know before I start coding?"   -> what_should_i_know
```

## All 36 Tools

### Security

| Tool | Description |
|------|-------------|
| `vulnerability_scan` | Scan dependencies for known CVEs via OSV.dev. Zero config. npm, Rust, Python, Go. |
| `project_health` | Dependency inventory + security score + vulnerable package list |
| `get_actionable_signals` | Classified alerts: security advisories, breaking changes, trending repos |

### Project Context

| Tool | Description |
|------|-------------|
| `get_context` | Your tech stack, interests, affinities, ACE-detected topics |
| `developer_dna` | Full tech identity: primary stack, dependencies, engagement patterns, blind spots |
| `knowledge_gaps` | Dependencies you use daily but never read about |
| `export_context_packet` | Portable context snapshot for session or agent handoff |

### Decision Memory

| Tool | Description |
|------|-------------|
| `decision_memory` | Record, query, and enforce architectural decisions across sessions |
| `tech_radar` | Technology adoption signals from your decisions + content trends |
| `check_decision_alignment` | Verify if a proposed change aligns with your recorded decisions |

### Agent Autonomy

| Tool | Description |
|------|-------------|
| `agent_memory` | Persistent memory that survives across sessions, agents, and editors |
| `agent_session_brief` | Tailored startup context so agents resume where you left off |
| `delegation_score` | Autonomy assessment: should the agent proceed or ask the human? |
| `what_should_i_know` | Pre-task intelligence briefing: advisories, decisions, signals |
| `record_agent_feedback` | Record whether agent recommendations were used or rejected |
| `get_agent_feedback_stats` | Agent recommendation accuracy: source usefulness, top items, trends |

### Content Intelligence

Requires the [4DA desktop app](https://4da.ai) for full functionality. These tools score and analyze content from Hacker News, GitHub, arXiv, Reddit, and 7 other sources against your tech stack.

| Tool | Description |
|------|-------------|
| `get_relevant_content` | Filtered content feed: only items that pass the 5-axis scoring gate |
| `daily_briefing` | AI-generated executive summary of today's discoveries |
| `explain_relevance` | Full axis breakdown of why a specific item scored the way it did |
| `record_feedback` | Teach 4DA what matters: save, dismiss, or mark items irrelevant |
| `score_autopsy` | Forensic analysis of how any item's relevance score was computed |
| `trend_analysis` | Statistical patterns, anomalies, and predictions across your feed |
| `context_analysis` | Recommendations to sharpen your scoring context |
| `topic_connections` | Knowledge graph of how your content topics relate |
| `signal_chains` | Causal chains connecting related events across sources over time |
| `semantic_shifts` | Detects when topics you follow are shifting in meaning or sentiment |
| `attention_report` | Where you spend attention vs. where your codebase actually needs it |
| `preemption_feed` | Forward-looking alerts on risks and ecosystem shifts affecting your stack |
| `trust_summary` | Intelligence quality metrics: precision, action rate, false positives |
| `reverse_mentions` | Where your projects are being discussed across monitored sources |
| `autophagy_status` | Self-cleaning intelligence health: calibration accuracy, anti-patterns |
| `decision_windows` | Time-bounded opportunities that need your attention now |
| `compound_advantage` | How much intelligence leverage your decisions are generating |

### Diagnostic

| Tool | Description |
|------|-------------|
| `source_health` | Source fetching status and data quality diagnostics |
| `config_validator` | Configuration validation and issue detection |
| `llm_status` | LLM and Ollama provider availability check |

## Standalone vs. Full Mode

The MCP server works without the desktop app. On first run it creates a local database and scans your project:

| Capability | Standalone | With 4DA Desktop |
|------------|-----------|-------------------|
| Vulnerability scanning (OSV.dev) | Yes | Yes |
| Tech stack detection | Yes | Yes |
| Project health + dependency inventory | Yes | Yes |
| Knowledge gaps | Yes | Yes |
| Decision memory | Yes | Yes |
| Agent memory | Yes | Yes |
| Content scoring (HN, GitHub, arXiv, etc.) | -- | Yes |
| Daily AI briefings | -- | Yes |
| Trend analysis + signal chains | -- | Yes |
| Compound intelligence (learns over time) | -- | Yes |

> **[Download 4DA](https://github.com/runyourempire/4DA/releases/latest)** for the full experience.

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
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FOURDA_DB_PATH` | Path to 4DA's SQLite database | Auto-detected |
| `FOURDA_OFFLINE` | Disable all network calls | `false` |

## FAQ

**Does this send my code anywhere?**
No. The vulnerability scanner sends package names and versions to [OSV.dev](https://osv.dev) -- the same public manifest data visible in your `package.json`. No source code, no file paths, no personal data. Everything else runs entirely locally. Set `FOURDA_OFFLINE=true` to disable all network calls.

**Do I need the 4DA desktop app?**
No. The MCP server works standalone: vulnerability scanning, tech stack detection, dependency health, knowledge gaps, decision memory, and agent memory all work immediately. The desktop app adds content intelligence from 10+ sources that compounds over time.

**Which AI tools does this work with?**
Any tool that supports [MCP](https://modelcontextprotocol.io): Claude Code, Claude Desktop, Cursor, Windsurf, VS Code (Copilot), and any custom MCP client.

## Build from Source

```bash
git clone https://github.com/runyourempire/4DA.git
cd 4DA/mcp-4da-server
pnpm install
pnpm build
pnpm test    # 71 contract tests
```

## License

MIT

---

Built by [4DA](https://4da.ai)
