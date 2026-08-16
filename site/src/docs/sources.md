---
layout: docs.njk
eyebrow: "How it works"
title: "Sources — 4DA Docs"
description: "The 22 content sources 4DA reads, all fetched locally from public endpoints you configure."
permalink: "/docs/sources/"
templateEngineOverride: md
---

# Sources

4DA reads from **22 source adapters**, all running locally in the background. Each one fetches public content from an endpoint you configured — there is no 4DA-operated relay in between.

## What's covered

| Category | Sources |
|---|---|
| **Community** | Hacker News, Reddit, Lobsters, DEV.to, Stack Overflow, Lemmy |
| **Code & releases** | GitHub, crates.io, npm, PyPI, Go modules |
| **Research** | arXiv, Papers with Code, Hugging Face |
| **Security** | CVE, OSV |
| **Social & video** | Twitter/X, Bluesky, Mastodon, YouTube |
| **Launches** | Product Hunt |
| **Anything else** | Custom RSS feeds |

## How adapters run

Adapters run on a background interval you control. Each pull is scored immediately against your context, so the database only accumulates items that matter — not a raw mirror of everything published.

Because the scoring signal comes from *your* filesystem, adding more sources doesn't add more noise. A firehose source and a niche RSS feed are held to the same gate: an item still needs [2+ independent axes](/docs/scoring/) to survive.

## Custom feeds

Point 4DA at any RSS/Atom feed to fold a newsletter, a changelog, or a personal blog into the same scoring pipeline. It's treated like every other source — no special-casing, same confirmation gate.

Next: **[The 5 axes](/docs/scoring/)** or **[Privacy & BYOK](/docs/privacy/)**.
