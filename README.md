<div align="center">

<img src="assets/4da-hero.png" alt="4DA — All signal. No feed." width="360" />

<br />

[![License: FSL-1.1](https://img.shields.io/badge/License-FSL--1.1--Apache--2.0-blue.svg)](LICENSE)
[![MCP Server](https://img.shields.io/npm/v/@4da/mcp-server?label=MCP%20Server&color=gold)](https://www.npmjs.com/package/@4da/mcp-server)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-brightgreen.svg)](#download)

**All signal. No feed.**

4DA is a developer intelligence system that scores content from 20+ sources against your actual codebase using 5-axis relevance scoring. An item needs 2+ independent signals to survive. Everything else is rejected. Typical rejection rate: **99%+**.

Privacy-first. Runs locally. Zero telemetry. BYOK. ~15MB download.

[Download](#download) &bull; [Quick Start](#quick-start) &bull; [How It Works](#how-it-works) &bull; [MCP Integration](#mcp-integration) &bull; [Pricing](#pricing)

</div>

---

## Download

> **Pre-built binaries** — no Rust toolchain required.

| Platform | Download | Auto-updates |
|----------|----------|:------------:|
| **Windows** | [`.exe` installer](https://github.com/runyourempire/4DA/releases/latest) | Yes |
| **macOS** | [`.dmg` (Apple Silicon & Intel)](https://github.com/runyourempire/4DA/releases/latest) | Yes |
| **Linux** | [`.AppImage` / `.deb`](https://github.com/runyourempire/4DA/releases/latest) | Yes |

Or install the **MCP server** for Claude Code / Cursor:
```bash
npx @4da/mcp-server
```

Or build from source — see [Quick Start](#quick-start).

---

## See It In Action

<p align="center">
  <img src="site/screenshots/feed-signals.png" alt="Scored feed with signal classification, knowledge gaps, and relevance tags" width="800" />
  <br />
  <em>Scored feed — every item earns its place through 5-axis relevance scoring</em>
</p>

<p align="center">
  <img src="site/screenshots/intelligence-briefing.png" alt="Intelligence Briefing with AI-generated daily summary and curated top picks" width="800" />
  <br />
  <em>Intelligence Briefing — AI-generated daily summary with curated top picks</em>
</p>

<p align="center">
  <img src="site/screenshots/score-autopsy.png" alt="Score Autopsy showing full 5-axis breakdown" width="800" />
  <br />
  <em>Score Autopsy — see exactly why an item scored the way it did</em>
</p>

<p align="center">
  <img src="site/screenshots/insights-tech-radar.png" alt="Tech Radar visualization and Decision Memory" width="800" />
  <br />
  <em>Tech Radar, Decision Memory, and Developer DNA — your full tech identity</em>
</p>

---

## The Problem

You skim 500+ articles a day trying to stay current. You miss the security advisory for a package you actually use. You read three "intro to X" posts about tech you already know. You never see the arXiv paper that's directly relevant to your current project.

Meanwhile, your dependency has a breaking change, and you find out when production breaks.

## The Solution

4DA is a developer intelligence framework that runs locally on your machine. It scans your projects, reads your `Cargo.toml` / `package.json` / `go.mod`, watches your Git activity, and builds a **domain profile** — a graduated understanding of your technology identity.

Then it scores every piece of incoming content against 5 independent signal axes:

| Axis | What it measures |
|------|-----------------|
| **Context** | Semantic similarity to your active codebase |
| **Interest** | Alignment with your declared and learned topics |
| **ACE** | Real-time signals from your Git commits and file edits |
| **Dependency** | Direct matches against your installed packages |
| **Feedback** | Save/dismiss signals boost or suppress future scores |

An item needs **2+ independent signals** to pass the confirmation gate. Everything else gets rejected. Typical rejection rate: **99%+**.

What survives is scored with content quality analysis (kills clickbait), novelty detection (demotes "intro to X" if you're advanced), competing tech penalties, and intent scoring from your recent work.

## Features

**Intelligence**
- 5-axis scoring with multi-signal confirmation gate (99%+ rejection rate)
- Domain profile: graduated tech identity (primary stack → dependencies → detected → interests)
- Content DNA: classifies content type (security advisory, release, tutorial, hiring, etc.)
- Novelty detection: demotes introductory content, boosts new releases
- Intent scoring: recent Git/file activity influences what surfaces
- Knowledge gap detection: finds blind spots in your dependency understanding

**Sources** — 20+ adapters, all running locally
- Hacker News, GitHub, Reddit, YouTube, arXiv, Stack Overflow
- Lobsters, DEV.to, Product Hunt, Twitter/X, Bluesky, Hugging Face
- Papers with Code, crates.io, npm, PyPI, Go modules
- CVE/OSV vulnerability databases, custom RSS feeds

**Analysis**
- Signal chains: tracks evolving stories across sources
- Semantic shift detection: notices when topics you follow are changing
- Reverse mentions: finds where your projects are discussed
- Project health radar: dependency freshness + security monitoring
- Attention dashboard: where you spend time vs. where you should

**Decision Intelligence**
- Record and query architectural decisions across sessions
- Tech radar: adoption signals from decisions + content trends
- Decision enforcement: AI agents check alignment before suggesting changes

**Agent Autonomy**
- Cross-session, cross-agent persistent memory
- Session briefs: tailored startup context for any AI tool
- Delegation scoring: should the agent proceed or ask you?
- Developer DNA: exportable tech identity profile

**Privacy & Control**
- All data stays on your machine. Raw content never leaves.
- BYOK: Anthropic Claude, OpenAI, or fully local with Ollama
- ed25519 license verification (public key embedded, private key server-side)
- Auto-updates via Tauri updater with GitHub releases
- System tray: runs in background, surfaces what matters

## Quick Start

**Prerequisites:** [Rust](https://rustup.rs/) 1.93+ (pinned in `rust-toolchain.toml`), [Node.js](https://nodejs.org/) 20+, [pnpm](https://pnpm.io/)

```bash
git clone https://github.com/runyourempire/4DA.git
cd 4DA
pnpm install
pnpm tauri dev
```

The app opens, walks you through onboarding (pick your stack, add an API key, point at your project directories), and runs your first scan. First useful results in under 3 minutes.

### Production Build

```bash
pnpm tauri build
```

Creates platform-specific installers (`.exe` / `.dmg` / `.AppImage`) in `src-tauri/target/release/bundle/`.

## How It Works

```
Your Codebase                    External Sources
     │                                │
     ▼                                ▼
┌─────────────┐              ┌──────────────┐
│     ACE     │              │  20+ Source    │
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

4DA ships with a Model Context Protocol server — plug your intelligence system directly into Claude Code, Cursor, or any MCP-compatible tool.

```bash
cd mcp-4da-server
pnpm install && pnpm build
```

**33 tools across 8 categories:**
- **Core** — `get_relevant_content`, `get_context`, `explain_relevance`, `record_feedback`
- **Intelligence** — `daily_briefing`, `get_actionable_signals`, `score_autopsy`, `trend_analysis`, `context_analysis`, `topic_connections`, `signal_chains`, `semantic_shifts`, `attention_report`
- **Diagnostic** — `source_health`, `config_validator`, `llm_status`
- **Innovation** — `knowledge_gaps`, `project_health`, `reverse_mentions`, `export_context_packet`
- **Decision Intelligence** — `decision_memory`, `tech_radar`, `check_decision_alignment`
- **Agent Autonomy** — `agent_memory`, `agent_session_brief`, `delegation_score`, `record_agent_feedback`, `agent_feedback_stats`, `what_should_i_know`
- **Developer DNA** — `developer_dna`
- **Intelligence Metabolism** — `autophagy_status`, `decision_windows`, `compound_advantage`

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

## Trust & Verification

4DA is architectured so that trust in us is optional — your data never leaves your machine. But don't take our word for it. Verify every claim:

| Document | What It Covers |
|----------|---------------|
| [**Trust Architecture**](docs/TRUST-ARCHITECTURE.md) | Why local-first means you don't need to trust us |
| [**Network Transparency**](docs/NETWORK-TRANSPARENCY.md) | Every outbound connection, with source code references |
| [**Privacy (Plain Language)**](docs/PRIVACY-PLAIN-LANGUAGE.md) | One-page, no-legalese privacy summary |
| [**Build from Source**](docs/BUILD-FROM-SOURCE.md) | Compile it yourself and verify the binary |
| [**Verify Downloads**](docs/VERIFY-DOWNLOADS.md) | Check signatures, checksums, and code signing |
| [**Security Audit Guide**](docs/SECURITY-AUDIT-GUIDE.md) | Map of trust-critical code paths for auditors |
| [**Privacy Policy**](docs/legal/PRIVACY-POLICY.md) | Full legal privacy policy |
| [**Security Policy**](SECURITY.md) | Vulnerability reporting and security model |

**Zero telemetry. Zero analytics. Zero tracking. Zero user accounts.** The free tier makes zero calls to 4DA Systems. With Ollama, the app runs fully offline.

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

Download the CLI binary from [Releases](https://github.com/runyourempire/4DA/releases), or build from source:
```bash
cd src-tauri && cargo build --release --bin 4da
```

## Development

```bash
pnpm tauri dev              # Dev server (localhost:4444)
cargo test                  # Rust tests (from src-tauri/)
pnpm test                   # Frontend tests
pnpm validate:all           # Full validation (lint + types + tests + build)
```

## Why Not Just Use...

| Tool | Approach | Limitation |
|------|----------|------------|
| **daily.dev** | Personalizes by engagement (what you click) | Optimizes for curiosity, not project relevance. Click on one ZFS article, get storage content for weeks. |
| **Feedly** | Aggregates by subscription (what feeds you add) | Solves aggregation, not relevance. 100+ feeds = a different firehose. AI features locked behind $156/yr. |
| **Pocket** | Saves what you manually bookmark | Shut down July 2025. Cloud-dependent tools can disappear. |
| **TLDR / newsletters** | Someone else curates for "developers" broadly | One person's bias. "Developers" includes React engineers, ML researchers, and game devs — one newsletter fits none. |
| **4DA** | Scores against your actual codebase (Cargo.toml, package.json, Git) | Requires a local codebase to scan. That's the point. |

4DA doesn't personalize by what you click or subscribe to. It scores by what you **build**. A categorically different approach to developer intelligence.

## Pricing

**Free** — $0 forever. No credit card. No account. No expiration.
- All 20+ sources, full 5-axis scoring engine, AI daily briefings (BYOK), natural language search (BYOK), behavior learning, STREETS Playbook (all 7 modules), MCP server (33 tools), CLI

**Signal** — $12/month or $99/year. Compound intelligence that gets smarter every day.
- Everything in Free, plus: Signal tab intelligence (Key Signals + analytics), Score Autopsy (5-axis breakdown), Developer DNA profiling, signal chain analysis, knowledge gap detection, semantic shift tracking, attention analytics, standing queries, project health radar

## License

[FSL-1.1-Apache-2.0](LICENSE) — free to use. Source available for inspection. Converts to Apache 2.0 two years after each release.

---

<div align="center">

**4DA** — *4 Dimensional Autonomy*

All signal. No feed.

---

"4DA" and the 4DA logo are trademarks of 4DA Systems Pty Ltd (ACN 696 078 841).
The [FSL-1.1-Apache-2.0](LICENSE) license does not grant rights to use these trademarks.

</div>
