---
layout: docs.njk
eyebrow: "How it works"
title: "Privacy & BYOK — 4DA Docs"
description: "4DA is local-first and direct-to-provider. No account, no telemetry, and your source code never leaves the machine. Here's every outbound connection."
permalink: "/docs/privacy/"
templateEngineOverride: md
---

# Privacy & BYOK

4DA is **local-first and direct-to-provider**. It has no analytics, no telemetry, no crash reporting and no user accounts, and 4DA runs no server that receives your projects, scores or reading. Your indexed content, scores, and intelligence live in a SQLite database on your machine. Your source code and file contents never leave it (one opt-in exception, cloud embeddings, is described in [NETWORK.md §2a](https://github.com/4DA-Systems/4DA/blob/main/NETWORK.md#2a-byok-cloud-llm-relevance-judging)).

## Outbound traffic, in summary

| Category | Where | What it receives |
|---|---|---|
| Dependency checks | OSV.dev; npm, crates.io, PyPI and the Go module proxy | The package names and versions from your lockfiles (OSV.dev gets installed versions; registries get names). Go module paths are sent in full, so a private module path such as `github.com/yourco/internal-svc` names that repository. Never your code. |
| News sources | HN, GitHub, Reddit, arXiv, RSS and the others you enable | Plain requests for public content; Reddit and Stack Overflow see the subreddits and tags you follow |
| AI provider | Anthropic / OpenAI / any OpenAI-compatible endpoint, or local Ollama | Only if you set one up with your own key: item titles and snippets plus a short context profile (dependency names, project labels, git-derived topics, interests). File paths are replaced with project labels; source code and commit messages are never sent |
| Licence | Keygen (`api.keygen.sh`) and 4DA's own `4da.ai` | Paid licences only: the key, to validate it and, for subscriptions, to renew it near expiry. `4da.ai` also receives your email if you use "Recover License by Email" |
| Updater | GitHub Releases | A plain request for the update manifest, once per session; updates are minisign-verified |
| One-time downloads | Hugging Face, GitHub | The local search model, only when a feature first needs it |
| Crash reports | **None** | 4DA sends none. Export a scrubbed diagnostic bundle locally, on demand. |

[NETWORK.md](https://github.com/4DA-Systems/4DA/blob/main/NETWORK.md) is the complete, source-referenced list, including the opt-in flows (cloud translation, digest email, GitHub release notes) and the local-only Signal Terminal port (`127.0.0.1:4446`). On Windows, the Microsoft WebView2 runtime that draws the window also contacts Microsoft on its own; [NETWORK.md §5](https://github.com/4DA-Systems/4DA/blob/main/NETWORK.md#5-not-4das-code-the-windows-webview-runtime) explains what it is and how to limit it.

## Bring your own key

AI features are opt-in and run on compute you own:

- **Local (fully private):** [Ollama](https://ollama.com/) or a downloaded model runs the AI features offline; nothing you ask it leaves the machine. The dependency checks above still go to OSV.dev and the registries (offline, 4DA shows cached data).
- **Cloud (your key):** add an Anthropic or OpenAI key for AI-written briefings and deeper reranking.

4DA never pays for your compute and never stores your keys on a remote server. Keys are held in your OS keychain, local to the device, and sent only to the provider they belong to.

## Don't take our word for it

Because 4DA is source-available, the outbound behavior above is verifiable in code. The repository ships several trust documents — a network-transparency map of every connection with source references, a plain-language privacy summary, a security-audit guide to the trust-critical paths, and instructions to build from source and verify the binary yourself.

Read them in the [repository](https://github.com/4DA-Systems/4DA). Local-first means you don't have to trust us — you can check.

Next: **[MCP server & agents](/docs/mcp/)**.
