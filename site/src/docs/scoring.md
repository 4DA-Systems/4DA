---
layout: docs.njk
eyebrow: "How it works"
title: "The 5 axes — 4DA Docs"
description: "The five independent signals 4DA scores every item against, and the gate that rejects single-axis matches."
permalink: "/docs/scoring/"
templateEngineOverride: md
---

# The 5 axes

Every item is scored on five independent axes. This is the core of what 4DA calls PASIFA scoring — the reason a keyword match alone can't buy its way into your feed.

| Axis | What it measures |
|---|---|
| **Context** | Semantic similarity to your active codebase |
| **Interest** | Alignment with your declared topics |
| **ACE** | Real-time signals from your Git commits and file edits |
| **Dependency** | Direct matches against your installed packages |
| **Learned** | Reserved — held out of scoring until it can be validated against your explicit feedback |

## The 2-of-5 gate

An item must pass **2 or more** axes to surface. Single-axis matches are hard-capped at **28%** — one strong signal, no matter how strong, cannot pass alone.

This is deliberate. A single axis is easy to trigger by accident (or on purpose). Requiring corroboration across independent signals — *semantic* relevance **and** a *dependency* you actually installed, say — is what separates "mentions your tech" from "you need to see this."

## Quality multipliers

Passing the gate is necessary, not sufficient. Survivors run through 12 multipliers:

- **Content depth** — thin content is demoted
- **Novelty detection** — introductory posts down; new releases and security advisories up
- **Title–body coherence** — a title has to deliver on its promise
- **Competing-tech penalties** — content pushing alternatives to your stack is discounted
- **Intent scoring** — recent Git and file activity nudges what surfaces toward what you're working on now

## Calibration

None of these constants are guesses. The pipeline is benchmarked against **9 simulated developer personas** (Rust systems, Python ML, fullstack TypeScript, DevOps/SRE, mobile, first-run, power user, stack switcher, niche specialist) with **245 labeled test items** scored as relevant or noise — 1,997 scored evaluations in total.

Measured result across those personas: **93% of content filtered as noise, 98.9% of actual noise correctly rejected**, at 86% precision. The suite is in the repo and the numbers regenerate with one command:

```bash
cd src-tauri && cargo test scoring::simulation -- --nocapture
```

CI enforces floors rather than the headline figures — aggregate precision at or above 0.70, and at least 80% noise rejection for every one of the nine personas — so a scoring regression fails the build. Your own rejection rate — computed from your data, not ours — is shown in the app.

> **Accurate first.** 4DA never shows intelligence the system can't stand behind. Correct results from a capable model beat fast results from a weak one.

Next: **[Privacy & BYOK](/docs/privacy/)**.
