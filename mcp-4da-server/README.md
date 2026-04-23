# @4da/mcp-server

[![npm version](https://img.shields.io/npm/v/@4da/mcp-server?color=gold)](https://www.npmjs.com/package/@4da/mcp-server)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Node](https://img.shields.io/badge/Node-%3E%3D18-brightgreen)](https://nodejs.org/)
[![smithery badge](https://smithery.ai/badge/@4da/mcp-server)](https://smithery.ai/server/@4da/mcp-server)

**Dependency intelligence for AI coding agents.** Live CVE scanning, dependency health checks, upgrade planning, ecosystem news, and persistent decision memory. Zero config, privacy-first.

```
You:     "Check my dependency health"
Claude:  Health: 72/100. 47 dependencies scanned, 3 vulnerable, 1 deprecated, 8 outdated.

         CRITICAL  openssl-sys 0.9.93  CVE-2025-4231        -> 0.9.96
         HIGH      serde       1.0.197 RUSTSEC-2026-12      -> 1.0.210
         MEDIUM    cookie      0.17.0  deprecated           -> 0.18.1

         Quick wins: 6 patch upgrades, 2 minor. Run upgrade_planner for full plan.
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

Then ask your AI: **"Check my dependency health"** or **"Scan for vulnerabilities"**

## How It Works

On startup, the server reads your manifest and lock files (`package.json`, `Cargo.toml`, `go.mod`, `pyproject.toml`), resolves exact dependency versions, and queries live APIs:

- **OSV.dev** for known CVEs across all ecosystems
- **npm registry** for version freshness, deprecation status, and weekly downloads
- **crates.io sparse index** for Rust package versions (avoids the 1 req/s API limit)
- **PyPI JSON API** for Python package metadata with license normalization
- **Go module proxy** for Go module versions
- **Hacker News Algolia API** for ecosystem news filtered by your tech stack

Results are cached (24h for registry data, 1h for vulnerabilities, 30min for news) and rate-limited per source.

**What's sent over the network:** package names + versions, generic tech keywords. The same data visible in your `package.json`. No source code, no file paths, no personal data. Set `FOURDA_OFFLINE=true` to disable all network calls.

**Ecosystems supported:** npm, crates.io (Rust), PyPI (Python), Go.

## What You Can Ask

```
"Check my dependency health"                  -> dependency_health
"Scan for vulnerabilities"                    -> vulnerability_scan
"Which deps should I upgrade first?"          -> upgrade_planner
"What should I know before I start coding?"   -> what_should_i_know
"What's happening in the ecosystem?"          -> ecosystem_pulse
"What's my tech stack?"                       -> get_context
"Record a decision: we chose Postgres"        -> decision_memory
"Does switching to MySQL align?"              -> check_decision_alignment
"Remember: never use ORM for batch inserts"   -> agent_memory
```

## All 14 Tools

### Dependency Security

| Tool | What it does |
|------|-------------|
| `vulnerability_scan` | Live CVE scanning via OSV.dev. Severity, fix versions, CVSS scores. |
| `dependency_health` | Health score (0-100) + version freshness, deprecation, CVE counts per dependency. |
| `upgrade_planner` | Ranked upgrade recommendations. Quick wins vs. breaking changes. Risk-sorted. |

### Intelligence

| Tool | What it does |
|------|-------------|
| `what_should_i_know` | Pre-task intelligence briefing: vulns, decisions, signals, ecosystem updates. |
| `ecosystem_pulse` | Live ecosystem news from Hacker News, filtered by your detected tech stack. |
| `get_context` | Your tech stack, resolved dependency versions, interests, affinities. |
| `get_relevant_content`* | Scored content feed — articles, advisories, releases ranked by relevance. |
| `get_actionable_signals`* | Classified alerts: security advisories, breaking changes, trending repos. |
| `knowledge_gaps`* | Dependencies you use daily but never read about. Surfaces missed CVEs and updates. |
| `record_feedback`* | Save or dismiss items to teach 4DA what matters. Sharpens future scoring. |

### Decisions & Memory

| Tool | What it does |
|------|-------------|
| `decision_memory` | Record, query, and manage architectural decisions across sessions. |
| `check_decision_alignment` | Verify if a proposed technology change aligns with recorded decisions. |
| `agent_memory` | Persistent memory that survives across sessions, agents, and editors. |

### Identity

| Tool | What it does |
|------|-------------|
| `developer_dna`* | Your tech identity: primary stack, engagement patterns, blind spots. |

*\* Requires the [4DA desktop app](https://4da.ai) for full data.*

## Standalone vs. Full Mode

The MCP server works without the desktop app. On first run it creates a local database and scans your project:

| Capability | Standalone | With 4DA Desktop |
|------------|-----------|-------------------|
| Vulnerability scanning (OSV.dev) | Yes | Yes |
| Dependency health (4 registries) | Yes | Yes |
| Upgrade planner | Yes | Yes |
| Ecosystem news (Hacker News) | Yes | Yes |
| Pre-task intelligence briefing | Yes | Yes |
| Tech stack detection + resolved versions | Yes | Yes |
| Decision memory + alignment checking | Yes | Yes |
| Agent memory (cross-session) | Yes | Yes |
| Scored content feed (20+ sources) | -- | Yes |
| Actionable signals + knowledge gaps | -- | Yes |
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
No. The server sends package names and versions to public APIs ([OSV.dev](https://osv.dev), npm registry, crates.io, PyPI, Go proxy) and generic tech keywords to [HN Algolia](https://hn.algolia.com/api). The same public data visible in your `package.json`. No source code, no file paths, no personal data. Set `FOURDA_OFFLINE=true` to disable all network calls.

**Do I need the 4DA desktop app?**
No. 9 tools work standalone: vulnerability scanning, dependency health, upgrade planning, ecosystem news, pre-task briefings, project context, decision memory, alignment checking, and agent memory. The desktop app adds scored content from 20+ sources that compounds over time.

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
