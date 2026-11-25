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
| **Linux** | [`.AppImage` / `.deb`](https://github.com/4DA-Systems/4DA/releases/latest) | Notified; you install the new package |

Every release publishes `SHASUMS256.txt` and per-file `.sha256` sidecars so you can verify the binary before you run it. The app checks GitHub Releases for a new version a few seconds after it starts, every 6 hours while it runs, and when you come back to its window. On Windows and macOS it downloads the update (showing progress), verifies its minisign signature and restarts into the new version. On Linux it tells you a new version is out and links to the release, because a `.deb` or `.rpm` install cannot be replaced from inside the app.

> **Windows users:** the Windows installer is not yet code-signed, so SmartScreen shows "Windows protected your PC / Unknown publisher". Check the installer's SHA-256 against `SHASUMS256.txt` first, then click **More info → Run anyway**. Code signing is in progress.

## Or run the MCP server

Already using Claude Code, Cursor, or Windsurf? One command gives your AI assistant live vulnerability scanning, dependency health, and ecosystem intelligence:

```bash
npx @4da/mcp-server
```

No API keys. No accounts. No desktop app required. See **[MCP server & agents](/docs/mcp/)**.

## System requirements

4DA runs on modest hardware. The dependency engine needs no GPU, no API key and nothing beyond the installer. On-device semantic search (interests and natural-language search) downloads its embedding model once, about 260 MB, the first time you turn it on; if Ollama is running, it can serve the embeddings instead.

| | Baseline (free) | + Cloud AI (BYOK) | + Local AI (offline) |
|---|---|---|---|
| RAM | 4 GB | 4 GB | 16 GB (8B model); 32 GB for 12–14B |
| CPU | any 64-bit | any 64-bit | 6–8 cores |
| GPU | not needed | not needed | optional (faster) |
| Disk | several GB (grows with use) | several GB (grows with use) | + 5–9 GB per model |

Disk use grows with your corpus: an active install's database reaches a couple of GB, each schema migration keeps up to 2 full database backups, and caches and models add about 1 GB. The data-retention setting (default 30 days, adjustable 7–365) bounds how long items are kept.

Supported: Windows 10 (1803+) / 11, macOS 10.15+, Ubuntu 22.04+ (WebKitGTK 4.1). Installers are about 40–50 MB; the Linux AppImage is about 120 MB because it carries its own WebKitGTK. The local embedding model is not in the installer: it is downloaded once (about 260 MB, every file checked against a pinned SHA-256) only when you turn on interests or run a natural-language search. Everything the dependency engine does works without it.

> **Linux, v1.0.4:** if 4DA exits with "Missing required system libraries", the check that prints it is wrong in two cases, both fixed in the next release. The `.deb` on Debian: start it with `PATH="$PATH:/usr/sbin:/sbin" fourda`. The AppImage on a system without WebKitGTK: `sudo apt install libwebkit2gtk-4.1-0` (Fedora: `sudo dnf install webkit2gtk4.1`). Details: [LINUX.md](https://github.com/4DA-Systems/4DA/blob/main/LINUX.md).

## Build from source

```bash
git clone https://github.com/4DA-Systems/4DA.git
cd 4DA
pnpm install
pnpm tauri dev   # First build: 5–15 min. Dev server: localhost:4444.
```

**Prerequisites:** Rust 1.95.0 (pinned in `src-tauri/rust-toolchain.toml`), Node.js 22, pnpm 10.34.6 (pinned by `packageManager`; `corepack enable` picks it up). Windows also needs VS Build Tools 2022 with the C++ workload.

Next: **[Quickstart](/docs/quickstart/)**.
