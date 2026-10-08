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
| **Windows** | [`.exe` installer](https://github.com/4DA-Systems/4DA/releases/latest) | Yes |
| **macOS** | [`.dmg` (Apple Silicon & Intel)](https://github.com/4DA-Systems/4DA/releases/latest) | Yes |
| **Linux** | [`.AppImage` / `.deb`](https://github.com/4DA-Systems/4DA/releases/latest) | Yes |

Every release publishes `SHASUMS256.txt` and per-file `.sha256` sidecars so you can verify the binary before you run it. The updater checks GitHub Releases once per session and validates signatures with minisign.

> **Windows users:** the Windows installer is not yet code-signed, so SmartScreen shows "Windows protected your PC / Unknown publisher". Check the installer's SHA-256 against `SHASUMS256.txt` first, then click **More info → Run anyway**. Code signing is in progress.

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
| Disk | several GB (grows with use) | several GB (grows with use) | + 5–9 GB per model |

Disk use grows with your corpus: an active install's database reaches a couple of GB, each schema migration keeps up to 2 full database backups, and caches and models add about 1 GB. The data-retention setting (default 30 days, adjustable 7–365) bounds how long items are kept.

Supported: Windows 10 (1803+) / 11, macOS 10.15+, Ubuntu 22.04+ (WebKitGTK 4.1). Installers are about 280–300 MB on Windows and macOS and about 370 MB for the Linux AppImage. Most of that is the local embedding model, which ships inside the installer and works fully offline on first run.

## Build from source

```bash
git clone https://github.com/4DA-Systems/4DA.git
cd 4DA
pnpm install
pnpm tauri dev   # First build: 5–15 min. Dev server: localhost:4444.
```

**Prerequisites:** Rust (pinned via `rust-toolchain.toml`), Node.js 20, pnpm 9.15. Windows also needs VS Build Tools 2022 with the C++ workload.

Next: **[Quickstart](/docs/quickstart/)**.
