# Intelligence Doctrine

**Non-negotiable rules for all intelligence surfaces in 4DA.** Enforced in code review. Violations block merge.

Canonical plan: `docs/strategy/INTELLIGENCE-RECONCILIATION.md`.
Canonical type: `docs/strategy/EVIDENCE-ITEM-SCHEMA.md`.

---

## The ten rules

### 1. One canonical type per concept

All intelligence output flows through `EvidenceItem`. Urgency is `Urgency`. Confidence is `Confidence` with provenance. A PR that introduces a new `Alert` / `Gap` / `Signal` / `Recommendation` / `Insight` struct is rejected. If a new *dimension* is needed, extend the canonical type via an ADR.

### 2. No new tabs without doctrine signoff

Main nav is locked at five tabs: Briefing · Preemption · Blind Spots · Evidence · Results. A new tab requires a written justification against the five-tab reconciliation, reviewed against the "what unique job does this tab do that existing tabs can't?" test.

### 3. No vanity metrics

Every number displayed to the user must pass the test: *"what action does this inform?"* If the answer is "none / it just looks busy," the number is cut. Explicit banned patterns:

- "Items monitored"
- "Sources producing"
- "Validated principles: 0"
- "Decisions tracked: 0"
- Coverage gauges whose denominator is always ~= numerator
- Sparklines synthesized from a single data point
- Percentages that default to 0% or 100% when the denominator is zero

### 4. No backend command without a lens

New backend intelligence commands (anything returning ranked items, alerts, recommendations) must route through the Evidence Materializer trait and emit `Vec<EvidenceItem>`. Direct frontend fetches of bespoke JSON blobs are rejected.

### 5. No AWE UI panels outside the four sanctioned surfaces

AWE is invisible infrastructure. Its only user-visible surfaces are:

1. Confession Box (global `⌘.` modal)
2. Decision Brief card
3. Commitment Contract prompt
4. Retrospective Card (in Evidence)

No "Wisdom Trajectory," no "Decisions Tracked" counter, no "Run Wisdom" button, no AWE-branded panel on any tab. AWE's contribution to all other surfaces is through item *reasoning* (explanation, calibrated confidence, precedents) not through its own UI.

### 6. Cold-start is non-negotiable

A feature that requires >7 days of user data before it lights up ships *silent* (no UI surface) until the data arrives. Empty states that say *"no data yet"* to a first-day user are banned. All cold-start inheritance flows from the three-layer strategy:

- Layer 1: Git history mining (personal priors, on first run)
- Layer 2: Curated domain corpus (domain priors, shipped in bundle)
- Layer 3: Public decision corpus (network priors, post-launch)

### 7. Materializer trait is the only entry point

UI components do not query the DB directly for intelligence data. They subscribe to materializer outputs. A PR that adds a direct DB query for intelligence data in a React component is rejected.

### 8. Dead code is deleted

No commented-out sections. No `// TODO: reinstate later`. No `// removed for now`. No backwards-compat shims for features that shipped and were removed. If it's unused, it's gone. Reflog and git history preserve everything.

### 9. Every EvidenceItem is schema-validated at runtime in dev

The validation contract in `EVIDENCE-ITEM-SCHEMA.md` runs at materializer output in debug builds. Violations hard-panic with a diagnostic. In release builds, violations drop the item and emit a structured log.

### 10. 7-day founder dogfood before any intelligence surface ships

No intelligence surface ships on a release branch until the founder has used it in production for seven consecutive days and filed zero "this isn't right" issues against it. The dogfood window is part of the phase's done-when gate, not an optional extra.

---

## Enforcement

- **Pre-commit:** (where automatable) lint for banned patterns — `IntelligenceProfile` strings, `CompoundAdvantage` imports, AWE-panel component names, etc.
- **PR review:** every intelligence-adjacent PR gets the doctrine checklist. Reviewer confirms each rule.
- **Post-merge:** runtime schema validation catches what lint cannot.
- **Pre-release:** the 7-day founder dogfood is a non-negotiable release gate.

---

## If a rule is wrong

These rules were written to solve the specific failure modes observed during the Momentum / AWE / Preemption / Blind Spots / Knowledge Decay accumulation incident (2026-04-16). If future learning invalidates a rule:

1. Write an ADR explaining which specific failure mode the rule no longer needs to prevent.
2. Cite what replaced it.
3. Only then update this document.

Rules do not quietly erode. They are retired deliberately or they stand.
