---
layout: docs.njk
eyebrow: "Start"
title: "Quickstart — 4DA Docs"
description: "From install to your first scored feed in a few minutes."
permalink: "/docs/quickstart/"
templateEngineOverride: md
---

# Quickstart

This walks the desktop app from first launch to a feed scored against your own stack. For the CLI path, see the **[MCP server](/docs/mcp/)**.

## 1. Point 4DA at your code

On first launch, add the project directories you actually work in. 4DA's context engine (ACE) scans them — manifests like `Cargo.toml`, `package.json`, and `go.mod`, plus recent Git history — to learn your stack, your dependencies, and what you've been touching lately.

Nothing about your code leaves the machine. The scan runs locally and populates a SQLite database on disk.

## 2. Choose how you want AI (optional)

4DA works out of the box with private, on-device semantic search — no key required. You can go further:

- **Local AI (fully private):** point 4DA at [Ollama](https://ollama.com/) for free local inference.
- **Bring your own key:** add an Anthropic or OpenAI key for AI-written briefings and deeper reranking.

You own the compute. 4DA never pays for it, never stores your keys remotely, and never makes a call you didn't configure. See **[Privacy & BYOK](/docs/privacy/)**.

## 3. Run an analysis

Trigger a run from the top bar. 4DA fetches from your configured [sources](/docs/sources/), scores everything against your context, and keeps only what passes the confirmation gate. Background monitoring can keep it fresh on an interval you set.

## 4. Read the four tabs

| Tab | What it answers |
|---|---|
| **Brief** | What are today's top picks, scored against my stack? |
| **Preemption** | What's coming that I'll have to act on — CVEs, breaking changes, dependency risk? |
| **Blind Spots** | What high-relevance items did I never see, and where's my coverage thin? |
| **Signal** | Which items earned their place through 2+ independent axes? |

## 5. Teach it

Save what's useful, dismiss what isn't. Saves boost the relevant topics and lift that source's reputation; dismissals form anti-patterns that suppress similar noise later. The engine sharpens against your real judgment — this is the part that compounds.

Next: **[The scoring engine](/docs/how-it-works/)**.
