# Embedding Calibration Architecture

> ADR-028 · 2026-04-25 · Status: Accepted

## Context

4DA's scoring pipeline (PASIFA) uses embedding similarity as one of 12+ independent scoring signals. Raw cosine similarity from embedding models compresses into a narrow band (typically 0.30–0.60), which must be stretched to a usable range via a sigmoid calibration function.

Different embedding models produce different similarity distributions for the same content:

| Model | Typical Mean | Typical Spread |
|-------|-------------|----------------|
| text-embedding-3-small (OpenAI) | 0.48 | 0.40–0.56 |
| nomic-embed-text (Ollama) | ~0.42 | ~0.34–0.50 |
| all-MiniLM-L6-v2 | ~0.38 | ~0.30–0.46 |

Hardcoding sigmoid parameters (center=0.48) for all models causes systematic mis-scoring when users run a different model. Items that should be relevant are silently dropped; items that should be noise pass through.

## Decision

### Embedding Layer: Opinionated Default, Adaptive Calibration

**Default embedding model:** `nomic-embed-text` via Ollama (local, free, private).

**Calibration strategy:** Three-tier fallback:

1. **Auto-compute from observed data.** After each analysis run, sample raw cosine similarities from the database. Compute mean and standard deviation. Derive sigmoid center (= mean) and scale (= 2.5 / stddev). This adapts to any embedding model automatically.

2. **Known-model lookup table.** For popular models (nomic-embed-text, text-embedding-3-small, mxbai-embed-large, etc.), ship pre-measured parameters. Used when insufficient data exists for auto-computation (<50 samples).

3. **Compile-time defaults.** The scoring DSL (`pipeline.scoring`) defines center=0.48, scale=12.0 as the last-resort fallback. These only apply before any calibration has run.

### LLM Layer: User Choice with Capability Tiers

Users choose their LLM provider and model. 4DA detects the model's capability tier on first use:

- **Full** (Claude, GPT-4o): All intelligence features enabled — LLM reranking, adversarial deliberation, briefing synthesis, analysis text.
- **Good** (70B+ local models, Mixtral): Most features enabled, analysis text quality may vary.
- **Basic** (small local models <14B): Pipeline scoring only. Heuristic explanations shown instead of LLM-generated text. Adversarial deliberation disabled. Briefing uses template format.

Features are gated by tier automatically. The reconciler caps LLM advisor impact at ±0.15 regardless of tier, ensuring the deterministic pipeline remains authoritative.

## Rationale

### Why lock embeddings but not LLMs?

**Embeddings are infrastructure.** Users have no preference between `nomic-embed-text` and `text-embedding-3-small`. They care about: does it work, is it free, is it private. All three answers point to a single opinionated default with adaptive calibration.

**LLMs are the product surface.** Users see and evaluate LLM output directly — the "Why this matters" text, the briefing narrative, the adversarial reasoning. They have genuine preferences about cost, quality, speed, and privacy. Choice matters here.

### Why auto-compute instead of per-model lookup tables?

Lookup tables cover known models. Auto-computation covers unknown models (custom fine-tunes, future models, self-hosted endpoints). The combination ensures reliability regardless of what the user runs.

### Why not give embedding model choice to users?

1. **Calibration complexity:** Each embedding model needs its own sigmoid parameters. More models = more failure modes.
2. **Testing surface:** Quality validation across N embedding models is O(N). One model = one target.
3. **Support burden:** "My scoring seems off" is unanswerable when you don't know the user's embedding distribution.
4. **No user value:** Embedding model choice does not improve the user's intelligence quality. It only creates risk.

### Why capability tiers instead of treating all LLMs equally?

Small models (8B parameters) fundamentally cannot produce the same quality structured output as large models. Pretending otherwise leads to:
- Verbose filler text in "Why this matters" that erodes trust
- Adversarial deliberation that filters legitimate items or passes noise
- Briefing synthesis that hallucinates product names
- Generic reasoning that provides no actionable insight

Honest tiers build trust. A user who sees crisp heuristic explanations on a Basic tier model has a better experience than one who sees hallucinated LLM filler.

## Architecture

```
Analysis Pipeline
    │
    ├── Embedding Similarity (adaptive calibration)
    │   ├── Auto-computed sigmoid (observed distribution)
    │   ├── Known-model lookup (pre-measured)
    │   └── DSL defaults (compile-time fallback)
    │
    ├── 11 Other Scoring Signals (model-independent)
    │   ├── Keyword matching
    │   ├── Dependency detection
    │   ├── ACE tech detection
    │   ├── Content sophistication
    │   ├── Source authority
    │   ├── Learned affinity
    │   ├── Taste vector
    │   ├── Temporal freshness
    │   ├── Domain relevance
    │   ├── Competing tech penalty
    │   └── Stack pain points
    │
    ├── Multi-Signal Confirmation Gate (2+ signals required)
    │
    └── LLM Advisory Layer (tier-gated)
        ├── Full:  Reranking + Adversarial + Analysis text + Briefing
        ├── Good:  Reranking + Adversarial + Analysis text + Briefing
        └── Basic: Pipeline-only scoring + Heuristic explanations
```

## Consequences

**Positive:**
- Scoring accuracy is consistent regardless of embedding model
- Users with Ollama get correct relevance from day one
- LLM quality expectations are set honestly
- Support surface is reduced (one embedding model to validate)
- Auto-calibration handles future models without code changes

**Negative:**
- First-run with <50 items falls back to lookup table or defaults (calibration needs data)
- Known-model table must be maintained as new models emerge
- Capability tiers may frustrate users who believe their small model is "good enough"

**Neutral:**
- The scoring DSL defaults remain as compile-time constants (no DSL changes needed)
- Existing calibration fitter (for LLM judge scores) is unaffected
- Provenance tracking continues to stamp model identity on every score

## Implementation

- `src-tauri/src/embedding_calibration.rs` — Adaptive sigmoid parameter computation
- `src-tauri/src/llm_capability.rs` — Model capability tier detection and feature gating
- `src-tauri/src/scoring/calibration.rs` — Uses adaptive parameters instead of hardcoded constants
- Initialization: runs on app startup after DB connection, before first analysis
- Re-calibration: triggers after re-embedding (model change) and after each analysis run
