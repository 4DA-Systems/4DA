---
layout: docs.njk
eyebrow: "How it works"
title: "The scoring engine — 4DA Docs"
description: "How 4DA turns 20+ noisy sources into a feed of only what earns its place."
permalink: "/docs/how-it-works/"
templateEngineOverride: md
---

# The scoring engine

4DA's job is subtraction. It reads a lot and shows you a little — only the items that clear a gate built from your own code. Here's the pipeline, end to end.

## The pipeline

```
Your codebase                     20+ sources
  (ACE scanner + Git watch)        (background adapters)
        \                             /
         v                           v
        +-------------------------------+
        |   5-axis scoring + gate       |
        |   pass 2+ of 5 to survive     |
        +---------------+---------------+
                        |
                        v
        +-------------------------------+
        |   12 quality multipliers      |
        |   depth, novelty, coherence…  |
        +---------------+---------------+
                        |
                        v
        +-------------------------------+
        |   LLM verification (top items)|
        |   strict 1–5 relevance rubric |
        +---------------+---------------+
                        |
                        v
                What survived
```

## The confirmation gate

Every item is scored on **five independent axes** — semantic context, your interests, real-time signals from your Git activity, direct dependency matches, and what you've learned to save or dismiss. An item must pass **2 or more** to surface.

Single-axis matches are hard-capped at **28%**. No matter how strong one signal is, it cannot pass alone. This is what makes the feed hard to game: keyword-stuffing hits one axis, and without matching your codebase, your installed packages, *and* your recent work, the gate rejects it. Your `Cargo.lock` doesn't lie.

Full breakdown: **[The 5 axes](/docs/scoring/)**.

## Quality multipliers

What clears the gate then runs through 12 multipliers — content depth, novelty detection (introductory content is demoted; new releases and security advisories are boosted), title–body coherence, competing-tech penalties, and intent scoring from your recent work. Every constant is calibrated across 9 simulated developer personas with 245 labeled test items.

## LLM verification

After keyword scoring, an optional LLM layer verifies the top items against your full context — stack, dependencies, recent commits, anti-technologies, engagement history — on a strict 1–5 rubric:

- **5 — must-read:** a security alert for *your* dependency; a breaking change *you* must act on
- **3 — worth knowing:** a tool that fits *your* exact stack
- **1 — noise:** mentions your tech but isn't actionable

This is where the gold surfaces: articles the keyword pipeline misses because there's no literal overlap, but the model understands the conceptual relevance to your project. You run this on your own compute — local via Ollama, or your own cloud key.

## Anti-gaming

Content built to score still can't win. Titles must deliver on what they promise (claim "React + Rust + Tauri" but only cover React → penalty). Repeating a keyword concentrates and *hurts* the score. Gamed articles get dismissed; sources that produce dismissed content lose reputation. Gaming becomes self-defeating.

Next: **[The 5 axes](/docs/scoring/)** or **[Sources](/docs/sources/)**.
