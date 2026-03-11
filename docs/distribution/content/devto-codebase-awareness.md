---
title: "Why Your AI Coding Assistant Doesn't Know What You Build (And How to Fix It)"
published: false
description: "AI assistants have no awareness of your tech stack, dependencies, or past decisions. Here's how codebase-aware intelligence changes what's possible."
tags: ai, developer-experience, productivity, mcp
canonical_url: https://4da.ai/blog/codebase-awareness
cover_image:
series:
---

Ask Claude to recommend a caching library. It will suggest Redis, Memcached, and maybe Dragonfly. Reasonable answers. But it does not know you are building a Tauri desktop app with a Rust backend and SQLite already in your dependency tree. It does not know you rejected Redis three months ago because you committed to a local-first architecture. It does not know your `Cargo.toml` already lists `moka` as a dependency.

The recommendation is technically correct and contextually useless.

This is the codebase awareness gap: AI coding assistants operate in a vacuum. They know the current file, maybe the current conversation, but nothing about the project they are helping you build.

## What the Gap Actually Costs

The cost is not wrong answers. Models are generally good at generating correct code. The cost is irrelevant answers that waste decision-making energy.

A developer working on a Go microservice sees a Hacker News thread about a critical vulnerability in `net/http`. That is directly relevant — `net/http` is in their import graph. But their AI assistant surfaces it alongside 50 other trending posts with no way to distinguish signal from noise.

A team decides to standardize on Fastify over Express. Three weeks later, a team member asks their AI assistant for help with a Node.js API endpoint. The assistant suggests Express middleware patterns because it has no knowledge of the decision. The developer either overrides the suggestion (burning context switches) or does not notice the conflict (introducing architectural drift).

These are not hypothetical problems. They are the default experience.

## What "Codebase-Aware" Means

Codebase awareness is not "read the current file." It is a layered understanding of what a developer works on:

**Layer 1: Manifest scanning.** Parse `Cargo.toml`, `package.json`, `go.mod`, `requirements.txt`, `pyproject.toml`, `composer.json`, `Gemfile`, `pom.xml`, `build.gradle`, `CMakeLists.txt`, `.csproj`, and `pubspec.yaml`. Extract dependencies, their versions, and the language ecosystem. This is the static skeleton of a project.

**Layer 2: Technology detection.** Go beyond declared dependencies. Detect frameworks from import patterns, configuration files, and directory structure. A project with a `next.config.js` is a Next.js project regardless of what its `package.json` description says.

**Layer 3: Active topics.** Monitor what a developer is actually working on through git activity, recently modified files, and file content analysis. Someone committing changes to authentication middleware has different information needs than someone refactoring a build pipeline.

**Layer 4: Learned preferences.** Track what content a developer engages with over time. Clicks, saves, and dismissals build a behavioral model that improves relevance scoring without explicit configuration.

**Layer 5: Decision memory.** Record and enforce architectural decisions. "We chose Tauri over Electron because of binary size constraints" is context that should persist across sessions and influence future suggestions.

Each layer adds resolution. A security advisory for `lodash` is noise for a Rust developer. A breaking change in `tokio` is critical for one. The layers make that distinction automatic.

## How This Works in Practice

The `@4da/mcp-server` package exposes 30 tools to any MCP-compatible AI assistant (Claude Code, Cursor, Windsurf, VS Code Copilot). These tools read from a local SQLite database populated by the 4DA desktop app, which runs the scanning and scoring in the background.

Here is what becomes possible:

### Security advisories that actually matter

The `knowledge_gaps` tool cross-references your project dependencies against content mentioning those packages. If a CVE is published for a library in your `Cargo.lock`, it surfaces as a critical knowledge gap — not because you searched for it, but because the system knows your dependency graph.

```
> knowledge_gaps({ min_severity: "critical" })

{
  "gaps": [{
    "dependency": "rustls",
    "version": "0.21.8",
    "project_path": "/home/dev/myproject",
    "missed_items": [{
      "title": "CVE-2024-XXXX: rustls certificate validation bypass",
      "source_type": "hackernews",
      "created_at": "2024-12-15 08:30:00"
    }],
    "gap_severity": "critical"
  }],
  "total_dependencies": 142,
  "gaps_found": 1
}
```

You did not subscribe to a security feed. You did not configure alerts. The system read your `Cargo.toml`, tracked the dependency, and matched it against incoming content.

### A personal tech radar that builds itself

The `tech_radar` tool synthesizes a ThoughtWorks-style radar from four data sources:

- **Adopt ring:** Technologies in your declared tech stack (used daily)
- **Trial ring:** Notable project dependencies (in use, not core identity)
- **Assess ring:** Auto-detected technologies from project scans (seen but not committed to)
- **Hold ring:** Alternatives explicitly rejected in developer decisions

```
> tech_radar({ ring: "hold" })

{
  "entries": [{
    "name": "electron",
    "ring": "hold",
    "quadrant": "frameworks",
    "source": "decision_hold",
    "decision_ref": 12
  }]
}
```

The radar updates automatically as your projects evolve. No manual curation. The "hold" ring is particularly useful — it prevents AI assistants from re-suggesting technologies you have already evaluated and rejected.

### Decision memory that persists across sessions

The `decision_memory` tool records architectural decisions with rationale and rejected alternatives:

```
> decision_memory({
    action: "record",
    decision_type: "tech_choice",
    subject: "HTTP client",
    decision: "Use reqwest with rustls",
    rationale: "Pure Rust TLS stack, no OpenSSL dependency. Important for cross-compilation.",
    alternatives_rejected: ["ureq", "hyper-direct", "reqwest-with-openssl"]
  })
```

Later, when you or an AI assistant considers `ureq` for a new HTTP endpoint, the `check_decision_alignment` tool flags the conflict:

```
> check_decision_alignment({ technology: "ureq" })

{
  "aligned": false,
  "conflicts": [{
    "decision_id": 7,
    "subject": "HTTP client",
    "reason": "'ureq' was rejected in favor of 'Use reqwest with rustls'
               (rationale: Pure Rust TLS stack, no OpenSSL dependency)"
  }]
}
```

This is institutional memory for solo developers. Decisions made at 2 AM do not get relitigated at 10 AM because the context was lost between sessions.

### Content scoring against your actual stack

The `get_actionable_signals` tool classifies incoming content (from Hacker News, arXiv, Reddit, GitHub, RSS feeds) into six signal types: security alerts, breaking changes, tool discoveries, tech trends, learning resources, and competitive intelligence.

The priority of each signal is boosted when it matches your detected tech stack. A generic "New Rust framework released" post gets medium priority. The same post gets high priority if it is a framework in your dependency tree. A security advisory for a package in your `Cargo.lock` gets critical priority.

```
> get_actionable_signals({ priority_filter: "critical" })

{
  "signals": [{
    "title": "Breaking change in tokio 2.0 — runtime API redesign",
    "signal_type": "breaking_change",
    "signal_priority": "critical",
    "action": "Check migration path — tokio breaking change",
    "triggers": ["breaking change", "major release"],
    "confidence": 0.87
  }]
}
```

The classification uses keyword pattern matching cross-referenced against your stack. No LLM call required for the base classification — it runs in milliseconds against the local database.

## What This Is Not

This is not a replacement for reading documentation, subscribing to release notes, or thinking about architecture. It is a filter. The internet produces an enormous volume of developer-relevant content every day. Most of it is irrelevant to any individual developer's work. Codebase-aware intelligence reduces the search space to what actually matters for the projects you are building.

It is also not a cloud service. The 4DA desktop app runs locally. The SQLite database lives on your machine. The MCP server reads from that local database. Project manifests, dependency lists, and engagement data never leave the machine. The optional LLM synthesis layer supports Ollama for fully local inference.

## Setting It Up

```bash
npx @4da/mcp-server --setup
```

This detects your installed editors (Claude Code, Cursor, VS Code, Windsurf) and writes the MCP configuration automatically. Verify with:

```bash
npx @4da/mcp-server --doctor
```

The doctor checks Node.js version, native SQLite bindings, database availability, and LLM provider configuration. Every check returns a pass, warning, or failure with an actionable next step.

The MCP server requires the [4DA desktop app](https://github.com/runyourempire/4DA/releases/latest) to populate the database. Install 4DA, point it at your project directories, and let it run a scan. The MCP server reads the results.

The full tool inventory:

- **Core:** Query content feed, get user context, explain relevance scores, record feedback
- **Intelligence:** Daily briefings, actionable signals, trend analysis, topic knowledge graphs, signal chains, semantic shifts, attention reports, score forensics, context optimization
- **Diagnostic:** Source health, config validation, LLM status
- **Knowledge:** Knowledge gaps, project health, reverse mentions, context export
- **Decisions:** Decision memory, personal tech radar, alignment checks
- **Agent:** Persistent cross-session memory, session startup briefs, delegation scoring
- **Identity:** Developer DNA profile export
- **Metabolism:** Intelligence autophagy status, decision windows, compound advantage scoring

## The Broader Point

The MCP specification exists so AI tools can access structured data. Most MCP servers connect to external services — Slack, GitHub, databases. That is useful, but it misses the most valuable data source: the developer's own machine.

Your project manifests, dependency trees, git history, and past decisions contain more context about what you need than any external API. Making that context available to AI assistants through a standard protocol turns generic suggestions into relevant ones.

The gap between "technically correct" and "useful for this developer on this project" is the codebase awareness gap. Closing it does not require better models. It requires better context.

---

*4DA Systems builds privacy-first developer intelligence tools. `@4da/mcp-server` is open source at [github.com/runyourempire/4DA](https://github.com/runyourempire/4DA).*
