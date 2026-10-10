---
layout: docs.njk
eyebrow: "Integrate"
title: "MCP server & agents — 4DA Docs"
description: "Plug 4DA into Claude Code, Cursor, Windsurf, or any MCP client with one command. 16 tools, 11 standalone."
permalink: "/docs/mcp/"
templateEngineOverride: md
---

# MCP server & agents

The fastest way to try 4DA is to plug it into the AI tools you already use. One command:

```bash
npx @4da/mcp-server
```

This scans your project, detects your stack, and gives your assistant live vulnerability scanning, dependency health, upgrade planning, and ecosystem intelligence. No API keys. No accounts. Works standalone — no desktop app required.

Requires **Node.js 22.13 or newer** (it uses Node's built-in SQLite, so nothing native is compiled on install). On Node 22.0–22.12 the server only works fully if the optional `better-sqlite3` module can build on your machine.

## Connect your client

Point any MCP-compatible client — Claude Code, Cursor, Windsurf, VS Code (Copilot) — at the server. A typical config:

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

## The 16 tools

**11 tools work standalone**, with zero setup — upgrade impact (what changes between your version and the next, and which of your files it touches), pre-install dependency checks, vulnerability scanning, dependency health, upgrade planning, ecosystem news, pre-task briefings, project context, decision memory, alignment checking against your past decisions, and cross-session agent memory.

**5 more activate with the desktop app** — your scored content feed, actionable signals, knowledge gaps, feedback learning, and Developer DNA.

Every tool reliably returns useful data; none are stubs. The standalone tools mean an agent gets real value the moment it connects, before you've installed anything else.

## Why an agent wants this

- **Security in the loop** — the agent can check *your* dependencies against CVE/OSV before it suggests an upgrade.
- **Persistent memory** — decisions and context carry across sessions and across tools, so the agent doesn't relearn your project every morning.
- **Pre-task briefs** — tailored startup context for whatever you're about to work on.

Next: **[CLI](/docs/cli/)**.
