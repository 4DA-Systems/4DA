---
layout: docs.njk
eyebrow: "Start"
title: "4DA Docs — Overview"
description: "4DA reads the internet for developers — privately, locally. Your codebase decides what's relevant. Start here."
permalink: "/docs/"
templateEngineOverride: md
---

# Overview

**4DA reads the internet for developers — privately, locally. Your codebase decides what's relevant.**

It scans your codebase — `Cargo.toml`, `package.json`, `go.mod`, Git history — and scores every article, advisory, and release from 20+ sources against what you actually build. An item needs **2 or more independent signals** to survive. Everything else is rejected.

Benchmarked across 9 developer personas against a 245-item labeled corpus: **93% of content is filtered as noise, 98.9% of actual noise is correctly rejected.** Measured, not asserted — [see the method](/docs/scoring/). Your real rejection rate — computed from your own data — is shown in the app.

Saves and dismissals build a preference profile you can inspect, pin, or forget — and teach the Brief what to stop showing you. Relevance scoring itself stays grounded in your actual stack. When the engine improves it re-judges everything it already holds: yesterday's noise becomes tomorrow's signal.

## Two ways to run it

You don't have to install a desktop app to get value on day one.

| | What it is | Setup |
|---|---|---|
| **MCP server** | A command-line server that plugs 4DA's intelligence into Claude Code, Cursor, Windsurf, or any MCP client | `npx @4da/mcp-server` — no keys, no account |
| **Desktop app** | The full Tauri app: scored feed, briefings, radar, blind spots, Learned Preferences | Download for Windows, macOS, or Linux |

The MCP server and the desktop app read from the same local database. Start with either.

## Where to go next

- **[Install](/docs/install/)** — download the app or run the MCP server
- **[Quickstart](/docs/quickstart/)** — from install to your first scored feed
- **[The scoring engine](/docs/how-it-works/)** — how an item earns its place
- **[Privacy & BYOK](/docs/privacy/)** — why local-first means you don't have to trust us

> **All signal. No feed.** 4DA is source-available under [FSL-1.1-Apache-2.0](https://github.com/4DA-Systems/4DA) and converts to Apache 2.0 three years after each release.
