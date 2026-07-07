---
layout: docs.njk
eyebrow: "Start"
title: "Install — 4DA Docs"
description: "Download 4DA for Windows, macOS, or Linux, run the MCP server, or build from source."
permalink: "/docs/install/"
templateEngineOverride: md
---

# Install

## Download the app

Pre-built binaries — no Rust toolchain required.

| Platform | Download | Auto-updates |
|---|---|:---:|
| **Windows** | [`.exe` installer](https://github.com/runyourempire/4DA/releases/latest) | Yes |
| **macOS** | [`.dmg` (Apple Silicon & Intel)](https://github.com/runyourempire/4DA/releases/latest) | Yes |
| **Linux** | [`.AppImage` / `.deb`](https://github.com/runyourempire/4DA/releases/latest) | Yes |

Every release publishes `SHASUMS256.txt` and per-file `.sha256` sidecars so you can verify the binary before you run it. The updater checks GitHub Releases once per session and validates signatures with minisign.

> **Windows users:** SmartScreen prompts on first launch — this is a new application still building reputation, not a warning about the binary. Click **More info → Run anyway**.

## Or run the MCP server

Already using Claude Code, Cursor, or Windsurf? One command gives your AI assistant live vulnerability scanning, dependency health, and ecosystem intelligence:

```bash
npx @4da/mcp-server
```

No API keys. No accounts. No desktop app required. See **[MCP server & agents](/docs/mcp/)**.

## System requirements

4DA runs on modest hardware. Private, on-device semantic search is built in — no GPU, no API key, and no first-run download required.

| | Baseline (free) | + Cloud AI (BYOK) | + Local AI (offline) |
|---|---|---|---|
| RAM | 4 GB | 4 GB | 16 GB (8B model); 32 GB for 12–14B |
| CPU | any 64-bit | any 64-bit | 6–8 cores |
| GPU | not needed | not needed | optional (faster) |
| Disk | ~500 MB | ~500 MB | + 5–9 GB per model |

Supported: Windows 10 (1803+) / 11, macOS 10.15+, Ubuntu 22.04+ (WebKitGTK 4.1). One installer is ~110 MB; the local embedding model ships inside it and works fully offline on first run.

## Build from source

```bash
git clone https://github.com/runyourempire/4DA.git
cd 4DA
pnpm install
pnpm tauri dev   # First build: 5–15 min. Dev server: localhost:4444.
```

**Prerequisites:** Rust (pinned via `rust-toolchain.toml`), Node.js 20, pnpm 9.15. Windows also needs VS Build Tools 2022 with the C++ workload.

Next: **[Quickstart](/docs/quickstart/)**.
