---
layout: docs.njk
eyebrow: "Integrate"
title: "CLI — 4DA Docs"
description: "The 4DA command-line interface reads the same local database as the desktop app. No extra setup."
permalink: "/docs/cli/"
templateEngineOverride: md
---

# CLI

The `4da` command-line interface reads from the same SQLite database as the desktop app. If the app has run, the CLI works — no extra setup, no separate configuration.

## Commands

```bash
4da briefing               # Latest AI briefing
4da signals                # All classified signals
4da signals --critical     # Critical / high priority only
4da gaps                   # Knowledge gaps in your dependencies
4da health                 # Project dependency health
4da status                 # Database stats
```

## When to use it

The CLI is built for scripting and for agents that prefer a shell over MCP. Pipe `4da signals --critical` into a morning check, wire `4da health` into a pre-commit hook, or let an agent shell out to `4da briefing` for context.

Because it reads the local database directly, output reflects your latest analysis instantly — there's no server round-trip and nothing leaves the machine.

For richer, structured access from inside an AI tool, prefer the **[MCP server](/docs/mcp/)** — it exposes the same intelligence as typed tools.
