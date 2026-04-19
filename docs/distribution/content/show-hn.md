# Show HN Post — 4DA

## Title

Show HN: 4DA — Scores HN, arXiv, Reddit, GitHub against your actual codebase

## Post Body

4DA is a desktop app that scans your local projects and scores developer content from Hacker News, arXiv, Reddit, GitHub trending, Product Hunt, and RSS feeds against what you're actually building. Instead of reading everything and hoping something is relevant, it surfaces what matters to your stack, dependencies, and problem domain.

How it works: An Autonomous Context Engine scans your local repos (package.json, Cargo.toml, imports, README files) to build a developer profile. Content from sources gets embedded and scored against that profile using a confidence-weighted algorithm called PASIFA. Results above threshold surface in the app. Everything below gets discarded.

Stack: Tauri 2.0 (Rust backend, React/TypeScript frontend), SQLite with sqlite-vec for local vector search, optional Ollama for embeddings. Nothing leaves your machine — no accounts, no telemetry, BYOK for any LLM features.

The MCP server component (@4da/mcp-server) exposes 35 tools to Claude Code, Cursor, or any MCP client so your LLM can query trends, knowledge gaps, and signals relevant to your codebase. MIT licensed.

Install the MCP server: `npx @4da/mcp-server --setup`

Website: https://4da.ai

---

## Strategy Notes

**Timing:** Post Tuesday or Wednesday, 8-9 AM US Eastern (when HN has highest engaged readership and moderator attention for "Show HN" posts). Avoid Mondays (backlog) and Fridays (low engagement).

**Engagement rules:**
- Respond to every comment within the first 2 hours — this is the critical window for HN ranking
- Be technical and direct. HN values substance over polish.
- Link to GitHub/docs, not the landing page, when answering technical questions
- If someone finds a bug or edge case, acknowledge it honestly and fix it live if possible
- Do not argue with critics — acknowledge valid points, correct factual errors briefly

**Link target:** Submit the GitHub repo URL, not the landing page. HN penalizes marketing sites. Put the website link in the post body.

---

## Prepared Responses

### Q: "Why not just use RSS?"

RSS gives you a firehose. The problem isn't access to content — it's that there's too much of it. 4DA scores every item against your actual codebase: your dependencies, your language, your domain. A new Rust async runtime matters if you write Rust. A React Server Components deep-dive matters if you're on Next.js. RSS can't make that distinction. The scoring algorithm (PASIFA) uses local embeddings to compare content semantics against your project context, so relevant items surface and everything else gets filtered out automatically.

### Q: "How is the scoring done?"

The scoring engine is called PASIFA (Privacy Aware Semantic Intelligence for File Analysis). It works in layers: first, the Autonomous Context Engine scans your local repos — package files, imports, READMEs, config — to build a developer profile with weighted topics. Then incoming content gets embedded locally (Ollama or fallback zero vectors) and compared against that profile using cosine similarity with confidence weighting. There's an auto-tuning threshold that adjusts based on your feedback. The algorithm is in the Rust backend, not behind an API — everything runs on your machine.

### Q: "Why not open source the whole thing?"

The MCP server is MIT licensed. The desktop app is source-available under FSL-1.1-Apache-2.0 — you can read, modify, and run the code. The only restriction is you can't use it to build a competing product. After two years, it converts to full Apache 2.0 automatically. This lets us build a sustainable business while keeping the code transparent and auditable, which matters for a tool that scans your local projects.

### Q: "Does it phone home?"

No. The app makes outbound requests only to fetch content from sources you've enabled (HN API, Reddit, arXiv, RSS feeds). Your codebase data, developer profile, embeddings, and scores never leave the machine. There's no account system, no telemetry, no analytics. Settings are stored in a local JSON file. The database is local SQLite. If you use LLM features, you provide your own API key (BYOK) and requests go directly from your machine to the provider — we never proxy or see them.

### Q: "What's the business model?"

Paid tiers for the desktop app. Free tier gives you full source scoring, the STREETS developer playbook, and the MCP server. Signal tier ($12/month) adds advanced analytics, compound advantage tracking, and priority source refresh. Team and Enterprise tiers add seat-based licensing. The MCP server stays MIT and free regardless of tier. We're not venture-funded and not optimizing for growth metrics — the goal is a sustainable tool that's worth paying for.
