---
layout: docs.njk
eyebrow: "Integrate"
title: "MCP server & agents — 4DA Docs"
description: "Plug 4DA into Claude Code, Cursor, Windsurf, or any MCP client with one command. 14 tools, 9 standalone."
permalink: "/docs/mcp/"
templateEngineOverride: md
---

# MCP server & agents

The fastest way to try 4DA is to plug it into the AI tools you already use. One command:

```bash
npx @4da/mcp-server
```

This scans your project, detects your stack, and gives your assistant live vulnerability scanning, dependency health, upgrade planning, and ecosystem intelligence. No API keys. No accounts. Works standalone — no desktop app required.

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

## The 14 tools

**9 tools work standalone**, with zero setup — vulnerability scanning, dependency health, upgrade planning, ecosystem news, pre-task briefings, decision memory, and cross-session agent memory.

**5 more activate with the desktop app** — your scored content feed, actionable signals, knowledge gaps, feedback learning, and Developer DNA.

Every tool reliably returns useful data; none are stubs. The standalone tools mean an agent gets real value the moment it connects, before you've installed anything else.

## Why an agent wants this

- **Security in the loop** — the agent can check *your* dependencies against CVE/OSV before it suggests an upgrade.
- **Persistent memory** — decisions and context carry across sessions and across tools, so the agent doesn't relearn your project every morning.
- **Pre-task briefs** — tailored startup context for whatever you're about to work on.

Next: **[CLI](/docs/cli/)**.
