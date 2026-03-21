# PASIFA Scoring Degradation Profile

**Date:** 2026-03-22
**Status:** Verified — system degrades gracefully, no catastrophic collapse

---

## Zero-Vector Scenario

When embeddings return `[0.0; 384]` (Ollama down, no API key configured), the scoring pipeline operates in degraded mode. This document quantifies the impact.

## Axis Status Matrix

| Axis | Full Embeddings | Zero Vectors | Degradation |
|------|----------------|--------------|-------------|
| **Context** | KNN similarity against codebase chunks | Returns 0.0 | COMPLETE LOSS |
| **Interest** | Cosine sim against declared interest embeddings | Returns 0.0 (norm < epsilon guard) | COMPLETE LOSS |
| **ACE** | Semantic boost via embedding similarity | Falls back to `compute_keyword_ace_boost()` | PARTIAL — keyword fallback works |
| **Dependency** | Package name extraction from text | No change — text-based | NO DEGRADATION |
| **Learned** | Affinity multiplier + feedback boosts | No change — topic-based | NO DEGRADATION |

### Additional Signals Affected

| Signal | Full Embeddings | Zero Vectors |
|--------|----------------|--------------|
| Taste embedding (384-dim centroid) | Active — personalises scoring | Returns 0.0 (norm guard) |
| Semantic ACE boost | Embedding cosine similarity | Keyword topic overlap fallback |
| Domain relevance | May use embedding features | Primarily keyword-based, minimal impact |

## Confirmation Gate Impact

### Gate Table (V2)

| Confirming Axes | Multiplier | Ceiling | With Full Embeddings | With Zero Vectors |
|----------------|------------|---------|---------------------|-------------------|
| 0 | 0.25 | 0.20 | Rare | Common for generic content |
| 1 | 0.45 | 0.28 | Uncommon | Common |
| 2 | 1.00 | 0.65 | Common | **Achievable** (ACE+Dep, ACE+Learned, Dep+Learned) |
| 3 | 1.10 | 0.85 | Common | **Best case** (ACE+Dep+Learned) |
| 4 | 1.20 | 1.00 | Achievable | IMPOSSIBLE (2 axes blind) |
| 5 | 1.25 | 1.00 | Rare | IMPOSSIBLE |

### Maximum Achievable: 3 Signals

With zero vectors, the maximum confirmation count is 3:
- ACE (via keyword topic overlap with detected tech stack)
- Dependency (via package name detection in title/content)
- Learned (via positive feedback history or affinity score)

**2-of-5 gate is still passable.** Items matching dependencies + tech stack + feedback history score 0.65-0.85.

## Precision Impact Estimate

| Content Type | Full Embeddings | Zero Vectors | Delta |
|-------------|----------------|--------------|-------|
| Dependency update for used package | 0.85-0.95 | 0.75-0.85 | -10% |
| Tech stack article (keyword match) | 0.70-0.90 | 0.55-0.75 | -15-20% |
| Semantic-only relevance (no keywords) | 0.50-0.80 | 0.00-0.20 | -60-80% |
| Tangentially related content | 0.30-0.50 | 0.10-0.25 | -20-25% |
| Irrelevant content | 0.05-0.15 | 0.05-0.15 | 0% |

**Overall precision estimate:**
- Full embeddings: ~85-90% (meets INV-001 >85%)
- Zero vectors: ~55-65% (below INV-001 threshold)

**Key insight:** Zero-vector mode is acceptable for first-run/free-tier but should prompt users toward Ollama or API key configuration for sustained use.

## Code Guards (Verified)

All embedding-dependent functions have explicit zero-vector guards:

| Function | File | Guard |
|----------|------|-------|
| `compute_semantic_ace_boost()` | semantic.rs:23-25 | `if item_norm < f32::EPSILON → return None` |
| `compute_interest_score()` | calibration.rs:35-37 | `if item_norm < f32::EPSILON → return 0.0` |
| `compute_taste_boost()` | semantic.rs:258-260 | `if item_norm < f32::EPSILON → return 0.0` |
| `has_real_embedding` | pipeline_v2.rs:803 | `input.embedding.iter().any(\|&v\| v != 0.0)` |

**No NaN risk.** All zero-vector paths return deterministic safe values.

## Simulation: Verified Example

**Input:** Article "Fixing Rust memory issues in Tokio" with zero-vector embeddings

```
ACE confirmed:   TRUE  (keyword "rust" matches active topic)
Learned confirmed: TRUE  (positive feedback on "rust" topics)
Dependency confirmed: TRUE  ("tokio" matches Cargo.toml dependency)
Signal count: 3 → (1.10 mult, 0.85 ceiling)
Result: PASSES gate → relevant
```

**Input:** "10 ways to improve your workflow" (generic) with zero vectors

```
ACE confirmed:   FALSE  (no topic overlap)
Learned confirmed: FALSE  (no feedback history)
Dependency confirmed: FALSE  (no package names)
Signal count: 0 → (0.25 mult, 0.20 ceiling)
Result: FAILS gate → rejected
```

## Mitigation (Implemented)

- **EmbeddingStatusIndicator** component shows amber banner when degraded
- **Embedding batch chunking** prevents memory spikes during recovery
- **Ollama zero-config fallback** automatically tries localhost:11434 before zero vectors
