# Product Hunt Launch Draft

**Launch day:** Tuesday or Wednesday, 12:01 AM PT
**Category:** Developer Tools
**Topics:** Developer Tools, Productivity, Privacy, AI, Open Source
**Prep:** Respond to every comment within 30 minutes for the first 6 hours.

---

## Tagline (60 chars max)

```
Your codebase decides what you read. Not an algorithm.
```

(54 chars)

---

## Description (260 chars max)

```
4DA scans your local projects and scores developer content from 20+ sources against your actual code. 5-axis gate rejects 99% as noise. What survives matters to YOUR stack. Runs locally, zero telemetry, free tier includes everything. Win/Mac/Linux.
```

(249 chars)

---

## First Comment (Maker's Story)

```
I tracked my content consumption for a month. 847 articles from HN, Reddit, arXiv, GitHub Trending, RSS feeds. I logged every one. 12 were directly relevant to what I was building. That is a 98.6% noise rate.

I tried the existing tools. Feedly collects feeds but scores nothing. daily.dev personalizes based on clicks -- I read one article about ZFS internals and got storage content for weeks. I do not work on storage. Newsletters are someone else's opinion about what "developers" should care about.

The fix was staring at me: my Cargo.toml already knows what matters. My package.json. My go.mod. Your codebase is the most accurate signal of what is professionally relevant to you -- not your clicks, not your subscriptions, not a topic checklist you set up once.

So I built 4DA. It scans your local projects, builds a technology profile, and scores every piece of content across 5 independent axes: context match, interest alignment, codebase relevance, dependency impact, and learned preferences. An item needs 2+ independent confirming signals to pass. A single signal -- even a strong one -- gets capped below the threshold. This is what produces the 99% rejection rate, and why what survives is actually useful.

Stack: Rust backend with Tauri 2.0, React + TypeScript frontend, SQLite with sqlite-vec for vector search, local embeddings via Ollama. 3,639 tests passing. 13 languages supported. ~15 MB installed.

Things worth knowing:

- The free tier is real. All 20+ sources, the full scoring engine, feedback-driven tuning, AI briefings, the MCP server (33 tools for Claude Code and Cursor). It is not a trial. Most people will never need Signal.

- Privacy is the architecture, not a feature. Your data literally cannot reach me. There is no server. No telemetry. No account. I do not know how many people use 4DA. That is by design.

- It works offline. Local embeddings via Ollama. No internet required for scoring after the initial content fetch.

- BYOK for LLM features. Bring your own API key for OpenAI, Anthropic, or OpenRouter. Keys stay on your machine. Or skip cloud APIs entirely and run everything through Ollama.

My content consumption went from roughly 2 hours a day to about 10 minutes. The biggest win is not time saved -- it is catching things I would have missed entirely. A dependency shipping a breaking change. An arXiv paper solving a problem I had been stuck on for a week.

I would love to hear what you think. Especially: does the scoring match your intuition about what matters? That is the hardest part to get right, and I am still calibrating.
```

(~470 words)

---

## Gallery Descriptions (5 screenshots)

### Screenshot 1: Scored Results Feed
```
The main feed. Every item scored against your actual codebase -- not clicks, not trending, not editorial picks. Multi-signal badges show which axes confirmed relevance. Items that fail the 2-signal gate never appear.
```

### Screenshot 2: Intelligence Preview
```
"Here's what I found." The daily intelligence briefing summarizes what matters across all 20+ sources, written by AI that has full context of your projects. No generic developer news -- every sentence ties back to your stack.
```

### Screenshot 3: Developer DNA Profile
```
Your technology fingerprint, built from scanning your actual projects. Languages, frameworks, dependencies, architecture patterns -- all discovered automatically. This is what the scoring engine uses to evaluate every piece of content.
```

### Screenshot 4: AI Briefing
```
AI-generated analysis that connects dots across sources. When a CVE affects a dependency you use, when an arXiv paper addresses a pattern in your code, when a trending repo solves a problem you have -- surfaced automatically, explained in context.
```

### Screenshot 5: Score Autopsy
```
Full transparency into why any item scored the way it did. See which of the 5 axes fired, the signal strength of each, the confirmation gate math, and exactly why something passed or got rejected. No black boxes.
```

---

## Notes

- Maker comment opens with specific, relatable numbers (847/12) rather than a vague problem statement.
- Names competitors respectfully, explains precisely why they fail the use case.
- "My Cargo.toml already knows what matters" is the memorable line.
- Feature list is short and leads with what PH audiences care about (free, private, small).
- Ends with a genuine question to invite engagement and drive comment threads.
- No exclamation marks. No hype language. No "we're so excited."
