# EvidenceItem — The Canonical Intelligence Type

**Status:** Contract. Any intelligence surface that emits output MUST produce `Vec<EvidenceItem>`. No bespoke types.
**Owner:** Intelligence Reconciliation (see `INTELLIGENCE-RECONCILIATION.md`).

---

## Why one type

Five parallel intelligence systems shipped five parallel type systems (`PreemptionAlert`, `UncoveredDep`, `MissedSignal`, `KnowledgeGap`, `SignalChainWithPrediction`). Each duplicates the same fields with different names. Each has a different confidence scale. Each hand-writes its own "why this matters" text. Consumers cannot compare, deduplicate, or route items across systems.

`EvidenceItem` is the single type every lens consumes. Producers differ in how they materialize it. Consumers differ in which `kind` they render. Everything else is shared.

---

## The struct

```rust
/// A single unit of actionable intelligence surfaced to the user.
/// Produced by any Evidence Materializer. Consumed by any Lens.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct EvidenceItem {
    /// Stable identifier. Must survive backend restart (derived from content hash + source).
    pub id: String,

    /// The category of evidence. Determines default rendering hints.
    pub kind: EvidenceKind,

    /// One-line summary. ≤ 120 chars. No trailing period.
    pub title: String,

    /// Full explanation. Produced by the judgment pipeline — not hand-written by the materializer.
    /// May be empty during transition phases; must be non-empty after Phase 9 wiring.
    pub explanation: String,

    /// Calibrated confidence with provenance. Replaces bespoke 0.0–1.0 floats.
    pub confidence: Confidence,

    /// Shared urgency scale. Replaces AlertUrgency / GapSeverity / risk_level / priority.
    pub urgency: Urgency,

    /// 0.0 = fully reversible (easy to change mind), 1.0 = irreversible.
    /// None only when reversibility is conceptually N/A for this kind (e.g. Retrospective).
    pub reversibility: Option<f32>,

    /// Citations backing the claim. Never empty for user-surfaced items.
    pub evidence: Vec<EvidenceCitation>,

    /// Projects this touches (empty if not project-scoped).
    pub affected_projects: Vec<String>,

    /// Dependencies this touches (empty if not dep-scoped).
    pub affected_deps: Vec<String>,

    /// Actions the user can take. At least one action required for actionable kinds.
    pub suggested_actions: Vec<Action>,

    /// Precedents from Wisdom Graph (user's history + curated corpus + public corpus).
    /// Empty allowed on cold-start; should populate after Phase 8.
    pub precedents: Vec<PrecedentRef>,

    /// User-set refutation condition (only for EvidenceKind::Decision with accepted outcome).
    /// Monitored by the refutation watcher. Surfaces a Refutation item when matched.
    pub refutation_condition: Option<String>,

    /// Which lenses this item is a candidate for. The Evidence Materializer sets these;
    /// each lens filters by its allowed kinds + its own query.
    pub lens_hints: LensHints,

    /// Unix timestamp, millis. When the item was produced by its materializer.
    pub created_at: i64,

    /// Unix timestamp, millis. When the underlying signal is predicted to stop being relevant.
    /// None for durable items (e.g. Retrospectives, Decisions).
    pub expires_at: Option<i64>,
}
```

---

## Supporting types

### EvidenceKind

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, Hash)]
#[ts(export, export_to = "bindings/")]
pub enum EvidenceKind {
    /// Forward-looking alert. Security advisory, breaking change, migration window.
    Alert,

    /// Coverage gap. Dependency or topic the user is not watching.
    Gap,

    /// Missed signal. Item that was relevant but the user did not see.
    MissedSignal,

    /// Connected signals forming a pattern over time.
    Chain,

    /// A decision the user is weighing (inferred or typed).
    Decision,

    /// A retrospective on a past decision with new signal.
    Retrospective,

    /// A refutation condition has been met.
    Refutation,

    /// A precedent relevant to the user's current context (informational, not actionable).
    Precedent,
}
```

### Urgency

Shared scale across all lenses. Replaces `AlertUrgency`, `GapSeverity`, `risk_level`, `priority`.

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "bindings/")]
pub enum Urgency {
    /// Act within 24 hours. Security, data loss risk, production incident risk.
    Critical,

    /// Act within the week. Breaking change arriving, deprecation with near deadline.
    High,

    /// Act within the month. Notable shifts, upgrade windows.
    Medium,

    /// Informational. No action required.
    Watch,
}
```

### Confidence

Replaces bare `f32` values. Every confidence score carries a provenance tag so callers can weight appropriately.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Confidence {
    /// 0.0–1.0.
    pub value: f32,

    /// Where this number came from. Never hide this from the UI in power mode.
    pub provenance: ConfidenceProvenance,

    /// If provenance is Calibrated, the N of samples backing the calibration.
    /// None for Checklist/Heuristic/LlmAssessed.
    pub sample_size: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum ConfidenceProvenance {
    /// Keyword/pattern matching. Fast, deterministic, limited.
    Checklist,
    /// Weighted formula (reversibility, timing, etc).
    Heuristic,
    /// Bayesian posterior with ≥10 feedback samples. sample_size required.
    Calibrated,
    /// LLM judgment from calibration stage.
    LlmAssessed,
}
```

### EvidenceCitation

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct EvidenceCitation {
    /// Source (e.g. "hackernews", "github-advisory", "git-history", "curated-corpus").
    pub source: String,

    /// Human-readable title of the cited artifact.
    pub title: String,

    /// URL if available. None for inferred signals (e.g. git-history).
    pub url: Option<String>,

    /// 0.0 = today, larger = older. Expressed in days (not ms) for legibility.
    pub freshness_days: f32,

    /// Why this citation was selected as evidence for the claim. ≤ 200 chars.
    pub relevance_note: String,
}
```

### Action

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Action {
    /// Canonical id. Frontend dispatches by this id.
    /// Allowed: "dismiss", "acknowledge", "snooze_7d", "brief_this",
    ///          "view_source", "investigate", "accept_decision",
    ///          "reject_decision", "set_refutation".
    pub action_id: String,

    /// Display label. Keep short.
    pub label: String,

    /// Hover description. Explains what the action does.
    pub description: String,
}
```

### PrecedentRef

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct PrecedentRef {
    /// Decision id in the Wisdom Graph.
    pub decision_id: String,

    /// One-line summary of the precedent decision.
    pub statement: String,

    /// Outcome if known. None if still pending feedback.
    pub outcome: Option<PrecedentOutcome>,

    /// Origin: "user-history" / "curated-corpus" / "public-corpus".
    pub origin: String,

    /// Similarity to the current situation, 0.0–1.0.
    pub similarity: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "bindings/")]
pub enum PrecedentOutcome {
    Confirmed,
    Refuted,
    Partial,
    Pending,
}
```

### LensHints

Which lenses the item is allowed to surface in. Producers set these; lenses filter.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "bindings/")]
pub struct LensHints {
    pub briefing: bool,
    pub preemption: bool,
    pub blind_spots: bool,
    pub evidence: bool,
}
```

---

## Materializer trait

Every existing intelligence system implements this. Output is the canonical `EvidenceItem`.

```rust
#[async_trait]
pub trait EvidenceMaterializer: Send + Sync {
    /// Materializer name, for logging/provenance.
    fn name(&self) -> &'static str;

    /// Produce evidence items from the materializer's data source.
    async fn materialize(&self, ctx: &MaterializeContext) -> Result<Vec<EvidenceItem>>;
}
```

`MaterializeContext` carries the user's DeveloperContext, time window, and any filter hints.

---

## Validation contract

Every `EvidenceItem` surfaced anywhere passes these checks (runtime-validated in dev, lint-enforced in prod):

| Field | Rule |
|-------|------|
| `title` | Non-empty, ≤ 120 chars, no trailing period |
| `explanation` | Non-empty after Phase 9 (judgment spine wired); may be empty earlier |
| `confidence.value` | 0.0 ≤ value ≤ 1.0 |
| `confidence.provenance` | If Calibrated, `sample_size` required and ≥ 10 |
| `reversibility` | If present, 0.0 ≤ x ≤ 1.0 |
| `evidence` | Non-empty for all user-surfaced kinds except Retrospective |
| `suggested_actions` | Non-empty for kinds: Alert, Gap, Decision, Refutation |
| `precedents` | May be empty (cold-start); must populate after Phase 8 |

Failure of any rule in dev = hard panic with diagnostic. In prod = item dropped with structured log.

---

## Migration notes

Existing types map to `EvidenceItem` as follows (reference for Phases 3–5):

| Legacy type | `EvidenceKind` | Field mapping |
|-------------|----------------|---------------|
| `PreemptionAlert` | `Alert` | title, explanation, evidence, affected_*, urgency, confidence, suggested_actions — direct |
| `UncoveredDep` | `Gap` | title = name, urgency from risk_level, evidence from recent signals |
| `MissedSignal` | `MissedSignal` | title, evidence = [{title, url, source_type, freshness}], relevance_note = why_relevant |
| `KnowledgeGap` | `Gap` | title from dependency, urgency from gap_severity, evidence from missed_items |
| `SignalChainWithPrediction` | `Chain` | title from chain_name, explanation from prediction.forecast |

During Phase 3–5 transition, each materializer returns `EvidenceItem` directly. The legacy UI consumers (`PreemptionView`, `BlindSpotsView`) are refactored to consume `EvidenceItem` before their source types are deleted.

---

**Authoritative source:** this document. Divergence between code and schema must update the code, not the schema. Schema changes require a new ADR (architectural decision record) and version bump.
