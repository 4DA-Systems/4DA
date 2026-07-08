---
layout: docs.njk
eyebrow: "How it works"
title: "Privacy & BYOK — 4DA Docs"
description: "4DA is local-first and direct-to-provider. No 4DA server, no account, no telemetry. Here's every outbound connection."
permalink: "/docs/privacy/"
templateEngineOverride: md
---

# Privacy & BYOK

4DA is **local-first and direct-to-provider**. There is no 4DA-operated server, no analytics, and no user account system. Your indexed content, scores, and intelligence live in a SQLite database on your machine.

## Every outbound connection

That's the whole promise, so here's the whole list:

| Category | Where | Why |
|---|---|---|
| Source adapters | HN, GitHub, Reddit, arXiv, etc. | Fetching the public content you configured |
| LLM providers | Anthropic / OpenAI / localhost Ollama | Only if *you* set up BYOK keys |
| License validation | Keygen | Only if you activated a paid license |
| Updater | GitHub Releases | Signed with minisign, once per session |
| Crash reports | **None** | 4DA sends none. Export a scrubbed diagnostic bundle locally, on demand. |

There is no 4DA telemetry endpoint because there is no 4DA cloud.

## Bring your own key

AI features are opt-in and run on compute you own:

- **Local (fully private):** [Ollama](https://ollama.com/) or a downloaded model runs everything offline. Nothing leaves the machine.
- **Cloud (your key):** add an Anthropic or OpenAI key for AI-written briefings and deeper reranking.

4DA never pays for your compute, never stores your keys on a remote server, and never makes an API call you didn't configure. Keys are held in your OS keychain, local to the device.

## Don't take our word for it

Because 4DA is source-available, the outbound behavior above is verifiable in code. The repository ships several trust documents — a network-transparency map of every connection with source references, a plain-language privacy summary, a security-audit guide to the trust-critical paths, and instructions to build from source and verify the binary yourself.

Read them in the [repository](https://github.com/4DA-Systems/4DA). Local-first means you don't have to trust us — you can check.

Next: **[MCP server & agents](/docs/mcp/)**.
