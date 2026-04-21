# Show HN Post — 4DA

## Title

Show HN: 4DA — MCP server that scans your deps for CVEs and scores HN against your stack

## Post Body

I built an MCP server that gives Claude/Cursor/Copilot awareness of your actual tech stack. It reads your lockfiles, scans dependencies against OSV.dev for known vulnerabilities, fetches Hacker News headlines relevant to your stack, and maintains persistent project memory across sessions. 36 tools, MIT licensed, works standalone with zero config.

    npx @4da/mcp-server --setup

What it does: On startup it reads your package-lock.json / Cargo.lock / go.sum / poetry.lock, resolves exact dependency versions, then batch-queries OSV.dev for CVEs. It also scores HN headlines against your detected tech stack using word-boundary matching. All results are cached in a local SQLite database (1h for vulns, 30min for headlines).

The vulnerability scanner sends only package names + versions to OSV.dev (the same data visible in your manifest files). Everything else is local-only — no accounts, no telemetry. Set FOURDA_OFFLINE=true to disable all network calls.

Beyond security scanning, there are tools for decision memory (record + enforce architectural decisions), knowledge gap detection, developer DNA profiling, tech radar, and agent session handoff. The desktop app (4DA) adds content scoring from HN, arXiv, Reddit, GitHub against your codebase using local embeddings.

Stack: TypeScript, @modelcontextprotocol/sdk, better-sqlite3. The companion desktop app is Tauri 2.0 (Rust + React + SQLite + sqlite-vec).

GitHub: https://github.com/runyourempire/4DA/tree/main/mcp-4da-server
npm: https://www.npmjs.com/package/@4da/mcp-server
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

### Q: "Why not just use npm audit / cargo audit?"

Those tools show you a list you have to go read. This plugs directly into your AI coding session. When you ask Claude "are there any issues with my deps?" it calls vulnerability_scan and gives you the answer in context — with upgrade paths, severity levels, and how they relate to your actual usage. It also means your AI knows not to recommend a package with an active CVE, because it checked before suggesting it.

### Q: "Does it phone home?"

The only outbound request is vulnerability_scan sending package names + versions to OSV.dev (same data that's public in your manifest files). Everything else is local SQLite reads. No account system, no telemetry, no analytics. Set FOURDA_OFFLINE=true to go fully offline. If you use LLM features in the desktop app, you provide your own API key (BYOK) and requests go directly from your machine to the provider.

### Q: "What's the business model?"

Paid tiers for the desktop app. Free tier gives you full source scoring, the STREETS developer playbook, and the MCP server. Signal tier ($12/month) adds advanced analytics, compound advantage tracking, and priority source refresh. Team and Enterprise tiers add seat-based licensing. The MCP server stays MIT and free regardless of tier. We're not venture-funded and not optimizing for growth metrics — the goal is a sustainable tool that's worth paying for.
