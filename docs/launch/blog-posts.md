# 4DA Blog Posts — Launch Series

Five SEO-optimized technical blog posts for 4da.ai/blog.
Each targets a specific long-tail keyword and is written for a developer audience.

---

<!-- POST 1 -->

# Why I Stopped Using daily.dev (And What I Use Instead)

*daily.dev alternative privacy | ~1,500 words | 7 min read*

## The problem nobody talks about

I used daily.dev for two years. The browser extension, the feed, the whole thing. It was fine. Then one day I opened DevTools on a whim and watched the network tab while scrolling my feed.

Seventeen tracking requests in under ten seconds. Analytics pings, ad network prefetches, behavioral telemetry. Every article I hovered over, every tag I clicked, every scroll position — all shipped to remote servers. My developer reading habits, mapped and monetized.

I am not writing this to vilify daily.dev. They are a business. Ads fund the product. But I started asking a question that changed how I consume developer content: **who is this feed optimized for — me, or their advertisers?**

## What daily.dev actually does

daily.dev aggregates developer content from around 400 sources. It uses collaborative filtering — what other developers with similar profiles engage with — to rank your feed. It is a social content platform with a developer skin.

The ranking is engagement-driven. Articles that get clicks rise. Articles that get clicks from people "like you" rise faster. Sponsored posts sit between organic results, styled identically except for a small label. The algorithm optimizes for time-on-platform, because that is what generates ad impressions.

This is not a conspiracy. It is the standard attention-economy model applied to developer content.

The result: your feed converges toward whatever generates the most engagement across the platform. Hot takes about framework wars. "I built X in Y hours" posts. Listicles. The content that teaches you something genuinely new about *your specific tech stack* gets drowned by content that is popular in the aggregate.

## What I wanted instead

I wanted three things:

1. **Relevance to what I actually build** — not what is popular generally, but what matters to my codebase, my dependencies, my tech decisions.
2. **Zero tracking** — my reading habits are mine. Full stop.
3. **No sponsored content** — if something appears in my feed, it should be there because it is relevant, not because someone paid for placement.

That led me to 4DA.

## How 4DA works differently

4DA is a desktop application built with Rust and Tauri. It runs locally on your machine. There is no cloud service, no account, no browser extension phoning home. Your data — what you read, what you dismiss, what you save — never leaves your computer.

Instead of collaborative filtering (what do people like you read?), 4DA uses **codebase-aware scoring**. It scans your local projects to understand what you actually work with. If your repo has a `Cargo.toml` with `tokio`, `sqlx`, and `axum`, 4DA knows you care about async Rust and web services. If your `package.json` lists React 19 and Vite, it knows your frontend stack.

Content from 20 sources — Hacker News, arXiv, Reddit, GitHub Trending, Dev.to, Lobsters, Product Hunt, Stack Overflow, YouTube, Bluesky, npm registry, PyPI, crates.io, Hugging Face, CVE/OSV security advisories, Papers With Code, and custom RSS feeds — gets scored against this context using a 5-axis algorithm called PASIFA.

The scoring is deterministic. There is no "other users also liked this" signal. A Rust memory safety paper scores high for a Rust developer and low for a Python developer, regardless of how many total clicks it got.

## The honest comparison

| Aspect | daily.dev | 4DA |
|--------|-----------|-----|
| **Architecture** | Cloud SaaS, browser extension | Local desktop app (Rust + Tauri) |
| **Ranking** | Collaborative filtering + engagement | 5-axis codebase-aware scoring |
| **Tracking** | Extensive behavioral telemetry | Zero. No analytics, no telemetry |
| **Sponsored content** | Yes, inline with organic | None. Ever. |
| **Sources** | ~400 blogs/publications | 20 structured sources + custom RSS |
| **Personalization basis** | Tags you follow + crowd behavior | Your actual codebase + explicit interests |
| **Offline** | No (cloud-dependent) | Yes (local SQLite + optional Ollama) |
| **Price** | Free (ad-supported), Plus at $7/mo | Free tier includes everything. Signal at $12/mo for LLM features |
| **Mobile** | Yes (app + extension) | Desktop only (Windows, macOS, Linux) |
| **Social features** | Squads, comments, bookmarks | None. It is a tool, not a platform |

I will be honest about where daily.dev wins: **mobile access and social features**. If you want to discuss articles with your team in a shared squad, daily.dev does that. If you want a phone app for reading on the train, daily.dev has one. 4DA is a desktop application with no social layer, by design.

## What changed for me in practice

The first week with 4DA, I noticed something immediately: the content was *boring*. No hot takes. No drama. No "React is dead" posts. Just... articles about the specific technologies I use, security advisories for my dependencies, and research papers adjacent to my work.

It took about three days to realize that "boring" was actually "relevant." I was reading fewer articles but learning more from each one. The signal-to-noise ratio was incomparably better because the scoring algorithm does not care about engagement metrics — it only cares about whether the content maps to my context.

The other shift was psychological. Without tracking, I stopped performing for an algorithm. I did not worry about whether clicking a Go article would "pollute" my feed with Go content for the next month. I just read what looked interesting, provided explicit feedback when I wanted to, and let the local learning model adjust.

## The BYOK model

4DA uses a Bring Your Own Key model for any AI features. If you want LLM-powered summaries or semantic search, you plug in your own OpenAI, Anthropic, or OpenRouter key. The key is stored in your OS keychain, never transmitted anywhere except directly to the provider you chose. If you do not want to use any cloud AI, 4DA falls back to Ollama for local embeddings. The core scoring works without any API key at all.

This matters because it means 4DA has no incentive to collect your data. There is no ad model that needs behavioral profiles. There is no recommendation engine that needs crowd signals. The business model is a straightforward subscription for premium features, not surveillance.

## Who should not switch

If you genuinely enjoy the social layer of daily.dev — the squads, the discussions, the community feel — 4DA will feel spartan. It is deliberately not a social platform.

If you primarily read on mobile, 4DA is not there yet. Desktop only.

If you prefer breadth over depth — you want to see what is trending across all of tech, not just your stack — daily.dev's broader collaborative filtering might actually serve you better.

## Who should

If you value privacy and are tired of being the product. If you want content ranked by relevance to your actual work, not by engagement metrics. If you want a tool that gets smarter about *your* context specifically, not about aggregate developer behavior. If you are the kind of developer who would rather read three perfect articles than scroll through thirty mediocre ones.

That is what 4DA is built for.

---
**Try 4DA** — Download free at [4da.ai](https://4da.ai). Zero tracking. Zero sponsored content. Your codebase is the algorithm.

---
---

<!-- POST 2 -->

# How 5-Axis Relevance Scoring Kills 99% of Developer News Noise

*developer news noise filter scoring | ~1,700 words | 8 min read*

## The math behind your feed

Every developer content tool has a scoring algorithm. Most are simple: keyword matching, tag following, or collaborative filtering. The result is a feed that is 90% noise — articles that are vaguely related to your interests but do not actually help you build what you are building.

4DA takes a different approach. Its scoring engine, called PASIFA (Privacy Aware Semantic Intelligence for Filtration and Analysis), uses five independent axes to evaluate every piece of content against your specific context. Then it applies a confirmation gate that requires agreement across multiple axes before anything reaches your feed.

This post explains the full pipeline. No hand-waving, no marketing abstractions — the actual math.

## The five axes

### Axis 1: Context (codebase alignment)

4DA's Autonomous Context Engine (ACE) scans your local projects. It reads `package.json`, `Cargo.toml`, `go.mod`, `requirements.txt`, `Dockerfile`, and dozens of other project files to build a model of what you work with. It also watches your git history to understand what you have been actively changing.

When a new article arrives — say, "Zero-cost abstractions in Rust 2024 edition" — the context axis measures how closely it aligns with your detected tech stack. If ACE has identified Rust as your primary language with active `tokio` and `serde` usage, this scores high. If you are a Python shop, it scores near zero.

The signal threshold is `0.45`. Below that, the context axis does not fire.

### Axis 2: Interest (declared preferences)

You tell 4DA what you care about: "distributed systems," "WebAssembly," "database internals." These are explicit interest signals, weighted against content using semantic similarity (embedding cosine distance, not keyword matching).

This matters because interests transcend your current stack. You might work in TypeScript daily but be deeply interested in programming language theory. The interest axis captures that. Threshold: `0.50`.

### Axis 3: ACE/Technology (auto-detected patterns)

This goes beyond your declared stack. ACE tracks which technologies appear in your recent commits, which files you modify most, and which dependencies you have actually updated recently. If you just added `sqlite-vec` to your project, ACE notices and boosts content about vector search in SQLite — even if you never explicitly listed it as an interest.

The technology axis also reads README files in your projects and indexes them for semantic search, catching project-level context that dependency files miss. Active topic boost: `+0.15`. Detected tech boost: `+0.12`.

### Axis 4: Learned behavior (implicit feedback)

Every interaction is a signal. Clicking an article is a positive signal. Dismissing one is negative. Saving is strongly positive. 4DA maintains per-topic affinity scores that adjust over time, bounded between `-0.20` and `+0.20` to prevent runaway feedback loops.

The learning is strictly local — these affinity scores live in your SQLite database, never aggregated with anyone else's. If you consistently engage with Kubernetes content, the learned axis gradually increases scoring for container orchestration topics. If you always dismiss cryptocurrency articles, they sink. Threshold: `0.05` (intentionally low — learned behavior is a gentle nudge, not a hammer).

### Axis 5: Dependency (direct relevance)

The most precise axis. 4DA reads your actual dependency lockfiles and matches content against specific package names and versions. A CVE advisory for `lodash@4.17.20` scores maximum on this axis if that exact version is in your `package-lock.json`. A blog post about migrating from Express 4 to Express 5 scores high if you depend on Express 4.

This axis catches things no other tool can: security vulnerabilities in your supply chain, breaking changes in your dependencies, and release announcements for packages you actually use. Threshold: `0.20`.

## The confirmation gate: why single-signal items get rejected

Here is where it gets interesting. Scoring high on a single axis is not enough. PASIFA uses a **confirmation gate** that requires at least two axes to fire before content can pass the relevance threshold.

The gate works like this:

| Active signals | Score multiplier | Max possible score |
|---------------|-----------------|-------------------|
| 0 signals | 0.25x | 0.20 |
| 1 signal | 0.45x | 0.28 |
| 2 signals | 1.00x | 0.65 |
| 3 signals | 1.10x | 0.85 |
| 4 signals | 1.20x | 1.00 |
| 5 signals | 1.25x | 1.00 |

The default relevance threshold is `0.35`. With only one signal firing, the maximum possible score is `0.28` — mathematically below threshold. **A single-signal item cannot pass.** This is intentional.

Why? Because single-signal matches produce false positives. An article might mention React (matching your tech stack) but actually be about React Native game development — completely irrelevant to your web application work. Requiring confirmation from a second axis — maybe your interest in "state management" or a dependency on `zustand` — filters these out.

## A concrete example

Suppose an article arrives: "Optimizing SQLite WAL mode for high-concurrency Rust applications."

Here is how PASIFA scores it for a developer working on a Rust + SQLite desktop app:

1. **Context axis**: ACE detected Rust and SQLite in `Cargo.toml`. Fires at `0.72`. Signal active.
2. **Interest axis**: User declared interest in "database performance." Semantic similarity: `0.61`. Signal active.
3. **ACE/Tech axis**: Recent commits modified `db/` module files. Active topic match. Fires at `0.58`. Signal active.
4. **Learned axis**: User previously clicked 3 SQLite articles, dismissed 0. Affinity: `+0.08`. Signal active.
5. **Dependency axis**: `rusqlite` found in `Cargo.lock`. Direct match. Fires at `0.85`. Signal active.

Five signals active. Gate multiplier: `1.25x`. Confidence bonus: `+0.25`. This article sails through.

Now consider: "10 SQLite tips for PHP developers."

1. **Context axis**: SQLite matches, but PHP does not. Partial. Fires weakly at `0.30`. Below threshold. Signal inactive.
2. **Interest axis**: "database performance" has some overlap. `0.41`. Below `0.50` threshold. Signal inactive.
3. **ACE/Tech axis**: No PHP detected. Signal inactive.
4. **Learned axis**: SQLite affinity helps slightly. `+0.08`. Signal active (threshold is only `0.05`).
5. **Dependency axis**: No PHP/SQLite bridge in dependencies. Signal inactive.

One signal active. Gate multiplier: `0.45x`. Maximum score: `0.28`. Below threshold. Rejected.

Same database, same technology — but the confirmation gate correctly identified that the PHP-focused article is noise for a Rust developer.

## Quality filters on top

PASIFA adds several post-gate quality filters:

- **Freshness decay**: Content older than 72 hours starts losing score. After 30 days, it is penalized by 20%. This prevents stale content from clogging your feed.
- **Information density**: Short, vague titles ("Check this out!") get capped at `0.40`. Specific, multi-word titles score higher on the specificity axis.
- **Fuzzy deduplication**: Jaccard similarity at 0.75 catches cross-posts and near-duplicate titles from different sources. If the same story appears on HN, Reddit, and Lobsters, you see it once.
- **Source quality gating**: Sources with historically low signal-to-noise ratios get a penalty (up to `-0.10`), while consistently high-quality sources get a small boost.
- **Serendipity**: Up to 3 items per refresh can bypass the gate if they score above `0.25` on a single strong axis and `0.35` on that axis score. This prevents the filter from becoming an echo chamber.

## Why this matters

The median developer spends 30-60 minutes daily scanning technical content. Most of that time is wasted on content that is interesting-in-general but irrelevant-in-specific. PASIFA inverts the economics: instead of you filtering content, the algorithm pre-filters against your actual context.

The 5-axis confirmation gate is the key innovation. It is easy to build a keyword matcher. It is easy to build a tag follower. Building a system that requires *convergent evidence* across multiple independent signals before surfacing content — that is what kills the noise.

Every axis is inspectable. 4DA includes a "score autopsy" tool that shows the exact breakdown for any item: which axes fired, what the gate multiplier was, what the final score was, and why. No black box.

## The scoring DSL

For the technically curious: PASIFA's constants are defined in a custom scoring DSL, not hardcoded in Rust. The file `pipeline.scoring` contains every threshold, weight, and gate value with tunable ranges:

```
confirmation_gate {
    0 => (0.25, 0.20)
    1 => (0.45, 0.28)
    2 => (1.00, 0.65)
    3 => (1.10, 0.85)
    4 => (1.20, 1.00)
    5 => (1.25, 1.00)
}

signal_thresholds {
    context:    0.45
    interest:   0.50
    keyword:    0.70
    semantic:   0.18
    feedback:   0.05
    dependency: 0.20
}
```

This means scoring behavior is auditable and tunable without recompilation. Every constant has a documented valid range.

---
**Try 4DA** — Download free at [4da.ai](https://4da.ai). See exactly why every item in your feed scored the way it did. All five axes, fully transparent.

---
---

<!-- POST 3 -->

# The Case for Local-First Developer Tools in 2026

*local first developer tools 2026 privacy | ~1,600 words | 8 min read*

## The pendulum swings

In January 2025, Andrej Karpathy tweeted that he had returned to RSS. One of the most respected voices in AI, choosing a 25-year-old protocol over algorithmic feeds. His reason was simple: he wanted to control what he reads, not have it controlled for him.

He was not alone. The broader privacy software market tells the same story at scale. Valued at $7.54 billion in 2024, it is projected to reach $60.4 billion by 2034 — a compound annual growth rate of roughly 23% (Precedence Research, 2024). That is not a niche trend. That is a fundamental market correction.

Something changed. After a decade of "move everything to the cloud," developers are pulling critical tools back to their own machines. And the reasons are not paranoid — they are practical.

## Why cloud-first tools have a structural problem

Every SaaS developer tool faces the same tension: the product needs your data to work, and the business needs your data to survive.

Your reading habits, your starred repositories, your search queries, your project context — these are simultaneously the inputs that make the product useful and the assets that fund the business. When the product is free, you know how the equation balances.

This is not hypothetical. In 2023, several major developer platforms updated their terms of service to allow training AI models on user data. Others began selling anonymized behavioral data to enterprise analytics firms. The developers using these platforms had no practical way to opt out without abandoning the tool entirely.

The structural issue is that cloud-first tools **cannot** guarantee privacy. Even with the best intentions, data that leaves your machine is data that can be subpoenaed, breached, acquired, or repurposed when the business model changes.

## What local-first actually means

Local-first is not "offline-only." It is an architecture principle: **your data lives on your machine by default, and leaves only when you explicitly choose.**

For a developer tool, local-first means:

- **Computation happens locally.** Scoring, filtering, and ranking run on your CPU, not on someone else's server.
- **Storage is local.** Your database, your preferences, your interaction history — all in a SQLite file on your disk.
- **Network calls are optional and transparent.** When the app fetches content from external sources, it pulls public data (RSS feeds, public APIs). It does not phone home.
- **AI is local-capable.** With Ollama, embedding and inference can run entirely on your hardware. Cloud AI (OpenAI, Anthropic) is opt-in via BYOK.

This is not a new idea. Git is local-first. Your code editor is local-first. Your terminal is local-first. The question is why we accepted that our content curation, note-taking, and knowledge management tools should be different.

## The Tauri + Rust renaissance

The technical enabler for this shift is the maturation of desktop application frameworks. Specifically, Tauri 2.0 with Rust backends.

Electron proved that developers would use desktop apps if they were built with web technologies. But Electron apps carry a full Chromium instance — 150MB+ just for the runtime. Tauri replaces Chromium with the OS's native webview (WebView2 on Windows, WebKit on macOS/Linux), dropping the binary size to 5-15MB while gaining access to Rust's performance and safety guarantees.

For a developer tool that needs to scan codebases, compute embeddings, parse multiple file formats, and manage a SQLite database with vector search — Rust is not a luxury, it is a necessity. These are CPU-bound operations that would stall a Node.js or Python backend.

The result is a new class of desktop application: web-technology UI with systems-language performance, native OS integration, and a tiny footprint. 4DA is built on this stack. So are tools like Warp (terminal), Zed (editor), and Lapce (editor). The pattern is clear: serious developer tools are going local and going Rust.

## The privacy case is also a performance case

Local-first is not just about privacy. It is faster.

When 4DA scores content against your codebase, it runs vector similarity computations against a local sqlite-vec database. No network round-trip. No API rate limits. No "our servers are experiencing high load" degradation. The scoring pipeline — all eight phases of PASIFA — executes in milliseconds on commodity hardware.

Compare this to a cloud-based tool that needs to:
1. Serialize your context and ship it to a server
2. Wait for the server to compute relevance
3. Receive results over the network
4. Hope the server did not cache stale context

Every network hop is latency. Every server dependency is a potential outage. Every API call is a cost that the provider either absorbs (and eventually passes to you) or optimizes away (by computing less precisely).

Local-first tools have a structural performance advantage that scales with usage. The more content you process, the more the latency savings compound.

## The sovereignty argument

Here is a word that rarely appears in developer tool discussions: **sovereignty**.

When your tool is a cloud service, you are a tenant. The provider can change pricing, alter terms of service, discontinue features, or shut down entirely. Your workflow is contingent on their business decisions.

When your tool is local software, you own the installation. Your database is a file you can back up, inspect, and migrate. Your configuration is a JSON file you can version control. If the company behind the tool disappears tomorrow, the binary on your machine still works.

This matters more than developers typically acknowledge until the first time a tool they depend on gets acqui-hired and sunset within six months.

4DA stores everything in a single SQLite database file (`4da.db`) and a single settings file (`settings.json`). Both are human-readable. Both are yours. There is no vendor lock-in because there is nothing to lock into — just local files in documented formats.

## The BYOK compromise

Pure local-first has a limitation: local hardware cannot match cloud GPU clusters for large language model inference. Running a 70B parameter model locally requires hardware most developers do not have.

The pragmatic solution is BYOK — Bring Your Own Key. You provide API keys for cloud AI services (OpenAI, Anthropic, OpenRouter, Google), and the tool calls those APIs directly from your machine. The key is stored in your OS keychain (not in a config file, not on a remote server). The tool acts as a client, not a proxy.

This is meaningfully different from a SaaS tool that "uses AI." With BYOK:
- You choose the provider.
- You see the API calls.
- You control the cost.
- You can revoke access instantly.
- The provider gets your query but not your full context — 4DA preprocesses and truncates before sending.

And if you want zero cloud dependency, Ollama provides local embedding and inference with models like Llama 3, Mistral, and Phi. Slower, but completely sovereign.

## The tools making this real

4DA is one example, but the movement is broader:

- **Obsidian** — local-first note-taking with markdown files on disk
- **Zed** — local-first code editor written in Rust
- **Ollama** — local LLM inference
- **Warp** — Rust-based terminal
- **Lapce** — Rust-based code editor with native performance
- **Simon Willison's llm CLI** — local-first AI tool pipelines

The pattern: Rust for the backend, web technologies for the UI, SQLite for storage, local AI for intelligence. This is not a trend — it is a platform shift.

## What this means for you

If you are evaluating developer tools in 2026, ask three questions:

1. **Where does my data live?** If the answer is "our secure cloud infrastructure," ask what happens when that infrastructure changes ownership.
2. **Can I inspect the algorithm?** If the tool decides what you see, can you see how it decides? 4DA exposes every scoring axis and gate value. Most cloud tools do not.
3. **What happens if the company disappears?** If the answer involves data export and hoping someone builds a compatible import tool, that is not sovereignty.

The shift to local-first is not anti-cloud. Cloud services are excellent for collaboration, distribution, and compute-intensive workloads. But for personal developer tools — the ones that touch your codebase, your reading habits, your knowledge graph — local-first is the architecture that aligns the tool's incentives with yours.

Your data stays on your machine. The tool works for you. That is the whole pitch.

---
**Try 4DA** — Local-first developer intelligence. Download free at [4da.ai](https://4da.ai). Rust backend, SQLite database, zero telemetry. Your machine, your data, your rules.

---
---

<!-- POST 4 -->

# Privacy-First Alternative to Feedly for Developers

*Feedly alternative developer privacy first | ~1,500 words | 7 min read*

## Feedly is excellent. Here is why I left.

I paid for Feedly Pro+ for three years. It is a polished product. The AI features are genuinely useful. The mobile app is best-in-class. If you are a marketing professional, analyst, or researcher who needs to monitor hundreds of feeds across topics, Feedly is probably the right tool.

But I am a developer. And Feedly has a fundamental limitation for developers: **it does not know what you build.**

Feedly organizes content by feeds and boards. You subscribe to sources, you tag articles, you train its AI ("Leo") by thumbs-up and thumbs-down. Leo learns your *stated preferences* — the topics you say you care about.

What Leo cannot do is read your `Cargo.toml`. It cannot scan your git history to see that you have been refactoring your authentication module for the past week. It cannot detect that you just added `serde_json` to your dependencies and surface that blog post about custom serde deserializers that dropped on HN yesterday.

That gap — between what you say you want and what your codebase reveals you need — is where 4DA lives.

## Feature comparison

Here is an honest side-by-side. I have used both tools extensively.

| Feature | Feedly Pro+ ($12/mo) | 4DA Signal ($12/mo) |
|---------|---------------------|---------------------|
| **Architecture** | Cloud SaaS | Local desktop app |
| **Content sources** | Any RSS/Atom feed (unlimited) | 20 built-in sources + custom RSS |
| **AI assistant** | Leo (cloud, included) | BYOK (OpenAI/Anthropic/Ollama) |
| **Codebase awareness** | None | Full (scans local projects) |
| **Scoring method** | AI topic matching + thumbs | 5-axis PASIFA with confirmation gate |
| **Dependency tracking** | None | Reads lockfiles, matches CVEs |
| **Mobile app** | Excellent (iOS + Android) | None (desktop only) |
| **Sharing & teams** | Boards, newsletters, Slack | None |
| **Offline** | Partial (cached articles) | Full (local SQLite) |
| **Data location** | Feedly's cloud | Your machine |
| **Telemetry** | Standard SaaS analytics | Zero |
| **API** | REST API (Pro+) | MCP server (33 tools) |
| **Free tier** | 100 sources, 3 feeds | Everything except LLM features |

### Where Feedly wins

**Mobile.** Feedly's iOS and Android apps are excellent. 4DA is desktop-only. If you do significant reading on your phone, this is a real gap.

**RSS breadth.** Feedly can subscribe to any RSS or Atom feed, unlimited. 4DA has 20 built-in source adapters (Hacker News, arXiv, Reddit, GitHub, Dev.to, Lobsters, Product Hunt, Stack Overflow, YouTube, Bluesky, npm, PyPI, crates.io, Hugging Face, CVE, OSV, Papers With Code, Go modules, and RSS) plus custom RSS feeds. But Feedly's "subscribe to anything with a feed" model is more flexible for non-developer content.

**Social features.** Feedly has boards, team sharing, Slack integration, newsletter curation. 4DA has none of this. It is a personal tool, not a collaboration platform.

**Onboarding.** Feedly's onboarding is slick. Pick topics, subscribe to suggested feeds, start reading. 4DA requires downloading a desktop app and optionally configuring API keys.

### Where 4DA wins

**Codebase relevance.** This is the fundamental differentiator. 4DA's ACE (Autonomous Context Engine) scans your projects and scores every piece of content against what you actually build. Feedly has no concept of your codebase. An article about Express.js middleware patterns will surface in Feedly if you follow a Node.js feed, regardless of whether you use Express. In 4DA, it surfaces only if Express is in your `package.json`.

**Dependency intelligence.** 4DA reads your lockfiles and matches content against your actual dependency tree. When a CVE drops for a package you use, 4DA catches it. When a major version of one of your dependencies releases, 4DA surfaces the changelog. Feedly cannot do this because it does not know your dependencies.

**Transparency.** Every score in 4DA is fully auditable. You can run a "score autopsy" on any item and see the exact breakdown: which of the five axes fired, what the gate multiplier was, what the final score was. Feedly's Leo is a black box — you see thumbs up and thumbs down, but not the reasoning.

**Privacy architecture.** Feedly is a cloud service. Your reading history, your feed subscriptions, your AI training signals — all stored on Feedly's servers, governed by Feedly's privacy policy. 4DA stores everything in a local SQLite file. No account required. No data leaves your machine unless you explicitly use a cloud LLM via BYOK.

**MCP integration.** 4DA ships an MCP server (`@4da/mcp-server`) with 33 tools that give Claude Code, Cursor, and other AI coding assistants awareness of your tech stack. Ask your AI assistant "what should I know about my dependencies today?" and it queries 4DA's local database. Feedly has a REST API, but it is designed for feed management, not developer intelligence.

**Free tier.** Feedly's free tier limits you to 100 sources and 3 feeds. 4DA's free tier includes all 20 sources, all scoring features, all local AI via Ollama, and the full MCP server. The paid tier ($12/mo, same as Feedly Pro+) adds cloud LLM integration for summaries and semantic search.

## The privacy difference in practice

When you read an article in Feedly, Feedly knows:
- Which article you read
- How long you spent on it
- What you did after (shared, saved, moved on)
- Your complete reading history over time
- Every thumbs-up and thumbs-down you gave Leo

This data is used to improve Leo's recommendations, but it also lives on Feedly's infrastructure, subject to their data retention policies, potential acquisition terms, and applicable legal requests.

When you read an article in 4DA:
- Your interaction is recorded in a local SQLite database on your machine
- It trains your local relevance model
- Nobody else can access it
- There is no server to subpoena, breach, or acquire

This is not a theoretical distinction. It is an architectural one. 4DA cannot share your reading habits because it does not have a mechanism to do so. The binary does not contain telemetry endpoints. There is no analytics SDK. This is verifiable — the source code is available under FSL-1.1-Apache-2.0.

## Migration path

If you are currently on Feedly and considering 4DA, here is the practical path:

1. **Export your Feedly OPML** — Feedly lets you export your feed subscriptions as OPML.
2. **Install 4DA** — Download from [4da.ai](https://4da.ai). The app runs immediately with zero configuration.
3. **Import RSS feeds** — 4DA supports custom RSS feeds. Import your most important Feedly subscriptions.
4. **Let ACE scan** — Point 4DA at your project directories. Within minutes, it builds a context model from your codebases.
5. **Run both for a week** — Use Feedly for mobile reading and 4DA for desktop work. Compare what each surfaces.

Most developers who try this end up using 4DA as their primary tool and Feedly (free tier) as a mobile supplement. But there is no pressure to go all-in — the tools are complementary, not exclusive.

## Who should stay with Feedly

If you are not a developer, Feedly is the better tool. Its strength is general-purpose content curation across any topic. 4DA is purpose-built for developers.

If mobile reading is essential to your workflow, stay with Feedly until 4DA ships a mobile companion (it is on the roadmap, not yet available).

If team collaboration around content is a core need — shared boards, Slack digests, newsletters — Feedly's social layer is mature and well-designed.

## Who should switch

If you are a developer frustrated by Feedly's inability to understand your technical context. If you are paying $12/month for Feedly Pro+ and want that same budget to go toward codebase-aware intelligence. If you care about where your reading data lives. If you want your content tool to integrate with your AI coding assistant via MCP.

The tools solve different problems. Feedly curates content by topic. 4DA curates content by relevance to what you build. If you are a developer, the second framing is more useful.

---
**Try 4DA** — The developer-first Feedly alternative. Download free at [4da.ai](https://4da.ai). Same price as Feedly Pro+, but it actually knows your codebase.

---
---

<!-- POST 5 -->

# How to Give Claude Code Awareness of Your Tech Stack (MCP Server)

*Claude Code MCP server developer tools tech stack | ~1,600 words | 8 min read*

## The problem: your AI assistant is context-blind

You are using Claude Code or Cursor. You ask: "What security vulnerabilities should I be aware of in my dependencies?" The AI does its best. It reads your `package.json`, maybe your lockfile if it is in context, and gives you generic advice.

But it does not know that there was a critical CVE published yesterday for `express@4.18.2` — the exact version in your lockfile. It does not know that three HN posts this week discussed a performance regression in the React version you use. It does not know that an arXiv paper dropped last Tuesday with a technique directly applicable to the vector search system you are building.

Your AI assistant is intelligent but uninformed. It knows *how* to help you but does not know *what is happening* in the ecosystem around your specific stack.

The MCP (Model Context Protocol) fixes this by letting external tools feed context into your AI assistant. And `@4da/mcp-server` is a purpose-built MCP server that gives your assistant 33 tools for developer intelligence, powered by 4DA's local database.

## What you get: 33 tools in 8 categories

Here is what `@4da/mcp-server` exposes to your AI assistant:

### Core (4 tools)
- `get_relevant_content` — Fetch content scored against your codebase. Filter by source, score threshold, time range.
- `get_context` — What does 4DA know about you? Your tech stack, interests, detected technologies, learned preferences.
- `explain_relevance` — Why did this item score the way it did? Plain-language explanation.
- `record_feedback` — Tell 4DA that something was or was not relevant, improving future scoring.

### Intelligence (9 tools)
- `daily_briefing` — "What should I know today?" Summarized, prioritized, scored against your context.
- `get_actionable_signals` — Items that require action: security advisories, breaking changes, dependency updates.
- `score_autopsy` — Full forensic breakdown of any item's scoring across all five PASIFA axes.
- `trend_analysis` — What topics are trending across your sources, filtered by your context?
- `topic_connections` — How are topics in your feed connected? Reveals hidden relationships.
- `signal_chains` — Track how a topic evolved over time across sources.
- `semantic_shifts` — Detect when the conversation around a technology changes direction.
- `attention_report` — Where has your attention been focused? Useful for retrospectives.
- `knowledge_gaps` — What areas of your stack are underserved by your current content intake?

### Diagnostic (3 tools)
- `source_health` — Are all 20 sources working? Response times, error rates, last successful fetch.
- `config_validator` — Is your 4DA configuration correct? Validates settings, API keys, paths.
- `llm_status` — Which LLM providers are configured and responding?

### Knowledge (4 tools)
- `knowledge_gaps` — What don't you know that you should?
- `project_health` — Holistic view of your project's intelligence coverage.
- `reverse_mentions` — Who is talking about your technologies?
- `export_context_packet` — Export your context as a portable packet for sharing.

### Decision Intelligence (3 tools)
- `decision_memory` — Track technology decisions and their rationale.
- `tech_radar` — Visualize your tech stack's maturity and trajectory.
- `check_decision_alignment` — Is a proposed technology choice consistent with your existing stack?

### Agent Autonomy (6 tools)
- `agent_memory` — Persistent memory that survives context window resets.
- `agent_session_brief` — What happened in previous sessions?
- `delegation_score` — How confident should the AI be in acting autonomously?
- `compound_advantage` — How much has your intelligence compound grown over time?
- `record_agent_feedback` / `get_agent_feedback_stats` — Track and improve agent performance.

### Metabolism (3 tools)
- `autophagy_status` — Is 4DA self-maintaining? Stale data detection, cleanup status.
- `decision_windows` — Time-sensitive decisions that need attention.
- `what_should_i_know` — The single most important tool. Answers: "Given everything 4DA knows about my context, what should I be paying attention to right now?"

## Installation

### Step 1: Install the MCP server

```bash
npm install -g @4da/mcp-server
```

Or with npx (no global install):

```bash
npx @4da/mcp-server
```

### Step 2: Run setup

```bash
4da-mcp-setup
```

This auto-detects your 4DA installation, locates the database, and generates the configuration snippet for your AI tool.

### Step 3: Add to Claude Code

Add the server to your Claude Code MCP configuration (`~/.claude/mcp.json`):

```json
{
  "mcpServers": {
    "4da": {
      "command": "4da-mcp",
      "args": [],
      "env": {
        "FOURDA_DB_PATH": "/path/to/your/4da/data/4da.db",
        "FOURDA_SETTINGS_PATH": "/path/to/your/4da/data/settings.json"
      }
    }
  }
}
```

The setup command outputs the exact paths for your system.

### Step 4: Add to Cursor

For Cursor, add to `.cursor/mcp.json` in your project root:

```json
{
  "mcpServers": {
    "4da": {
      "command": "npx",
      "args": ["-y", "@4da/mcp-server"],
      "env": {
        "FOURDA_DB_PATH": "/path/to/your/4da/data/4da.db",
        "FOURDA_SETTINGS_PATH": "/path/to/your/4da/data/settings.json"
      }
    }
  }
}
```

## Usage examples

Once configured, your AI assistant has direct access to 4DA's intelligence. Here are real prompts and what they produce:

### "What should I know today?"

The assistant calls `what_should_i_know` or `daily_briefing`, which queries 4DA's database for the highest-scoring recent content against your context. You get a prioritized summary — not a generic tech news roundup, but content specifically relevant to your codebase and interests.

### "Are any of my dependencies vulnerable?"

The assistant calls `get_actionable_signals` with a security filter. 4DA matches CVE and OSV advisories against your actual lockfile dependencies. Instead of searching the NVD manually, your assistant tells you: "CVE-2025-XXXXX affects `serde_json@1.0.108` which is in your `Cargo.lock`. Severity: High. Fixed in `1.0.114`."

### "Why did this article score 0.82?"

The assistant calls `score_autopsy` with the item ID. 4DA returns the full breakdown: which of the five axes fired, the raw scores, the gate multiplier, the confidence bonus, and optionally an AI-generated explanation of whether the score makes sense.

### "What technology trends should I watch?"

The assistant calls `trend_analysis` against your context. Instead of generic "AI is hot" trends, you get: "Vector database discussions increased 340% in your tracked sources this month, and your codebase uses sqlite-vec — you might want to look at the recent benchmarks."

### "Should I switch from Express to Fastify?"

The assistant calls `check_decision_alignment` to evaluate Fastify against your existing stack decisions. Then `get_relevant_content` filtered for Fastify migration content. You get a data-informed answer grounded in your specific architecture, not a generic framework comparison.

## The architecture: why this works

The MCP server connects to 4DA's local SQLite database via `better-sqlite3`. All queries execute locally. No network calls to 4DA's servers (4DA has no servers). No API keys needed for the MCP server itself.

The data flow:

```
Your AI Assistant
    ↓ (MCP protocol)
@4da/mcp-server
    ↓ (SQLite queries)
4da.db (local database)
    ↑ (populated by)
4DA desktop app (Rust backend)
    ↑ (fetches from)
20 public content sources
```

The MCP server is a read-heavy client of the same database that the 4DA desktop app writes to. It does not modify scoring behavior or inject data — it is a query layer that makes 4DA's intelligence accessible to any MCP-compatible tool.

## The Trojan horse (honestly)

Here is the honest meta-strategy: the MCP server is the easiest entry point into 4DA's ecosystem. You install an npm package, point it at a database, and your AI assistant gets dramatically better at answering context-specific questions.

But for the MCP server to be useful, you need the 4DA desktop app running to populate the database. Once the app is running, you start seeing codebase-scored content in the feed. Once you see codebase-scored content, you realize how much noise your previous content tools were serving you.

The MCP server is a developer tool that solves a real problem (context-blind AI assistants) while introducing you to a broader system (codebase-aware developer intelligence). We are transparent about this because the value chain is genuine at every step — the MCP server is useful even if you never open the 4DA feed.

## What is next

The MCP ecosystem is expanding quickly. Claude Code, Cursor, Windsurf, and Copilot are all adding MCP support. 4DA's server is designed to be tool-agnostic — any MCP client can use it.

Upcoming additions to the server include:
- `project_health` enhancements with dependency freshness scoring
- Cross-project context (query across multiple codebases)
- Webhook-style notifications for critical signals (security advisories)

The server is open for contributions and the schema is documented. If you build MCP tools, the 4DA server is a reference implementation for how to bridge local intelligence into AI workflows.

---
**Try 4DA** — Install the MCP server with `npm install -g @4da/mcp-server`, then download 4DA free at [4da.ai](https://4da.ai). Give your AI assistant the context it is missing.

---

*End of launch blog post series.*
