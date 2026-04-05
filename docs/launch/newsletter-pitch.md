# Newsletter Pitch Template

Customizable 3-paragraph pitch for newsletter outreach. Adjust paragraph 3 per newsletter.

---

## Subject Line Options

- "Desktop app that scores developer content against your actual codebase"
- "4DA: 5-axis relevance scoring for developer content (Rust + Tauri)"
- "Open-source tool that rejects 99% of developer content as noise"

---

## Base Template

```
Hi [NAME],

4DA is a desktop app that scans your local projects and scores developer content from 20+ sources (HN, GitHub, Reddit, arXiv, CVE databases, package registries, and more) against your actual codebase. It runs locally, collects zero telemetry, requires no account, and the full scoring engine is free.

What makes it different from feed readers or personalized news tools: 4DA uses a 5-axis confirmation gate that evaluates content across context match, interest alignment, codebase relevance, dependency impact, and learned preferences. An item needs 2+ independent signals to pass -- a single signal gets capped and rejected. The result is a 99% rejection rate. What survives is genuinely relevant to what you are building, not what you clicked on last week. Built with Rust and Tauri 2.0, 3,639 tests passing, 13 languages supported, FSL-1.1-Apache-2.0 licensed.

[PARAGRAPH 3 -- customize per newsletter below]

Website: https://4da.ai
MCP server (33 tools): npx @4da/mcp-server

Best,
[YOUR NAME]
```

---

## Paragraph 3 Customizations

### This Week in Rust

```
I think your readers would find the Rust implementation details interesting. The backend is pure Rust (Tauri 2.0), using sqlite-vec for KNN vector search, a custom scoring DSL with tunable parameters, local embeddings via Ollama, and a macro-generated 5-axis confirmation gate. The codebase has 2,215 Rust tests. The scoring pipeline alone is ~800 lines across 8 phases. If you are looking for Show-and-Tell submissions or project spotlights, I would be happy to write up the technical details -- particularly the confirmation gate architecture and how we use procedural macros to generate the axis evaluation.
```

### Console.dev

```
4DA fits the Console model well: it is a developer tool that does one thing rigorously. No cloud dependency, no account, no tracking. It auto-discovers your stack from manifest files (Cargo.toml, package.json, go.mod, requirements.txt, and 30+ others), builds a local vector profile, and scores content against it. The free tier includes every source and the full scoring engine -- Signal ($12/mo) adds compound intelligence features like Developer DNA, Signal Chains, and Knowledge Gaps. It also ships a 33-tool MCP server on npm (@4da/mcp-server) that plugs into Claude Code and Cursor, so AI coding assistants can query the scored feed directly.
```

### TLDR

```
For a TLDR mention, the core pitch: 4DA is a free, privacy-first desktop app (Rust + Tauri) that scores developer content from 20+ sources against your actual codebase using 5-axis relevance scoring. 99% rejection rate. Zero telemetry. Works offline via Ollama. Ships a 33-tool MCP server for AI coding assistants. Signal tier ($12/mo) adds compound intelligence. FSL-1.1-Apache-2.0 licensed. Available for Windows, macOS, and Linux at https://4da.ai.
```

### Changelog

```
4DA touches a few angles your audience cares about: the tension between useful AI and privacy, the resurgence of local-first tools, and the MCP ecosystem growing around AI coding assistants. The app is BYOK (bring your own API key for OpenAI, Anthropic, or OpenRouter) or fully offline with Ollama. No data ever leaves the machine. The MCP server exposes 33 tools -- developers using Claude Code or Cursor can query their scored feed, check dependency advisories, and surface signal chains without leaving their editor. Happy to come on the podcast or write a guest post about scoring architecture, privacy-first design in the LLM era, or building with Tauri 2.0.
```

### Hacker Newsletter

```
This started as a personal tool after I tracked my HN consumption for a month and found that 12 out of 847 articles were relevant to my active projects. 4DA now monitors HN (among 20+ sources) and scores every submission against the user's local codebase. The 5-axis confirmation gate means trending-but-irrelevant posts get filtered, while a niche library update that affects your dependencies gets surfaced. It is the kind of tool HN readers build for themselves -- except this one ships with 3,639 tests, 13 language translations, and an MCP server. Free tier includes everything needed to use it seriously.
```

---

## Outreach Notes

- **Personalize the subject line.** Reference a recent issue or article from their newsletter that connects to 4DA's angle (privacy, Rust tooling, developer productivity, MCP/AI).
- **Keep it short.** Newsletter editors get dozens of pitches. The 3-paragraph structure respects their time.
- **Do not oversell.** Let the specifics (99% rejection, 5-axis gate, 3,639 tests, zero telemetry) do the work. No superlatives.
- **Offer assets.** Screenshots, technical write-ups, or a 15-minute call. Make their job easier.
- **Follow up once.** If no reply in 7 days, one follow-up referencing a specific detail from their newsletter. Then stop.
