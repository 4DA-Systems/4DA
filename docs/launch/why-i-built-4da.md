# Why I Built 4DA

*The story behind a desktop app that scores developer content against your actual codebase.*

---

I used to start every morning the same way. Open Hacker News. Scroll through 30 links. Click on 4-5. Realize 3 of them were irrelevant to anything I'm actually building. Close the tabs. Repeat on Reddit, DEV.to, and Twitter. Thirty minutes gone. Maybe one useful article found.

The problem isn't that good content doesn't exist. It does. The problem is that **relevance is personal** — what matters to you depends on what you're building, what stack you use, what dependencies you have, and what you already know. Generic feeds can't know any of that.

So I built 4DA.

## What It Actually Does

4DA is a desktop app that scores developer content from 20+ sources — Hacker News, GitHub, Reddit, arXiv, Stack Overflow, CVE databases, package registries, and more — against your **actual codebase**.

When you point 4DA at your project directories, it scans your `package.json`, `Cargo.toml`, `.csproj`, and `requirements.txt`. It detects your tech stack, your dependencies, your active branches. Then it evaluates every article, release, advisory, and discussion against what you actually use.

The scoring pipeline uses 5 independent axes:

1. **Context** — Does it match code you're actively writing?
2. **Interest** — Does it match topics you've declared or explored?
3. **Tech Stack** — Is it about technology you actually use?
4. **Learned Behavior** — Based on what you've saved, dismissed, and clicked before
5. **Dependencies** — Does it affect packages in your lock files?

An item needs **2 or more** of these signals to pass. Everything else gets rejected. In benchmark testing across 9 developer personas with 215 labeled items: 92% overall rejection rate, 98% of actual noise correctly identified.

That means from 500 articles across 20 sources, you might see 5-8 that genuinely matter to you. Not because an algorithm wants your engagement — because they actually affect your work.

## Why Desktop, Why Local, Why Now

I could have built this as a web app. It would have been easier to distribute, easier to monetize, easier to grow. But it would have required sending your codebase metadata to a server. Your dependency lists. Your git history. Your tech stack.

I wasn't willing to do that. Not because I think cloud services are evil — I use them every day. But because a tool that knows your entire development context is a tool that should run on YOUR machine, under YOUR control.

4DA runs 100% locally. Zero telemetry. Zero analytics. Zero user accounts. Your data never leaves your computer. The scoring pipeline, the content fetching, the AI briefings — all local.

For AI features, you bring your own key (Anthropic, OpenAI) or use Ollama for fully offline operation. Your API calls go directly from your machine to the provider. 4DA never proxies, caches, or sees them.

## The Technical Decisions

I built the backend in **Rust** because I needed something fast enough to score hundreds of articles in seconds and reliable enough to run as a background process. The frontend is **React + TypeScript** running in **Tauri 2.0** — a framework that gives you a native desktop app with a web frontend and a Rust backend, using the system WebView instead of bundling Chromium.

The result: 4DA uses ~60MB of RAM instead of the ~300MB an Electron app would need. It starts in under a second. And it has access to system-level features like the OS keychain for secure API key storage.

The database is **SQLite** with the **sqlite-vec** extension for vector similarity search. Embeddings are generated locally via Ollama or via your API provider, and stored alongside the content for semantic matching.

The scoring pipeline compiles from a custom DSL that defines every constant with tunable ranges. This means the algorithm can be calibrated without recompiling — and it self-tunes based on your feedback over time.

## What's Free, What Costs Money

Everything that makes 4DA useful is free:
- All 20+ sources
- Full 5-axis scoring engine
- AI daily briefings (BYOK)
- The complete STREETS Playbook (7 modules on building a sovereign developer career)
- 13 languages

Signal ($12/month) adds compound intelligence features that build over time:
- Developer DNA profiling (auto-detected tech identity)
- Knowledge gap detection (blind spots in your stack)
- Signal chain analysis (connected trends across sources)
- Semantic shift tracking (how your interests evolve)
- Score autopsy (detailed breakdown of why each item scored the way it did)

The free tier is not a trial. It's the product. Signal is for developers who want to see the intelligence layer deepen over weeks and months.

## The MCP Server

4DA also ships as a 33-tool MCP server (`@4da/mcp-server` on npm) that integrates with Claude Code, Cursor, and any MCP-compatible AI tool. Install it with `npx @4da/mcp-server`, and your AI assistant gets awareness of your tech stack, dependencies, scoring intelligence, and developer context.

The MCP server is free. It works standalone or alongside the full desktop app.

## What I Learned Building It

Three things surprised me:

**The scoring pipeline is the product.** I thought the UI and the AI briefings would be the differentiator. They're not. The 5-axis confirmation gate — the rule that an item needs 2+ independent signals to pass — is what makes 4DA feel different from every other content tool. It's the reason 92% of content gets rejected instead of dumped into a feed — with 98% of actual noise correctly caught (measured across 9 developer personas, 215 labeled items each).

**Compound intelligence is real.** After 7 days of use, the scoring noticeably improves. After 30 days, it's like having a research assistant who knows exactly what you need. The feedback loop (save, dismiss, click) feeds back into the scoring weights, and the accuracy tracking shows measurable improvement over time.

**Privacy-first is a feature, not a constraint.** I expected "runs locally" to be a limitation I'd apologize for. Instead, it turned out to be the thing people care about most. In a world where every tool wants your data, running locally is a statement.

## Try It

4DA is available for Windows, macOS, and Linux. Download it at [4da.ai](https://4da.ai) or install the MCP server with `npx @4da/mcp-server`.

The source is available on [GitHub](https://github.com/runyourempire/4DA) under the Functional Source License (FSL-1.1-Apache-2.0) — source-available now, converts to Apache 2.0 in two years.

If you have questions, the best place to reach me is the GitHub issues or the discussions tab. I read everything.

---

*4DA is built by [4DA Systems Pty Ltd](https://4da.ai), an independent Australian software company. No VC funding. No user tracking. Just a tool that helps developers focus on what matters.*
