# AWE Experience Plan — How Wisdom Surfaces in 4DA

## The Perception Challenge

Some developers will feel threatened by a tool that tracks their decisions and
tells them they're biased. The instinct is to resist. This is the same resistance
people had to spell-checkers in the 90s, linters in the 2000s, and code review
tools in the 2010s. Every tool that surfaces uncomfortable truth faces this.

The solution is NOT to hide what AWE does. The solution is to make the experience
feel like a superpower, not a judgment. The developer should feel smarter when
using 4DA, not watched.

## Design Principles

1. **AWE never criticizes. AWE observes patterns.**
   "You chose simpler tools 5/5 times" is a pattern.
   "You have sunk cost bias" is a judgment.
   Same data. Different framing. First one feels empowering.

2. **AWE is opt-in by behavior, not by toggle.**
   No "enable AWE" switch. AWE activates gradually as the user
   makes decisions naturally. First decision: nothing visible.
   Third decision: subtle pattern note. Tenth decision: personalized
   wisdom appears. The user never "turns it on" — it emerges.

3. **Show the user THEIR data, not AWE's analysis.**
   "Your last 5 database decisions all confirmed" is their data.
   "AWE predicts 78% success rate" is AWE's analysis.
   Lead with their data. Let them draw conclusions.

4. **Wisdom should feel like memory, not surveillance.**
   "Last time you faced this, you chose X and it worked" feels
   like a friend reminding you. Not like a system tracking you.

## Implementation Phases

### Phase 1: Natural Emergence (Backend — DONE)
- [x] AWE Wisdom Graph fills automatically from git commits
- [x] Decisions bridge from 4DA decision-memory to AWE
- [x] Principles inject as context chunks (1.5x weight)
- [x] WisdomPulse shows in briefing (self-hides when empty)

### Phase 2: Pattern Visibility (Frontend — Next)
Goal: Show the user their patterns without making it feel like analysis.

#### 2A. Enrich WisdomPulse
Current: Shows counts ("4 principles, 12 decisions detected")
Target: Shows the ACTUAL top principle in natural language:

```
┌─────────────────────────────────────────┐
│  Your pattern: simpler tools outperform │
│  5 decisions confirmed this.            │
│                                         │
│  3 outcomes pending · Brier 0.12        │
└─────────────────────────────────────────┘
```

No "AWE" label. No gold dot. Just "Your pattern:" — it's THEIR wisdom.

Implementation:
- Tauri command: `get_awe_summary` → returns top principle + stats
- WisdomPulse.tsx calls on mount, renders natural language
- Falls back to hidden when no principles exist

#### 2B. Result Item Wisdom Annotation
When a result matches a domain where the user has validated principles,
show a subtle one-line note:

```
"Why We Switched from SQLite to Postgres" — HN 342pts
  Matches your pattern: you chose simpler databases 4/4 times ←
```

Not "AWE says this contradicts your principle." Just "matches your pattern."
The user connects the dots themselves.

Implementation:
- During scoring, check if result's topic overlaps with wisdom chunk topics
- If overlap > threshold, attach annotation to result metadata
- ResultItem.tsx renders annotation when present

#### 2C. Decision Moment Prompt
When the user engages deeply with content (clicks through, reads >30s),
and the content is decision-relevant, surface a soft prompt:

```
"Thinking about this? You've made 4 similar decisions before."
[See your history]  [Dismiss]
```

Not "Transmute this decision!" — that sounds clinical.
"See your history" is natural. It's THEIR history.

Implementation:
- Track engagement signals (click, time-on-content)
- Cross-reference content domain with AWE decision domains
- Surface prompt after engagement threshold met
- One-click opens decision history panel

### Phase 3: Wisdom Voice (Optional, Power Users)
For users who explicitly want AWE's full voice — accessible from
the Insights view alongside DecisionMemory:

#### 3A. Transmute Panel
Inline form in Insights view:
```
What are you deciding?
[______________________________________]
[Transmute]  [Voice Mode]  [Challenge]
```

Output renders as a card below the form. Not a new page.
Not a modal. Inline, contextual, part of the flow.

#### 3B. Decision Timeline
Visual timeline of past decisions with outcomes:
```
Mar 2026: Chose Tauri over Electron ✓ confirmed
Mar 2026: Chose SQLite over Postgres ✓ confirmed
Mar 2026: Chose fastembed ✗ reverted
          ↓ Principle emerged: "Simpler tools outperform"
```

This is the compounding loop VISIBLE. The user sees their
wisdom forming in real time. No analysis language. Just
decisions → outcomes → patterns.

### Phase 4: Ambient Intelligence (Long-term)
AWE becomes invisible. The user doesn't interact with "AWE."
They interact with 4DA, and 4DA is smarter because of AWE.

- Content ranking silently influenced by wisdom chunks
- Briefing sections informed by decision patterns
- Source quality weighted by past decision relevance
- No AWE branding needed — it's just "how 4DA works"

## Language Guide

### DO say:
- "Your pattern"
- "Your history shows"
- "You've made N similar decisions"
- "Last time you faced this"
- "Matches your experience"
- "Based on your outcomes"

### DON'T say:
- "AWE detected"
- "Bias warning"
- "Confidence calibration"
- "Transmutation pipeline"
- "Wisdom engine"
- "Decision analysis"

The technical terms are for us. The user sees their own patterns.

## The Positioning (for developers who investigate)

When developers look under the hood (and they will), they should find:
1. Open-source Rust engine (12 crates, 653 tests)
2. Local-only processing (nothing leaves the machine)
3. Every heuristic weight documented with justification
4. Brier calibration proving accuracy over time
5. The compounding loop explained clearly

The reaction we want: "Oh, this is real engineering, not marketing."
The code IS the marketing. Developers trust code they can read.

## What We're NOT Building

- No social features (comparing your patterns to other users)
- No cloud analytics (your decisions never leave your machine)
- No gamification (no points, badges, or streaks)
- No notifications (no "you have 5 pending decisions!")
- No dashboards (no charts, graphs, or metrics displays)

AWE surfaces wisdom when it's relevant. That's it.
