# 4DA v1.0.0 — All signal. No feed.

4DA reads the internet for developers — privately, locally — and gets sharper every day.

Every piece of content is scored against your actual codebase using PASIFA, a 5-axis relevance engine that runs entirely on your machine. No feeds to scroll. No algorithms optimizing for engagement. Just the signals that matter to the code you're writing right now.

## What's Inside

- **5-axis relevance scoring** — PASIFA rates every item against your local projects (context, interest, dependencies, learned patterns, ACE codebase scan)
- **20+ built-in content sources** — Hacker News, Reddit, GitHub, arXiv, CVE/OSV, Stack Overflow, crates.io, PyPI, npm, Lobsters, Product Hunt, Bluesky, Dev.to, Hugging Face, YouTube, Go Modules, RSS, and more
- **36 MCP tools** — `npx @4da/mcp-server --setup` installs in one command. Works standalone or with the desktop app. Claude Code, Cursor, Windsurf, GitHub Copilot compatible.
- **AI briefings** — daily and weekly intelligence summaries generated locally via your own API key
- **STREETS Playbook** — 7 modules for developer growth, included free
- **14 languages** — Arabic, Chinese, English, French, German, Hindi, Italian, Japanese, Korean, Portuguese (BR), Russian, Spanish, Turkish. Content translation, cross-lingual search, native briefings.

## Download

| Platform | File | Notes |
|----------|------|-------|
| Windows | `4DA_1.0.0_x64-setup.exe` | NSIS installer, auto-updates |
| macOS (Apple Silicon) | `4DA_1.0.0_aarch64.dmg` | M1/M2/M3/M4 |
| macOS (Intel) | `4DA_1.0.0_x64.dmg` | Intel Macs |
| Linux | `4DA_1.0.0_amd64.AppImage` | Most distros |
| Linux | `4da_1.0.0_amd64.deb` | Debian/Ubuntu |

CLI binaries (`4da-cli-*`) are also attached for headless / CI use.

### Verify your download

Every asset includes a SHA-256 checksum. Verify before running:

```bash
# All platforms — compare against SHASUMS256.txt
sha256sum -c SHASUMS256.txt
```

See [VERIFY-DOWNLOADS.md](https://github.com/runyourempire/4DA/blob/main/docs/VERIFY-DOWNLOADS.md) for full verification instructions including minisign signatures.

### Windows: first-run note

Because 4DA is a newly signed application, Windows SmartScreen may prompt on first launch. Click **More info > Run anyway**. SmartScreen reputation builds automatically as downloads accumulate.

## Quick Start

1. Download and install for your platform
2. Launch 4DA — it auto-discovers your local projects
3. See your first intelligence briefing within 60 seconds

No account required. No configuration needed. No sign-up.

### MCP Integration (Claude Code / Cursor / Windsurf)

```bash
npx @4da/mcp-server --setup
```

The MCP server works standalone — no desktop app required. 14 tools for vulnerability scanning, dependency health, decision memory, and more.

## Privacy

- **100% local.** Zero telemetry. No analytics. No tracking. No accounts.
- **BYOK** — bring your own API key (Anthropic, OpenAI, Google, or Ollama for fully offline use)
- API keys are stored in local config and never transmitted anywhere
- All content processing and scoring happens on your machine
- Full network transparency: [NETWORK-TRANSPARENCY.md](https://github.com/runyourempire/4DA/blob/main/docs/NETWORK-TRANSPARENCY.md)

## Pricing

| | Free | Signal ($12/mo or $99/yr) |
|---|---|---|
| Content sources | All 20+ | All 20+ |
| PASIFA scoring | Full engine | Full engine |
| AI briefings | Included | Included |
| STREETS Playbook | All 7 modules | All 7 modules |
| MCP tools | All 35 | All 35 |
| Developer DNA | — | Included |
| Signal Chains | — | Included |
| Knowledge Gaps | — | Included |
| Score Autopsy | — | Included |
| Natural Language Search | — | Included |

Free is not a demo. It's the full intelligence engine with 20+ sources, AI briefings, and MCP integration. Signal adds compound intelligence features for developers who want the system to learn faster.

**14-day free trial** of Signal — no credit card required.

## Build from Source

```bash
git clone https://github.com/runyourempire/4DA.git
cd 4DA && git checkout v1.0.0
pnpm install && pnpm run tauri build
```

See [BUILD-FROM-SOURCE.md](https://github.com/runyourempire/4DA/blob/main/docs/BUILD-FROM-SOURCE.md) for prerequisites and detailed instructions.

## Links

- Website: [4da.ai](https://4da.ai)
- Documentation: [github.com/runyourempire/4DA](https://github.com/runyourempire/4DA)
- MCP Server: [npmjs.com/package/@4da/mcp-server](https://www.npmjs.com/package/@4da/mcp-server)
- Security: security@4da.ai

## License

FSL-1.1-Apache-2.0 — source-available, converts to Apache 2.0 after two years.

---

4DA Systems Pty Ltd (ACN 696 078 841)
