<div align="center">

<img src="assets/4da-hero.png" alt="4DA — All signal. No feed." width="360" />

<br />

[![License: FSL-1.1](https://img.shields.io/badge/License-FSL--1.1--Apache--2.0-blue.svg)](LICENSE)
[![MCP Server](https://img.shields.io/npm/v/@4da/mcp-server?label=MCP%20Server&color=gold)](https://www.npmjs.com/package/@4da/mcp-server)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-brightgreen.svg)](#download)

**All signal. No feed.**

</div>

---

**4DA reads the internet for developers — privately, locally — and gets sharper every day.**

It scans your codebase — `Cargo.toml`, `package.json`, `go.mod`, Git history — and scores every article, advisory, and release from 20+ sources against what you actually build. An item needs 2+ independent signals to survive. Everything else is rejected.

Typical rejection rate: **99%+**. What's left is yours.

Privacy-first. Local-first. BYOK. Your indexed content stays on your machine — there is no 4DA-operated server for it to go to. It learns from how you engage with what it shows you — yesterday's noise becomes tomorrow's signal. Crash reporting is opt-in, off by default. For the full list of outbound connections with source-code references, see [NETWORK.md](NETWORK.md).

<p align="center">
  <img src="site/screenshots/01-brief.png" alt="4DA Brief tab — top picks and live signal stream scored against your stack" width="800" />
</p>

---

## How It Scores

5 independent signal axes. An item must pass **2 or more** to surface. Single-axis matches are hard-capped at 28% — no matter how strong one signal is, it cannot pass alone.

| Axis | What it measures |
|------|-----------------|
| **Context** | Semantic similarity to your active codebase |
| **Interest** | Alignment with your declared and learned topics |
| **ACE** | Real-time signals from your Git commits and file edits |
| **Dependency** | Direct matches against your installed packages |
| **Learned** | Save/dismiss feedback boosts or suppresses future scores |

What passes the gate goes through 12 quality multipliers: content depth, novelty detection, competing tech penalties, title-body coherence, and intent scoring from recent work. Every constant is calibrated across 9 simulated developer personas with 215 labeled test items.

### LLM Verification (Optional)

After keyword scoring, an optional LLM layer verifies the top items against your full developer context — stack, dependencies, recent commits, anti-technologies, and engagement history. Strict 1-5 rubric:

- **5 = MUST-READ**: Security alert for YOUR dependency, breaking change YOU must act on
- **3 = WORTH KNOWING**: Useful tool that fits YOUR exact stack
- **1 = NOISE**: Mentions your tech but isn't actionable

This is where gold nuggets surface — articles the keyword pipeline misses because there's no keyword overlap, but the LLM understands the conceptual relevance to your specific project.

**You control the compute.** Use [Ollama](https://ollama.com/) for free local inference (fully private), or bring your own Anthropic/OpenAI key. 4DA never pays for your compute, never stores your keys remotely, never makes API calls you didn't configure. No LLM = keyword pipeline only (85-90% accurate).

### It Gets Smarter

Every interaction is a training signal:

- **Save** → topics boost, source reputation rises, taste embedding sharpens
- **Dismiss** → topics suppress, anti-patterns form, future noise drops
- Autophagy cycles detect systematic over/under-scoring and self-correct
- Topic decay calibrates per-topic: "Rust" stays relevant for days, "JS framework of the week" decays in hours

### It Can't Be Gamed

Content creators who learn the scoring algorithm still can't game it:

- **Title-body coherence**: titles must deliver on what they promise. Claim "React + Rust + Tauri" but only discuss React? Penalty.
- **Keyword concentration**: repeating "Rust" four times in a title hurts your score.
- **Confirmation gate**: keyword-stuffing hits one axis. Without matching the user's codebase, installed packages, AND recent work — the gate rejects it.
- **Feedback loop**: gamed articles get dismissed. Sources that produce dismissed content lose reputation. Gaming becomes self-defeating.

No algorithm can be gamed when the scoring signal comes from your local filesystem. Your `Cargo.lock` doesn't lie. Your Git history doesn't optimize for engagement.

---

## How 4DA Is Different

| Tool | What it does well | How 4DA differs |
|------|-------------------|-----------------|
| **daily.dev** | Great for discovery — surfaces trending content across the developer ecosystem | 4DA scores against your codebase, not your clicks. Relevance comes from what you build, not what you browse. |
| **Feedly** | Powerful aggregation — brings all your feeds into one place | 4DA adds relevance scoring on top of aggregation. 20+ sources built in, no subscription management. |
| **Pocket** | Clean read-it-later experience for saving articles | 4DA surfaces what you should read before you know to save it. Proactive, not reactive. |
| **TLDR / newsletters** | Consistent daily curation from knowledgeable editors | 4DA personalizes to YOUR stack. A Rust systems developer and a React frontend developer get completely different results. |

These are good tools that solve different problems. 4DA solves a specific one: scoring content against your actual technology identity — your `Cargo.lock`, your `package.json`, your Git history. That's a fundamentally different signal source than clicks, subscriptions, or editorial judgment.

---

## Download

> **Pre-built binaries** — no Rust toolchain required.

| Platform | Download | Auto-updates |
|----------|----------|:------------:|
| **Windows** | [`.exe` installer](https://github.com/runyourempire/4DA/releases/latest) | Yes |
| **macOS** | [`.dmg` (Apple Silicon & Intel)](https://github.com/runyourempire/4DA/releases/latest) | Yes |
| **Linux** | [`.AppImage` / `.deb`](https://github.com/runyourempire/4DA/releases/latest) | Yes |

Every release publishes `SHASUMS256.txt` (all platforms, canonical) plus per-file `<artifact>.sha256` sidecars. Verify before running:

```bash
# Bash (macOS/Linux/Git-Bash):  compare against SHASUMS256.txt
sha256sum -c SHASUMS256.txt --ignore-missing

# PowerShell (Windows):  compute and compare one file (substitute the
# actual installer filename from the release page — Tauri tags platform + version)
Get-FileHash -Algorithm SHA256 .\4DA.Home_1.0.0_x64-setup.exe
```

> **Windows users:** because 4DA is a new release, SmartScreen will prompt on first launch. Click **More info → Run anyway**. Signed Windows builds deliver silently via the auto-updater once EV certification completes. See the [Windows install guide](docs/launch/WINDOWS-INSTALL.md) for verification steps.

Or install the **MCP server** for Claude Code / Cursor:
```bash
npx @4da/mcp-server
```

## Quick Start (Build from Source)

**Required tools** (install in this order):

1. **Rust** (auto-pins to 1.93.1 via `rust-toolchain.toml`) — [rustup.rs](https://rustup.rs/)
2. **Node.js 20 LTS** — [nodejs.org](https://nodejs.org/) (`.nvmrc` provided for nvm/fnm/Volta)
3. **pnpm 9.15.0** (pinned in `package.json` `packageManager`): `corepack enable && corepack prepare pnpm@9.15.0 --activate`

**Platform build tools** (required before `pnpm tauri dev`):

- **Windows:** Visual Studio Build Tools 2022 with the **"Desktop development with C++"** workload. Without it, the Rust build fails with `linker 'link.exe' not found`.
- **macOS:** `xcode-select --install`
- **Linux (Debian/Ubuntu):** see the full apt list in [docs/BUILD-FROM-SOURCE.md#linux-debianubuntu](docs/BUILD-FROM-SOURCE.md) — webkit2gtk, gtk3, libappindicator, etc.

**Build & run:**

```bash
git clone https://github.com/runyourempire/4DA.git
cd 4DA
pnpm install
pnpm tauri dev   # First build: 5-15 min. Dev server: localhost:4444.
```

> Hitting an issue? **[docs/BUILD-FROM-SOURCE.md](docs/BUILD-FROM-SOURCE.md)** covers MSVC linker errors, pkg-config issues, port conflicts, WebView2 quirks, and slow first builds.

> First-run app setup (API keys, context dirs, sources): **[docs/GETTING_STARTED.md](docs/GETTING_STARTED.md)**.

Onboard (pick your stack, add an API key, point at your projects), and run your first scan. First useful results in under 3 minutes once the build completes.

---

## Architecture

```
Your Codebase                    External Sources
     |                                |
     v                                v
+-----------+                +--------------+
|    ACE    |                |  20+ Source   |
| Scanner + |                |  Adapters     |
| Git Watch |                |  (background) |
+-----+-----+               +------+-------+
      |                            |
      v                            v
+------------------------------------------+
|         5-Axis Scoring Engine            |
|                                          |
|  context --+                             |
|  interest --+- confirmation gate (2+/5)  |
|  ace -------+                            |
|  dependency-+  x quality x novelty       |
|  learned ---+  x domain  x intent        |
+------------------+-----------------------+
                   |
                   v
          +-----------------+
          |  What survived  |
          +-----------------+
```

| Layer | Technology |
|-------|-----------|
| App Shell | Tauri 2.0 (Rust backend + WebView) |
| Frontend | React 19 + TypeScript + Tailwind CSS v4 |
| Database | SQLite 3.45+ with sqlite-vec (vector search) |
| Scoring | Custom DSL -> build-time Rust codegen |
| Embeddings | OpenAI text-embedding-3-small / Ollama |
| LLM | Anthropic Claude / OpenAI / Ollama (BYOK) |

---

## Privacy & Trust

4DA is local-first and direct-to-provider. There is no 4DA-operated server, no 4DA-operated analytics, and no user account system. Your indexed content, scores, decisions, and AWE wisdom live in a SQLite database on your machine.

**The only outbound traffic 4DA makes:**

- **Source adapters** fetching public content (HN, GitHub, Reddit, arxiv, CVE feeds — see [NETWORK.md](NETWORK.md))
- **BYOK LLM providers** you explicitly configured (Anthropic / OpenAI / localhost Ollama)
- **License validation** (Keygen) if you activated a paid license
- **Updater** against GitHub Releases (signed via minisign)
- **Opt-in crash reports** (Sentry), *off by default*, fully disabled unless you turn it on in Settings → Privacy

That's the whole list. There is no 4DA telemetry endpoint because there is no 4DA cloud.

- Core scoring is pure Rust — zero API calls, 2 seconds for 500 items, $0.00 per analysis
- LLM verification (optional): free with Ollama, or ~$0.05/analysis with your own cloud key
- BYOK: your keys, your models, your billing. We never touch your compute costs.
- With Ollama, the entire app runs fully offline

Don't take our word for it. Verify every claim:

| Document | What It Covers |
|----------|---------------|
| [**Trust Architecture**](docs/TRUST-ARCHITECTURE.md) | Why local-first means you don't need to trust us |
| [**Network Transparency**](docs/NETWORK-TRANSPARENCY.md) | Every outbound connection, with source code references |
| [**Privacy (Plain Language)**](docs/PRIVACY-PLAIN-LANGUAGE.md) | One-page, no-legalese privacy summary |
| [**Build from Source**](docs/BUILD-FROM-SOURCE.md) | Compile it yourself and verify the binary |
| [**Security Audit Guide**](docs/SECURITY-AUDIT-GUIDE.md) | Map of trust-critical code paths for auditors |
| [**Security Policy**](SECURITY.md) | Vulnerability reporting and security model |

---

## Pricing

**Free** — $0 forever. No credit card. No account. No expiration.
- All 20+ sources, full 5-axis scoring engine, AI daily briefings (BYOK), natural language search (BYOK), behavior learning, STREETS Playbook (all 7 modules), MCP server (33 tools), CLI

**Signal** — $12/month or $99/year. Compound intelligence that gets smarter every day.
- Everything in Free, plus: Signal tab intelligence (Key Signals + analytics), Score Autopsy (5-axis breakdown), Developer DNA profiling, signal chain analysis, knowledge gap detection, semantic shift tracking, attention analytics, standing queries, project health radar

---

## Features

<details>
<summary><strong>Intelligence</strong></summary>

- 5-axis scoring with multi-signal confirmation gate (99%+ rejection rate)
- Domain profile: graduated tech identity (primary stack -> dependencies -> detected -> interests)
- Content DNA: classifies content type (security advisory, release, tutorial, hiring, etc.)
- Novelty detection: demotes introductory content, boosts new releases and security advisories
- Role-aware scoring: security engineers see security content prominently; experience level adjusts tutorial/depth balance
- Intent scoring: recent Git/file activity influences what surfaces
- Knowledge gap detection: finds blind spots in your dependency understanding
- Anti-gaming: title-body coherence, keyword concentration, adversarial resistance built into the pipeline

</details>

<details>
<summary><strong>Sources</strong> — 20+ adapters, all running locally</summary>

- Hacker News, GitHub, Reddit, YouTube, arXiv, Stack Overflow
- Lobsters, DEV.to, Product Hunt, Twitter/X, Bluesky, Hugging Face
- Papers with Code, crates.io, npm, PyPI, Go modules
- CVE/OSV vulnerability databases, custom RSS feeds

</details>

<details>
<summary><strong>Analysis</strong></summary>

- Signal chains: tracks evolving stories across sources
- Semantic shift detection: notices when topics you follow are changing
- Reverse mentions: finds where your projects are discussed
- Project health radar: dependency freshness + security monitoring
- Attention dashboard: where you spend time vs. where you should

</details>

<details>
<summary><strong>Decision Intelligence</strong></summary>

- Record and query architectural decisions across sessions
- Tech radar: adoption signals from decisions + content trends
- Decision enforcement: AI agents check alignment before suggesting changes

</details>

<details>
<summary><strong>Agent Autonomy</strong></summary>

- Cross-session, cross-agent persistent memory
- Session briefs: tailored startup context for any AI tool
- Delegation scoring: should the agent proceed or ask you?
- Developer DNA: exportable tech identity profile (markdown, SVG, or shareable card)

</details>

<details>
<summary><strong>MCP Integration</strong> — 33 tools across 8 categories</summary>

Plug your intelligence system directly into Claude Code, Cursor, or any MCP-compatible tool.

```bash
npx @4da/mcp-server
```

**Core** — `get_relevant_content`, `get_context`, `explain_relevance`, `record_feedback`
**Intelligence** — `daily_briefing`, `get_actionable_signals`, `score_autopsy`, `trend_analysis`, `context_analysis`, `topic_connections`, `signal_chains`, `semantic_shifts`, `attention_report`
**Diagnostic** — `source_health`, `config_validator`, `llm_status`
**Innovation** — `knowledge_gaps`, `project_health`, `reverse_mentions`, `export_context_packet`
**Decision Intelligence** — `decision_memory`, `tech_radar`, `check_decision_alignment`
**Agent Autonomy** — `agent_memory`, `agent_session_brief`, `delegation_score`, `record_agent_feedback`, `agent_feedback_stats`, `what_should_i_know`
**Developer DNA** — `developer_dna`
**Intelligence Metabolism** — `autophagy_status`, `decision_windows`, `compound_advantage`

</details>

<details>
<summary><strong>CLI</strong></summary>

Reads from the same database as the desktop app. No extra setup.

```bash
4da briefing               # Latest AI briefing
4da signals                # All classified signals
4da signals --critical     # Critical/high priority only
4da gaps                   # Knowledge gaps in your dependencies
4da health                 # Project dependency health
4da status                 # Database stats
```

</details>

---

## Screenshots

<p align="center">
  <img src="site/screenshots/01-brief.png" alt="Brief tab" width="800" />
  <br />
  <em>Brief — today's top picks and live signal stream scored against your stack</em>
</p>

<p align="center">
  <img src="site/screenshots/02-preemption.png" alt="Preemption Radar" width="800" />
  <br />
  <em>Preemption — forward-looking intelligence: CVEs, breaking changes, dependency risks</em>
</p>

<p align="center">
  <img src="site/screenshots/03-blind-spots.png" alt="Blind Spot Index" width="800" />
  <br />
  <em>Blind Spots — coverage gaps and high-relevance items you never saw</em>
</p>

<p align="center">
  <img src="site/screenshots/04-signal.png" alt="Signal tab" width="800" />
  <br />
  <em>Signal — the items that earn their place, confirmed through 2+ independent axes</em>
</p>

<p align="center">
  <img src="site/screenshots/05-playbook.png" alt="STREETS Playbook" width="800" />
  <br />
  <em>Playbook — the STREETS sovereignty playbook built in, seven modules, always free</em>
</p>

---

## Development

```bash
pnpm tauri dev              # Dev server (localhost:4444)
cargo test                  # Rust tests (from src-tauri/)
pnpm test                   # Frontend tests
pnpm validate:all           # Full validation (lint + types + tests + build)
```

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
