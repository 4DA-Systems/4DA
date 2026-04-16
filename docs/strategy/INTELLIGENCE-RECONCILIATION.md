# Intelligence Reconciliation Plan

**Status:** Approved 2026-04-16. This is THE plan. No v1.x follow-up architecture discussions on this system.
**Owner:** Lead development (Claude + founder)
**Supersedes:** Any prior ad-hoc treatment of Momentum, AWE UI exposure, Preemption/Blindspots integration.

---

## Problem statement

4DA has five overlapping intelligence systems — AWE, Preemption, Blind Spots, Knowledge Decay, Signal Chains — each with its own backend, type system, confidence scale, UI, and tab. They all answer variations of one question (*"what from the signal firehose matters to this user right now?"*) and they do not know about each other.

Symptoms observed pre-launch:
- The Momentum tab has become a dashboard of vanity and empty states.
- The same concept (e.g. "react") surfaces as a knowledge gap, an uncovered dep, a preemption alert, a signal chain, and a potential decision — five surfaces, one truth.
- AWE presents as a user feature (with a "Run wisdom" button, a "Decisions tracked" counter, and a "Wisdom Trajectory" panel) but has no natural trigger surface, no input conversion layer, no output rendering, no feedback wiring, and no cold-start strategy. It is effectively a dev-tool in a user costume.
- Knowledge gaps trigger on zero-day-stale deps. Active-work topic extraction returns escaped regex fragments. Multiple metrics divide by zero and display 100%. Users see the app looking busy and broken simultaneously.

## Target architecture

**One Intelligence Core. AWE is its judgment spine. Preemption / Blind Spots / Decay / Chains are lenses onto the Core's output. Not independent systems.**

```
                     ┌────────────────────────────────┐
                     │       INTELLIGENCE CORE         │
INPUT PLANE          │                                 │      OUTPUT PLANE
─────────────        │  ┌──────────────────────────┐  │      ─────────────
ACE (projects/       │  │   AWE Judgment Spine     │  │      Evidence Items
  dependencies)  ──► │  │   (7-stage pipeline)     │  │  ──► (canonical shape)
User feedback    ──► │  └──────────────────────────┘  │
Ingested items   ──► │  ┌──────────────────────────┐  │
Git history      ──► │  │  Evidence Materializer   │  │
Curated corpus   ──► │  └──────────────────────────┘  │
                     └────────────────────────────────┘
                                    │
                                    ▼
          ┌───────────────┬─────────────────┬────────────────┬───────────┐
          │  Briefing     │   Preemption    │  Blind Spots   │ Evidence  │
          │  (today)      │   (ahead)       │  (missed)      │  (proof)  │
          └───────────────┴─────────────────┴────────────────┴───────────┘
```

- All intelligence emits one canonical type: `EvidenceItem` (see `EVIDENCE-ITEM-SCHEMA.md`).
- All lenses consume `EvidenceItem`, differing only in which slice they surface and how they present.
- AWE never has its own dashboard. AWE's presence in the UI is felt as *better reasoning on every item*, plus four sanctioned user surfaces:
  1. **Confession Box** — global `⌘.` shortcut, opens a single-input modal for ad-hoc decisions.
  2. **Decision Brief** — the canonical AWE output card (one scrollable card, five fixed sections).
  3. **Commitment Contract** — the refutation-condition follow-up prompt on accepted decisions.
  4. **Retrospective Card** — weekly retrospectives surfaced in the Evidence lens.

## Tab rationalization — 12 → 5

Main nav:
- **Briefing** (today's signal)
- **Preemption** (forward-looking alerts)
- **Blind Spots** (coverage gaps)
- **Evidence** (compound proof + AWE surfaces) — replaces Momentum
- **Results** (raw feed / search)

Sidebar (contextual, not main nav):
- **Saved** (bookmarks)
- **Profile** (developer DNA)
- **Playbook** (documentation, not intelligence)

Advanced toggle (hidden by default):
- **Console**, **Toolkit**, **Calibrate**

## Kill list (explicit)

### Frontend — delete

- `src/components/Momentum.tsx` and all of `src/components/momentum/*`
- `src/components/awe/MomentumWisdomTrajectory.tsx`
- `src/components/awe/BriefingWisdomSignal.tsx`
- `src/components/awe/PlaybookWisdomResonance.tsx`
- `src/components/awe/ProfileWisdomDna.tsx`
- `src/components/awe/momentum-wisdom-helpers.tsx`
- `src/components/CompoundAdvantageScore.tsx` (vanity)
- `src/components/IntelligenceProfileCard.tsx` (vanity)
- `src/components/CategoryChapterView.tsx` (merged into Results)

### Backend — delete (after UI deletion)

- The `get_compound_advantage` command handler
- The `get_active_work_context` command handler
- `extract_rich_universal` TODO-scan in `src-tauri/src/ace/watcher.rs` (the regex-garbage source)
- The hardcoded `security` / `active_problem` / `error_handling` topic buckets in the same file

### Types — collapse into canonical

- `PreemptionAlert` + `AlertEvidence` + `UncoveredDep` + `MissedSignal` + `KnowledgeGap` + AWE's `Decision` → **one** `EvidenceItem`
- `AlertUrgency` + `GapSeverity` + `risk_level` + `priority` → **one** `Urgency`
- Five separate `explanation` strings → **one** field populated by AWE.articulate
- Five separate confidence representations → **one** `Confidence` with provenance

### i18n — retire

All keys matching `momentum.*`, `awe.momentum.*`, `intelligence_profile.*`, `compound_advantage.*`.

## Ship phases (each has a binding done-when)

| Phase | What | Done when |
|-------|------|-----------|
| **0** | Lock the plan (this doc + schema + doctrine) | Three docs committed, referenced in CLAUDE.md |
| **1** | Kill dead code (frontend first, backend follows) | `pnpm validate:all` green, ~3000 LOC removed |
| **2** | Define `EvidenceItem` canonical type | Trait compiles, ts-rs exports generate, roundtrip tests pass |
| **3** | Collapse Preemption to `EvidenceItem` | Preemption tab visually identical, backed by shared types |
| **4** | Collapse Blind Spots to `EvidenceItem` | Same for Blind Spots |
| **5** | Collapse Knowledge Decay + Signal Chains | No UI references these types directly |
| **6** | Context Bridge — AWE's DeveloperContext expanded 5→17 fields | Transmutations carry full user situation |
| **7** | Cold Start Layer 1 — Git history miner | Fresh install has ≥20 seeded decisions from git |
| **8** | Cold Start Layer 2 — Curated 200-decision corpus | `awe transmute` returns ≥3 precedents for common queries on fresh install |
| **9** | AWE Judgment Spine wiring | Every `EvidenceItem` has AWE-generated explanation + calibrated confidence + precedents |
| **10** | The Confession Box + Decision Brief | `⌘.` from any tab returns a useful brief in <5s |
| **11** | Commitment Contract + Refutation Monitor | Accepted decision + refutation signal → Refutation card in Evidence |
| **12** | Evidence tab (replaces Momentum slot) | Three sections render real data, no empty-state pseudo-gauges |
| **13** | Pre-launch hardening | 7-day founder soak with zero "this isn't right" reports |
| **14** | Ship | Release Gate passes |

**Realistic total:** 11–13 weeks of focused work.

## Anti-rot standards (enforced in code review)

Canonical form at `.claude/rules/intelligence-doctrine.md`. Summary:

1. One canonical type per concept.
2. No new tabs without written justification against the 5-tab reconciliation.
3. No vanity metrics.
4. No backend command without a lens.
5. No AWE UI panels outside the four sanctioned surfaces.
6. Cold-start is non-negotiable — features ship silent until data arrives.
7. Materializer trait is the only entry point for intelligence data.
8. Dead code is deleted (no commented sections, no `// TODO: reinstate`).
9. Every `EvidenceItem` is schema-validated at runtime in dev.
10. 7-day founder dogfood required before any intelligence surface ships.

## What this plan explicitly does NOT include (scope-creep defense)

- Precedent Marketplace (cross-user anonymized precedents) — deferred to v2.x.
- Co-Pilot Mode (real-time annotations in the reader) — deferred to v1.5.
- Wisdom Voice (spoken judgment) — deferred to v2.x.
- Second Brain Export — deferred to v1.3.
- Decision Heatmap — killed.

---

**Reference:** Full session transcript and architectural audit captured in the conversation that produced this plan (2026-04-16). The overlapping-systems audit and the decision to collapse to one pipeline were the turning point.
