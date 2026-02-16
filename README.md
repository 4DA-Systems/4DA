<div align="center">

<img src="assets/4da-hero.png" alt="4DA — All signal. No feed." width="360" />

<br />

**Stop scrolling. Start knowing.**

4DA monitors Hacker News, arXiv, Reddit, GitHub, Product Hunt, YouTube, and RSS feeds — then ruthlessly filters out everything that doesn't matter to your specific tech stack, projects, and codebase. What survives is signal.

[Download](#download) &bull; [Quick Start](#quick-start) &bull; [How It Works](#how-it-works) &bull; [MCP Integration](#mcp-integration)

</div>

---

## Download

> **Pre-built binaries** — no Rust toolchain required.

| Platform | Download | Auto-updates |
|----------|----------|:------------:|
| **Windows** | [`.msi` installer](https://github.com/runyourempire/4da-v3/releases/latest) | Yes |
| **macOS** | [`.dmg` (Apple Silicon & Intel)](https://github.com/runyourempire/4da-v3/releases/latest) | Yes |
| **Linux** | [`.AppImage` / `.deb`](https://github.com/runyourempire/4da-v3/releases/latest) | Yes |

Or install the **MCP server** for Claude Code / Cursor:
```bash
npx @4da/mcp-server
```

Or build from source — see [Quick Start](#quick-start).

---

## The Problem

You skim 500+ articles a day trying to stay current. You miss the security advisory for a package you actually use. You read three "intro to X" posts about tech you already know. You never see the arXiv paper that's directly relevant to your current project.

Meanwhile, your dependency has a breaking change, and you find out when production breaks.

## The Solution

4DA runs locally on your machine. It scans your projects, reads your `Cargo.toml` / `package.json` / `go.mod`, watches your Git activity, and builds a **domain profile** — a graduated understanding of your technology identity.

Then it scores every piece of incoming content against 5 independent signal axes:

| Axis | What it measures |
|------|-----------------|
| **Context** | Semantic similarity to your active codebase |
| **Interest** | Alignment with your declared and learned topics |
| **ACE** | Real-time signals from your Git commits and file edits |
| **Dependency** | Direct matches against your installed packages |
| **Learned** | Behavioral patterns from your save/dismiss feedback |

An item needs **2+ independent signals** to pass the confirmation gate. Everything else gets rejected. Typical rejection rate: **99%+**.

What survives is scored with content quality analysis (kills clickbait), novelty detection (demotes "intro to X" if you're advanced), competing tech penalties, and intent scoring from your recent work.

## Features

**Intelligence**
- 5-axis scoring with multi-signal confirmation gate
- Domain profile: graduated tech identity (primary stack → dependencies → detected → interests)
- Content DNA: classifies content type (security advisory, release, tutorial, hiring, etc.)
- Novelty detection: demotes introductory content, boosts new releases
- Intent scoring: recent Git/file activity influences what surfaces
- Knowledge gap detection: finds blind spots in your dependency understanding

**Sources** — 11 adapters, all running locally
- Hacker News, Reddit, arXiv, GitHub Releases
- Product Hunt, YouTube, Twitter/X
- Dev.to, Lobsters, custom RSS feeds

**Analysis**
- Signal chains: tracks evolving stories across sources
- Semantic shift detection: notices when topics you follow are changing
- Reverse mentions: finds where your projects are discussed
- Project health radar: dependency freshness + security monitoring
- Attention dashboard: where you spend time vs. where you should

**Privacy & Control**
- All data stays on your machine. Raw content never leaves.
- BYOK: Anthropic Claude, OpenAI, or fully local with Ollama
- Transparent cost tracking with daily limits
- System tray: runs in background, surfaces what matters

## Quick Start

**Prerequisites:** [Rust](https://rustup.rs/) 1.70+, [Node.js](https://nodejs.org/) 18+, [pnpm](https://pnpm.io/)

```bash
git clone https://github.com/runyourempire/4da-v3.git
cd 4da-v3
pnpm install
pnpm tauri dev
```

The app opens, walks you through onboarding (pick your stack, add an API key, point at your project directories), and runs your first scan. First useful results in under 3 minutes.

### Production Build

```bash
pnpm tauri build
```

Creates platform-specific installers (`.msi` / `.dmg` / `.AppImage`) in `src-tauri/target/release/bundle/`.

## How It Works

```
Your Codebase                    External Sources
     │                                │
     ▼                                ▼
┌─────────────┐              ┌──────────────┐
│     ACE     │              │  11 Source    │
│  Scanner +  │              │  Adapters     │
│  Git Watch  │              │  (background) │
└──────┬──────┘              └──────┬───────┘
       │                            │
       ▼                            ▼
┌──────────────────────────────────────────┐
│           5-Axis Scoring Engine          │
│                                          │
│  context ─┐                              │
│  interest ─┼─ confirmation gate (2+ of 5)│
│  ace ──────┤                             │
│  dependency┤   × quality × novelty       │
│  learned ──┘   × domain × intent         │
└──────────────────┬───────────────────────┘
                   │
                   ▼
          ┌─────────────────┐
          │  Signal + Feed  │
          │  (what survived)│
          └─────────────────┘
```

## Developer DNA

4DA builds a **Developer DNA** profile from your codebase and behavior — a quantified snapshot of your technology identity.

- **Primary stack**: what you declared + what ACE detected
- **Dependency graph**: every package you actually use
- **Topic engagement**: where your attention goes
- **Blind spots**: gaps between what you use and what you track

Shareable as markdown.

## MCP Integration

4DA ships with a Model Context Protocol server — plug your intelligence feed directly into Claude Code, Cursor, or any MCP-compatible tool.

```bash
cd mcp-4da-server
pnpm install && pnpm build
```

**13 tools available:**
- `get_relevant_content` — query filtered content by relevance
- `get_context` — your interests, tech stack, learned affinities
- `get_actionable_signals` — classified signals with priority levels
- `knowledge_gaps` — dependency blind spots
- `project_health` — dependency freshness + security radar
- `daily_briefing` — executive summary
- `topic_connections` — knowledge graph from content
- `signal_chains` — causal event chains over time
- `attention_report` — attention allocation vs. codebase needs
- And 4 more diagnostic/analysis tools

Add to your Claude Code config:
```json
{
  "mcpServers": {
    "4da": {
      "command": "node",
      "args": ["path/to/mcp-4da-server/dist/index.js"]
    }
  }
}
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| App Shell | Tauri 2.0 (Rust backend + WebView) |
| Frontend | React 19 + TypeScript + Tailwind CSS v4 |
| Database | SQLite 3.45+ with sqlite-vec (vector search) |
| Scoring | Custom DSL → build-time Rust codegen |
| Embeddings | OpenAI text-embedding-3-small / Ollama |
| LLM | Anthropic Claude / OpenAI / Ollama |

## Philosophy

1. **Privacy first** — raw data never leaves your machine
2. **BYOK** — your keys, your models, your choice
3. **Local first** — works offline with Ollama
4. **Minimal** — no feature bloat; every element earns its place
5. **Signal over noise** — 99%+ rejection rate is a feature, not a bug

## CLI

The CLI reads from the same database as the desktop app. No extra setup needed.

```bash
4da briefing               # Latest AI briefing
4da signals                # All classified signals
4da signals --critical     # Critical/high priority only
4da gaps                   # Knowledge gaps in your dependencies
4da health                 # Project dependency health
4da status                 # Database stats
```

Download the CLI binary from [Releases](https://github.com/runyourempire/4da-v3/releases), or build from source:
```bash
cd src-tauri && cargo build --release --bin 4da
```

## Development

```bash
pnpm tauri dev              # Dev server (localhost:4444)
cargo test                  # Rust tests (from src-tauri/)
pnpm test                   # Frontend tests (98)
pnpm validate:all           # Full validation (lint + types + tests + build)
```

## License

[BUSL-1.1](LICENSE) — free for non-commercial use. Converts to Apache 2.0 on 2029-02-15.

---

<div align="center">

**4DA** — *4 Dimensional Autonomy*

The intelligence layer for developers who'd rather build than scroll.

</div>
