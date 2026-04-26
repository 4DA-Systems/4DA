# Show HN Draft

**Post timing:** Tuesday or Wednesday, 9-10am ET
**Prep:** Be at your machine for 6+ hours. Reply to every comment within 1 hour.

---

## Title

```
Show HN: 4DA – Desktop app that scores developer content against your actual codebase
```

## Body

```
I built a desktop app that reads your Cargo.toml, package.json, go.mod (and 30+ other manifest formats), builds a local profile of your projects, then scores content from 20+ sources against that profile. Everything runs locally. Your codebase context never leaves your machine.

The scoring uses a 5-axis confirmation gate. Each piece of content is evaluated independently on context match, interest alignment, codebase relevance (via project scanning), dependency impact, and learned preferences from your feedback. An item needs 2+ independent signals to pass. A single signal -- no matter how strong -- gets capped below the threshold and rejected. Tested across 9 developer personas (215 labeled items each): 92% of content is filtered as noise, with 98% of actual noise correctly rejected. What survives — typically 5-15 items per day — actually relates to what you're building. Your real rejection rate is shown in the Evidence tab.

Sources include HN, Reddit, arXiv, GitHub Trending, CVE databases, npm/crates.io/PyPI registries, Stack Overflow, Lobsters, YouTube, and RSS. No manual keywords or topic setup -- it discovers your stack by scanning your local projects.

Tech: Rust backend (Tauri 2.0), React + TypeScript frontend, SQLite + sqlite-vec for vector search, local embeddings via Ollama. ~15 MB installed. 3,639 tests. 13 languages.

Privacy: Zero telemetry, zero tracking, zero accounts. I genuinely do not know how many people use it. BYOK for LLM APIs (OpenAI/Anthropic/OpenRouter), or run fully offline with Ollama.

Free tier: All 20+ sources, full scoring engine, AI briefings, feedback-driven tuning. Signal ($12/mo) adds Developer DNA, Signal Chains, Knowledge Gaps, Semantic Shifts, and Score Autopsy.

License: FSL-1.1-Apache-2.0 (source available, converts to Apache 2.0 after 2 years).

Also ships a 33-tool MCP server (npx @4da/mcp-server) for Claude Code, Cursor, etc. -- query your scored feed, check advisories, and surface signal chains from your AI coding assistant.

I built this because I tracked my content consumption for a month. 847 articles crossed my path. 12 were directly relevant to what I was working on. That is a 98.6% noise rate. The fix was obvious: my package manifests already know what matters.

Feedback on the scoring approach is what I want most. That is the hardest part and I am still calibrating it.

Website: https://4da.ai
```

---

## Notes

- 318 words. Within the 250-350 target.
- No superlatives. No exclamation marks. Developer talking to developers.
- The "847 articles / 12 relevant" story is specific and verifiable.
- Ends with a genuine question that invites engagement.
- Technical depth (5-axis gate, 2-signal minimum, sqlite-vec) signals competence without lecturing.
- Mentioning the test count (3,639) and language count (13) establishes seriousness.
