---
layout: docs.njk
eyebrow: "Start"
title: "4DA Docs — Overview"
description: "4DA knows every dependency on your machine and tells you — and your coding agent — what changed and exactly what to run. Privately, locally. Start here."
permalink: "/docs/"
templateEngineOverride: md
---

# Overview

**4DA knows every dependency on your machine and tells you — and your coding agent — what changed and exactly what to run. Privately, locally.**

It reads npm, pnpm, Yarn, Bun, Cargo, Python and Go lockfiles today, with early support for Ruby and PHP; Maven, Gradle and .NET projects are not covered yet.

It reads the lockfiles across all your projects, checks the exact installed versions against OSV advisories and registry releases, and names the fix: the version that clears each advisory and, where the lockfile shows it, whether a lockfile refresh is enough or which parent package to upgrade. No account, and your source code never leaves the machine.

- **Preemption** is the worklist: every version-confirmed finding, scoped by dev-only, transitive and per-platform reachability, with the version that fixes it.
- **The Brief** answers "anything I need to do today?" from the same facts.
- **The MCP server** gives your coding agent the same answers before it touches a dependency: what an upgrade changes, which of your files import the package, which vulnerabilities a bump fixes.

4DA also reads developer news (Hacker News, Reddit, RSS and 20+ other sources) and scores it against your stack. That reading is a supporting extra, not the product; [the scoring engine](/docs/how-it-works/) explains how it works.

## Two ways to run it

You don't have to install a desktop app to get value on day one.

| | What it is | Setup |
|---|---|---|
| **MCP server** | A command-line server that plugs 4DA's dependency answers into Claude Code, Cursor, Windsurf, or any MCP client. 11 of its 16 tools work without the app | `npx @4da/mcp-server` — no keys, no account |
| **Desktop app** | The full Tauri app: Preemption, the Brief, the Signal stream, Blind Spots | Download for Windows, macOS, or Linux |

When both are installed, the MCP server reads the app's local database. Start with either.

## Where to go next

- **[Install](/docs/install/)** — download the app or run the MCP server
- **[Quickstart](/docs/quickstart/)** — from first launch to your first findings and Brief
- **[MCP server & agents](/docs/mcp/)** — the tools your coding agent gets
- **[Privacy & BYOK](/docs/privacy/)** — why local-first means you don't have to trust us

> **All signal. No feed.** 4DA is source-available under [FSL-1.1-Apache-2.0](https://github.com/4DA-Systems/4DA) and converts to Apache 2.0 three years after each release.
