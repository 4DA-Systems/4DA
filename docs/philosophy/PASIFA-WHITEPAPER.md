# PASIFA: Privacy-Aware Semantic Intelligence Framework for Analysis

## A Multi-Axis Scoring Methodology for Private Developer Intelligence

**Version:** 1.0.0
**Author:** 4DA Systems Pty Ltd
**Date:** March 2026

---

## Abstract

PASIFA is a multi-axis scoring methodology designed to determine whether a piece of content is relevant to a specific software developer, using only signals that exist locally on that developer's machine. It addresses the fundamental failure of single-axis recommendation systems by evaluating content across five independent axes — Context, Interest, ACE, Learned, and Dependency — and requiring confirmation from at least two axes before surfacing any item. This **confirmation gate** is the key innovation: it enforces precision over recall, eliminating the single-axis flukes that plague conventional scoring. Tested across 9 simulated developer personas with 215 labeled items (1,997 total evaluations), PASIFA achieves 92% overall rejection with 98% noise accuracy (true negative rate: 97.8%), 77% precision, and 36% recall — conservative by design, as the system prioritizes showing less but being right. PASIFA operates through an eight-phase pipeline that extracts, calibrates, combines, gates, and thresholds signals in a structured sequence. All computation is local. No data leaves the machine. The system compounds accuracy over time through feedback integration, autophagy calibration, and taste embedding — creating a personal relevance model that cannot be replicated by cloning the code.

---

## 1. Introduction

### 1.1 The Noise Problem

A working software developer is exposed to thousands of content signals daily: Hacker News posts, GitHub trending repositories, RSS articles, Reddit threads, arXiv papers, security advisories, dependency updates. The volume exceeds any human's capacity to evaluate manually.

Existing tools fail because they optimise for the wrong objective. Algorithmic feeds optimise for engagement. Newsletters optimise for subscriber count. RSS readers provide completeness without filtering. AI summary services process content through external models, requiring the user's queries and reading patterns to leave their machine.

The result is a binary failure: developers either consume too much (attention fatigue, context-switching cost) or too little (missed security advisories, overlooked breaking changes in dependencies they use).

### 1.2 Why Single-Axis Scoring Fails

The simplest approach to content relevance is a single similarity score — typically cosine similarity between the content's embedding vector and some representation of the user's interests. This approach is fragile for three reasons:

1. **Semantic ambiguity.** A keyword like "rust" matches both the programming language and corrosion. Embedding similarity helps but does not eliminate this class of false positive.

2. **Contextual blindness.** A high semantic similarity score says nothing about whether the content relates to a library the developer actually depends on, a technology their codebase actually uses, or a topic they have historically found valuable.

3. **Single-point failure.** Any single scoring axis can produce coincidental high scores. A post about "building with React" scores highly on semantic similarity for a React developer, even if it is a beginner tutorial and the developer has ten years of experience — or if the post is actually about chemical reactions.

### 1.3 The Multi-Axis Confirmation Thesis

PASIFA's thesis is that relevance is not a single dimension. It is a convergence of independent signals. Content is relevant when multiple independent axes agree — the same way a GPS receiver requires multiple satellite signals to determine position accurately, and the same way scientific findings require independent replication to be considered robust.

A single strong signal is interesting. Two independent confirmations are convincing. This is the principle behind the confirmation gate.

---

## 2. The Five Axes

Every content item is evaluated across five independent axes. Each axis answers a different question about relevance, uses different signal sources, and is computed by different functions. Independence is structural, not assumed — no axis reads from or depends on the output of another.

### 2.1 Context Axis

**Question:** Does this content relate to what the developer is actively working on?

**Signal source:** KNN embedding similarity against locally indexed code contexts. The developer's IDE files, project structures, and recent git activity are embedded into 384-dimensional vectors and stored in a local SQLite database with sqlite-vec. When content arrives, its embedding is compared against these stored contexts using `find_similar_contexts()`.

**Computation:** The raw similarity score (derived from L2 distance as `1.0 / (1.0 + distance)`) is passed through `calibrate_knn()`, a sigmoid function with parameters `center=0.49, scale=12.0`:

```
calibrated = 1.0 / (1.0 + exp((0.49 - raw) * 12.0))
```

**Confirmation threshold:** `0.45` (from `scoring_config::CONTEXT_THRESHOLD`). Values at or above this threshold indicate meaningful code-context similarity.

**Independence:** The Context axis operates entirely on local code embeddings. It knows nothing about declared interests, dependencies, or user feedback.

### 2.2 Interest Axis

**Question:** Does this content match the developer's declared or inferred interests?

**Signal source:** Two sub-signals contribute to this axis:

1. **Embedding similarity** (`compute_interest_score()`): The content's embedding is compared against each declared interest's embedding using cosine similarity, weighted by the interest's declared weight and a specificity factor. Broad terms (e.g., "AI", "open source", "programming") receive a 0.40x specificity weight via `embedding_specificity_weight()` to prevent them from dominating.

2. **Keyword matching** (`compute_keyword_interest_score()`): Direct textual matching of interest terms in content title and body, weighted by a specificity scale (broad: 0.25, single-word: 0.60, multi-word: 1.00).

**Computation:** The embedding similarity is calibrated through `calibrate_score()` with `center=0.48, scale=12.0`. Either the calibrated embedding score or the keyword score can independently confirm this axis.

**Confirmation threshold:** Embedding interest >= `0.50` OR keyword score >= `0.70`.

**Independence:** The Interest axis operates on declared interests and their embeddings. It does not reference project files, dependencies, or feedback history.

### 2.3 ACE Axis (Autonomous Context Engine)

**Question:** Does this content involve technology that the developer's codebase actually uses?

**Signal source:** The ACE engine scans local projects to detect languages, frameworks, databases, and active development topics. It produces two key data structures: `active_topics` (weighted topics from project manifests and git history, with confidence scores) and `detected_tech` (inferred technology stack).

**Computation:** Semantic ACE boost (`compute_semantic_ace_boost()`) computes weighted cosine similarity between the content embedding and topic embeddings, scaled by topic confidence and learned affinities. When embeddings are unavailable, a keyword fallback (`compute_keyword_ace_boost()`) uses direct topic overlap with configurable boost values (active topic: +0.15, detected tech: +0.12, capped at 0.30).

The ACE axis confirms through any of three paths:
- Semantic boost >= `0.18` (the `SEMANTIC_THRESHOLD`)
- Topic overlap between content topics and ACE active topics (using word-boundary-aware matching via `topic_overlaps()` to prevent false positives like "frustrating" matching "rust")
- Stack pain point match (content addresses known problems with the developer's stack)

**Confirmation threshold:** Any of the three paths above.

**Independence:** The ACE axis derives from codebase analysis. It does not reference declared interests, KNN context matches, or feedback signals.

### 2.4 Learned Axis

**Question:** Has the developer's past behaviour indicated that this kind of content is valuable?

**Signal source:** Two sub-signals:

1. **Feedback boost:** Explicit signals (save, dismiss, mark irrelevant) and implicit signals (time spent, clicks) are aggregated per topic with a 30-day half-life. Topic-level feedback scores are looked up for the content's extracted topics, averaged, scaled by `FEEDBACK_SCALE` (0.30), and capped to [-0.20, +0.20].

2. **Affinity multiplier** (`compute_affinity_multiplier()`): Computed from learned topic affinities stored in the ACE context. Each affinity is a `(score, confidence)` pair, where score ranges from -1.0 (strong dislike) to +1.0 (strong preference). The multiplier formula is:

```
multiplier = (1.0 + avg_effect * 0.7).clamp(0.3, 1.7)
```

where `avg_effect` is the confidence-weighted average of matching topic affinities.

**Confirmation threshold:** Feedback boost > `0.05` OR affinity multiplier >= `1.15`.

**Independence:** The Learned axis derives entirely from historical user behaviour. It does not reference the current codebase state, declared interests, or dependency manifests.

### 2.5 Dependency Axis

**Question:** Is this content about a library the developer actually depends on?

**Signal source:** Package manifests (`Cargo.toml`, `package.json`, `requirements.txt`, etc.) are parsed to extract dependency names, versions, and metadata. The function `match_dependencies()` checks whether any extracted dependency names appear in the content's title, body, or extracted topics.

**Computation:** Dependency matching produces a match score and a list of matched packages with version delta information (same version, newer patch, newer minor, newer major). The match score represents the strength and count of dependency matches found.

**Confirmation threshold:** Dependency match score >= `0.20`.

**Independence:** The Dependency axis operates on package manifest data. It does not reference embeddings, user interests, or feedback history.

---

## 3. The Confirmation Gate

### 3.1 Purpose

The confirmation gate is the mechanism that enforces PASIFA's precision-over-recall design. It operates after signals are counted but before the final score is computed, applying both a multiplier and a ceiling to the score based on how many independent axes confirm relevance.

### 3.2 The Gate Table

The gate table defines the relationship between confirmation count and score treatment:

| Confirming Axes | Multiplier | Score Ceiling | Effect |
|:-:|:-:|:-:|:--|
| 0 | 0.25 | 0.20 | Heavy penalty. Score quartered and capped at 0.20. |
| 1 | 0.45 | 0.28 | Strong penalty. Ceiling of 0.28 is below the 0.35 relevance threshold. |
| 2 | 1.00 | 0.65 | Neutral multiplier. Content can reach moderate scores. |
| 3 | 1.10 | 0.85 | Mild boost. Content can reach high scores. |
| 4 | 1.20 | 1.00 | Strong confirmation. No ceiling constraint. |
| 5 | 1.25 | 1.00 | Full confidence. Maximum boost, no ceiling. |

These values are defined as `V2_GATE` in `pipeline_v2.rs`.

### 3.3 Why 2-of-5 Is the Minimum

The critical property of the gate table is that **no content with fewer than two confirming axes can reach the relevance threshold.** With one signal, the ceiling is 0.28 — below the 0.35 default relevance threshold. With zero signals, the ceiling is 0.20.

This means that regardless of how strong any single signal is — even a perfect 1.0 on one axis — it cannot push content past the threshold without at least one other axis independently agreeing.

This design choice trades recall for precision. Some genuinely relevant content will be missed because only one axis detects it. This is acceptable because:

1. **False positive cost is asymmetric.** An irrelevant item shown to the user wastes attention and erodes trust. A missed relevant item can be discovered later through other channels. The cost of a false positive exceeds the cost of a false negative.

2. **Independent confirmation is statistically robust.** If two unrelated signal sources agree that content is relevant, the probability of coincidental agreement is the product of the individual false-positive rates. With five axes, 2-of-5 agreement substantially reduces false positives while maintaining sensitivity to genuinely multi-dimensional relevance.

3. **Bootstrap mode provides a safe exception.** For new users with fewer than 10 feedback interactions, the minimum signal requirement relaxes to 1, preventing the cold-start problem from making the system appear broken. As the user provides feedback, the full 2-of-5 requirement engages.

### 3.4 The Ceiling Mechanism

The gate applies two operations in sequence:

```
gated_score = base_score * multiplier
final_score = min(gated_score, ceiling)
```

The ceiling is the more important constraint. A multiplier of 0.45 on a base score of 0.90 yields 0.405 — but the ceiling of 0.28 caps it further. This double constraint ensures that neither high base scores nor favourable multipliers can circumvent the gate.

At 2 confirming axes, the ceiling of 0.65 allows content to be clearly relevant but prevents it from reaching the highest scores without broader confirmation. At 3 axes, the ceiling rises to 0.85. Only at 4+ axes does the ceiling fully open to 1.00.

### 3.5 Domain Gate Integration

After the confirmation gate, a domain gate multiplier is applied based on domain relevance (computed from the user's graduated technology identity). Primary domain content receives a 1.10x boost. Off-domain content is penalised down to 0.40x. The score ceiling is applied **after** the domain gate, ensuring that domain boosting cannot push a low-confirmation score above the gate ceiling.

---

## 4. The V2 Pipeline

The V2 pipeline structures PASIFA scoring into eight sequential phases. Each phase has a defined input, output, and invariant. The pipeline is implemented in `pipeline_v2.rs` and dispatched by the `score_item()` function in `scoring/mod.rs`.

### Phase 1: Signal Extraction

**Function:** `extract_signals()`

All raw signal values are extracted independently from the content item and the scoring context. This phase produces a `RawSignals` struct containing:

- `context`: Best KNN similarity score from embedding search
- `interest`: Embedding similarity against declared interests
- `keyword_score`: Textual keyword matching against interests, weighted by specificity
- `semantic_boost`: Semantic ACE boost (embedding-based or keyword fallback)
- `dep_match_score`: Dependency matching score and matched package list
- `feedback_boost`: Net feedback learning signal for matched topics
- `affinity_mult`: Multiplicative factor from learned topic affinities (0.3 to 1.7)
- `anti_penalty`: Penalty from anti-topics (0.0 to 0.7)
- `domain_relevance`: Graduated domain relevance from technology identity
- `taste_boost`: Boost from taste embedding similarity (clamped to +/-0.08)
- `stack_boost`: Boost from curated stack profile matching
- `stack_pain_match`: Boolean indicating pain-point content for user's stack
- `topics`: Extracted topic strings from content title and body

**Invariant:** No signal depends on any other signal's value. All extraction is independent.

### Phase 2: KNN Calibration

**Function:** `calibrate_signals()`

Distance-based scores are compressed into a narrow band by the nature of embedding similarity. A raw cosine similarity of 0.45 and 0.55 may represent the difference between irrelevant and highly relevant, but the numerical range is too small for downstream combination to distinguish.

The sigmoid calibration function stretches this narrow band into a usable distribution:

```
calibrated = 1.0 / (1.0 + exp((center - raw) * scale))
```

Two calibration functions are applied:

- **`calibrate_knn()`** for context scores: `center=0.49, scale=12.0`. Tuned for L2-distance-derived similarity scores from sqlite-vec KNN queries.
- **`calibrate_score()`** for interest scores: `center=0.48, scale=12.0`. Tuned for direct cosine similarity against interest embeddings.

Keyword and semantic boost scores are passed through uncalibrated, as they are already in a usable range.

**Invariant:** Calibration is monotonically increasing. Higher raw scores always produce higher calibrated scores.

### Phase 3: Gate Count

**Function:** `count_confirmed_signals()` (in `gate.rs`)

Signal confirmation is counted **before** any combination of signals. This is a deliberate design choice. If gate counting occurred after signals were combined (e.g., after semantic integration or boost application), the gate would operate on artefacts of the combination formula rather than on clean, independent signal assessments.

The function evaluates each of the five axes against its threshold and produces a `SignalConfirmation` struct with per-axis booleans and a total count.

**Invariant:** Gate count operates only on calibrated individual signals and threshold comparisons. It does not read or depend on any combined score.

### Phase 4: Semantic Integration

**Function:** `compute_relevance()`

The base relevance score is computed by combining calibrated signals. The combination strategy depends on what context is available:

1. **Both context and interest available:** Dynamic weighting where context weight scales with context score strength (`0.15 + context_score * 0.40`, capped at 0.55). Remaining weight is split between interest (55%) and keyword (45%). Semantic boost is applied multiplicatively: `base * (1.0 + semantic_boost)`.

2. **Interest only:** Interest and keyword are weighted (0.45 and 0.35 respectively). Semantic boost is multiplicative with a scaling factor of 1.2x, reduced to 0.48x for new users with sparse signals to prevent over-amplification during bootstrap.

3. **Context only:** Context score with multiplicative semantic: `context * (1.0 + semantic_boost)`.

4. **Neither available:** Fallback to `semantic_boost * 2.0`, clamped to [0.0, 1.0].

**Key design decision:** Semantic integration is multiplicative, not additive. In the V1 pipeline, semantic boost was added to the base score (`base + semantic_boost * weight`), which allowed a strong semantic signal to overwhelm a weak base. Multiplicative integration (`base * (1.0 + semantic_boost)`) preserves proportionality: a strong base is amplified, but a zero base remains zero regardless of semantic strength.

### Phase 5: Quality Composite

**Function:** `compute_quality_composite()`

All quality multipliers are combined in a single pass using asymmetric dampening. Each multiplier represents a quality dimension:

- **Freshness:** Topic-aware temporal decay with peak-hours bonus
- **Source quality:** Learned source-type preferences
- **Anti-topic penalty:** Content matching rejected topics
- **Domain quality:** Graduated technology relevance (NOT dampened — preserves full penalty for off-domain content)
- **Competing tech penalty:** Content about competing technologies
- **Content quality:** Structural content analysis (title length, substance indicators)
- **Content DNA:** Content type classification (tutorial, news, discussion, show-and-tell)
- **Novelty:** Inverse familiarity signal
- **Ecosystem shift:** Stack migration signals
- **Stack competing:** Stack-profile-aware competing tech
- **Affinity multiplier:** Learned topic preferences

The dampening function is asymmetric:

```
if multiplier < 1.0:
    dampened = 1.0 + (multiplier - 1.0) * 0.65    // penalties at 65% strength
if multiplier >= 1.0:
    dampened = 1.0 + (multiplier - 1.0) * 0.55    // boosts at 55% strength
```

This asymmetry reflects the trust-is-asymmetric principle: penalties are applied more strongly than boosts because false positives are more costly than false negatives.

**Invariant:** All multipliers are dampened before application, preventing any single quality signal from dominating the composite. Domain quality is the sole exception — it is applied at full strength because domain mismatch is a hard relevance signal, not a soft quality preference.

### Phase 6: Boost Application

**Function:** `compute_boosts()`

All additive boosts are summed, capped, dampened, and added in a single pass:

| Boost | Source |
|:--|:--|
| Dependency boost | `dep_match_score * 0.15` (doubled in bootstrap mode) |
| Stack boost | Stack profile matching |
| Intent boost | Recent work topic matching (+0.12 single, +0.25 multi) |
| Feedback boost | Topic-level feedback learning |
| Decision window boost | Open decision window matching |
| Skill gap boost | Content matching identified skill gaps (+0.15 single, +0.20 multi) |
| Calibration correction | Autophagy-derived scoring correction (clamped to +/-0.10) |
| Taste boost | Taste embedding similarity (clamped to +/-0.08) |

The total is capped to `[-0.15, +0.35]`, then dampened:

```
total_capped = sum_of_boosts.clamp(-0.15, 0.35)
total_dampened = total_capped * dampening_factor
boosted_score = quality_score + total_dampened
```

**Why cap boosts?** Without a cap, a content item matching multiple boost sources (dependency + intent + feedback + taste) could accumulate +0.55 or more, overwhelming the base signal. The cap at +0.35 ensures that boosts can meaningfully improve a score but cannot transform an irrelevant item into a relevant one. The negative cap at -0.15 prevents boost penalties from being more destructive than the confirmation gate itself.

### Phase 7: Confirmation Gate

**Function:** `apply_gate_effect()`

The gate table from Phase 3's signal count is applied to the boosted score. The multiplier is applied first, then the domain gate multiplier, then the score ceiling:

```
gated = score * confirmation_multiplier
gated = gated * domain_gate_multiplier
final = min(gated, score_ceiling)
```

Applying the ceiling last is critical. It ensures that neither the confirmation multiplier nor the domain gate can push a low-confirmation score above the ceiling. This is the structural enforcement of the 2-of-5 requirement.

### Phase 8: Final Threshold

**Function:** `apply_final_adjustments()` and threshold comparison

Two final adjustments:

1. **Short title cap:** Content with fewer than 3 meaningful words in the title is capped at 0.40. Short titles provide insufficient signal for confident scoring.

2. **Relevance threshold:** The gated score is compared against the auto-tuning relevance threshold (default: 0.35). Items at or above the threshold are marked relevant. Items below are rejected.

An additional quality floor applies: items must have at least `min_signals` confirming axes (2 in normal mode, 1 in bootstrap mode) OR a score above 0.70 to be marked relevant. This prevents high-scoring items with zero signal confirmation from slipping through.

---

## 5. Calibration and Learning

PASIFA is not a static scoring system. Four mechanisms enable it to compound accuracy for each individual user over time.

### 5.1 Feedback Integration

Explicit user signals (save, dismiss, mark irrelevant) and implicit signals (time on content, link clicks) are recorded per topic. These signals are aggregated with a **30-day half-life**, ensuring that recent preferences dominate while old preferences decay naturally.

Feedback produces two outputs:
- **Topic feedback boosts:** Per-topic net scores in [-1.0, +1.0], scaled by `FEEDBACK_SCALE` (0.30) and capped to [-0.20, +0.20]. These enter the pipeline in Phase 6.
- **Topic affinities:** Stored as `(affinity_score, confidence)` pairs in the ACE context, where confidence increases with exposure count. These contribute to the affinity multiplier in Phase 5 and to gate confirmation via the Learned axis.

The half-life prevents stale preferences from permanently biasing the system. A developer who was interested in Kubernetes six months ago but has since shifted to edge computing will see Kubernetes content gradually deprioritised without any explicit action.

### 5.2 Autophagy Calibration

The system periodically re-evaluates its own scoring accuracy by comparing predicted relevance (the PASIFA score at the time of analysis) against observed user behaviour (did the user save, click, or dismiss the item?). This produces **calibration deltas** — per-topic corrections that are injected into the scoring pipeline in Phase 6.

Calibration deltas are clamped to [-0.10, +0.10] per topic. If the system consistently over-scores "DevOps" content (high scores, low engagement), the calibration delta for "DevOps" becomes negative, correcting the systematic bias. Conversely, if "WebAssembly" content is consistently under-scored relative to engagement, its delta becomes positive.

Autophagy calibration operates at the topic level, not the item level. This prevents overfitting to individual content items while still correcting for systematic biases in how the scoring pipeline treats specific domains.

### 5.3 Taste Embedding

The taste embedding is a **384-dimensional unit vector** representing the user's holistic preference profile. It is computed by `compute_taste_embedding()` as a weighted centroid of topic affinity embeddings:

```
centroid[i] = sum(affinity_score * confidence * topic_embedding[i])
              for all (topic, affinity_score, confidence) in affinities

taste = centroid / ||centroid||
```

Positive affinities pull the taste vector toward liked content domains. Negative affinities push it away from disliked domains. The result is a single vector that captures the user's preference landscape in embedding space.

The taste boost (`compute_taste_boost()`) computes cosine similarity between the content embedding and the taste embedding, centred at 0.4 (typical background similarity) and scaled to produce a small boost in the range [-0.08, +0.08]:

```
taste_boost = ((cosine_sim - 0.4) * 0.2).clamp(-0.08, 0.08)
```

The taste boost is intentionally small. It personalises scoring at the margin — nudging borderline items — without dominating the multi-axis decision. A content item must still pass the confirmation gate regardless of taste alignment.

### 5.4 Domain Profile

The domain profile represents the developer's **graduated technology identity** — a structured model of which technologies they use, how deeply, and how recently. Unlike a flat list of detected technologies, the domain profile distinguishes between primary stack (daily use, deep expertise), secondary technologies (regular use, moderate expertise), and peripheral technologies (occasional use, awareness).

Domain relevance scoring (`compute_domain_relevance()`) uses this graduated profile to produce a continuous relevance value rather than a binary match/no-match. Content about a primary technology receives full domain relevance (1.0). Content about a secondary technology receives partial relevance. Content about an unrelated technology receives low relevance, triggering the domain quality penalty in Phase 5 and the domain gate reduction in Phase 7.

---

## 6. The ScoringContext

Every scoring decision is made in the context of a `ScoringContext` struct, computed once per analysis run and shared across all items scored in that run. The context encapsulates all per-user state that the pipeline needs. Its fields (21+) include:

| Field | Type | Purpose |
|:--|:--|:--|
| `cached_context_count` | `i64` | Number of indexed code contexts (determines Context axis availability) |
| `interest_count` | `usize` | Number of declared interests (determines Interest axis availability) |
| `interests` | `Vec<Interest>` | Full interest objects with embeddings and weights |
| `exclusions` | `Vec<String>` | User-defined exclusion terms (pre-gate hard filter) |
| `ace_ctx` | `ACEContext` | Full ACE context: topics, tech, anti-topics, affinities, dependencies |
| `topic_embeddings` | `HashMap<String, Vec<f32>>` | Pre-computed 384-dim embeddings for all ACE topics |
| `feedback_boosts` | `HashMap<String, f64>` | Per-topic feedback scores from user behaviour |
| `source_quality` | `HashMap<String, f32>` | Learned source-type quality preferences |
| `declared_tech` | `Vec<String>` | Explicitly declared tech stack (3-5 items from onboarding) |
| `domain_profile` | `DomainProfile` | Graduated technology identity |
| `work_topics` | `Vec<String>` | Recent work topics from git activity (last 2 hours) |
| `feedback_interaction_count` | `i64` | Total feedback interactions (bootstrap mode detection) |
| `composed_stack` | `ComposedStack` | Curated stack profile for stack-aware scoring |
| `open_windows` | `Vec<DecisionWindow>` | Active decision windows for contextual boost |
| `calibration_deltas` | `HashMap<String, f32>` | Autophagy correction deltas per topic |
| `taste_embedding` | `Option<Vec<f32>>` | 384-dim holistic preference vector |
| `topic_half_lives` | `HashMap<String, f32>` | Topic-aware decay rates (hours) |
| `sovereign_profile` | `Option<SovereignDeveloperProfile>` | Unified developer profile with skill gaps |
| `dominant_persona` | `Option<(usize, f32)>` | Dominant taste persona from continuous inference |

The `ScoringContext` is constructed by `build_scoring_context()` and is immutable during scoring. This ensures that scoring is deterministic for a given context — the same input and context always produce the same score.

---

## 7. Design Decisions

### 7.1 Why Five Axes, Not Three or Seven

Five axes were chosen through empirical evaluation of the signal landscape available to a local-first developer tool:

- **Three axes** (context, interest, learned) provide insufficient triangulation. Two false positives on three axes yield 67% confirmation. Two false positives on five axes yield 40% confirmation — below the 2-of-5 gate.

- **Seven axes** would require additional independent signal sources that do not exist in a privacy-first local system. Candidates like "social proof" (how many others saved this) or "authority" (author reputation) require external data. Adding axes that are not truly independent weakens the gate rather than strengthening it.

Five axes represent the natural decomposition of locally available relevance signals: what you are working on (Context), what you care about (Interest), what your code uses (ACE), what you have valued before (Learned), and what you depend on (Dependency).

### 7.2 Why Multiplicative Semantic Integration

The V1 pipeline used additive semantic integration: `base + semantic_boost * weight`. This produced a class of false positive where a content item with weak base relevance (e.g., context score 0.15) but strong semantic match (e.g., semantic boost 0.40) scored above threshold.

Multiplicative integration (`base * (1.0 + semantic_boost)`) fixes this by design. If the base is near zero, no amount of semantic boost can produce a high score. The semantic signal amplifies genuine relevance rather than creating it from nothing.

### 7.3 Why Sigmoid KNN Calibration

Raw embedding similarity scores from KNN search are compressed into a narrow band, typically [0.35, 0.60] for text-embedding-3-small L2 distances. Without calibration, the scoring pipeline cannot distinguish between a mediocre match (0.42) and a strong match (0.55) — both appear as "about 0.5" to downstream combination.

The sigmoid function `1.0 / (1.0 + exp((center - raw) * scale))` was chosen because:
1. It is monotonically increasing (preserves ordering)
2. It maps the empirical midpoint (0.48-0.49) to approximately 0.5
3. It stretches the active band to [0.15, 0.85], providing usable dynamic range
4. It naturally saturates at 0 and 1, preventing extreme values
5. Parameters (`center`, `scale`) are tunable per embedding model

### 7.4 Why Separate Gate Count Before Combination

Counting confirming signals after combination would introduce a subtle dependency: the gate would operate on a number that is itself a function of the signals being gated. This circular dependency makes the gate's behaviour unpredictable and difficult to reason about.

By counting signals in Phase 3 (before combination in Phase 4), the gate operates on clean, independent threshold comparisons. The count is a pure function of the individual calibrated signals and their thresholds. This makes the gate's behaviour deterministic, testable, and explainable.

### 7.5 Why Boost Caps

Without caps, boost accumulation creates a class of scoring distortion where an item matching many boost sources (dependency + intent + feedback + taste + skill gap + decision window) accumulates enough additive boost to overwhelm the base signal and the gate. The cap at [-0.15, +0.35] was chosen empirically:

- **+0.35 maximum:** A quality score of 0.30 (borderline) plus maximum boost reaches 0.65 — above threshold but not dominant. A quality score of 0.00 (irrelevant) plus maximum boost reaches only 0.35 — still borderline, not confidently relevant.
- **-0.15 minimum:** Prevents boost penalties from being more destructive than the gate itself. The gate's 0-signal multiplier (0.25x) already handles aggressive suppression.

---

## 8. Privacy Guarantees

PASIFA achieves its scoring objectives without any data leaving the developer's machine. This is not a feature — it is an architectural constraint.

### 8.1 Local Signal Sources

Every signal source in the PASIFA pipeline operates on local data:

| Signal | Data Source | Location |
|:--|:--|:--|
| Context embeddings | IDE files, project structure, git history | Local filesystem |
| Interest embeddings | User-declared interests | Local SQLite database |
| ACE topics/tech | Project manifest scanning | Local filesystem |
| Dependencies | `Cargo.toml`, `package.json`, etc. | Local filesystem |
| Feedback signals | User interactions within the app | Local SQLite database |
| Topic affinities | Aggregated from local feedback | Local SQLite database |
| Taste embedding | Computed from local affinities | In-memory (per-run) |
| Calibration deltas | Computed from local predictions vs. behaviour | Local SQLite database |

### 8.2 Embedding Generation

Embeddings are generated locally via Ollama (a local LLM runtime) or through the user's own API key (BYOK — Bring Your Own Key). When using an external API, only the content text is sent (which is public content that the user already fetched). The user's context embeddings, interests, feedback, and scoring results are never transmitted.

### 8.3 Zero Telemetry

PASIFA includes no analytics, no tracking, no phone-home, and no error reporting to any external service. This is enforced by:

- Content Security Policy restricting network requests to a whitelist
- Pre-commit secrets scanning (23+ pattern detectors)
- Invariant INV-004: "NO data leaves the machine without explicit user consent"
- Architectural principle W-2: "Data that can leak will leak. Enforce by structure, not policy."

---

## 9. Reference Implementation

The reference implementation of PASIFA is in the 4DA application, a source-available (FSL-1.1-Apache-2.0) Tauri 2.0 desktop application.

### Source Files

| Module | Path | Purpose |
|:--|:--|:--|
| Pipeline dispatch | `src-tauri/src/scoring/mod.rs` | Runtime V1/V2 dispatch, `ScoringContext` struct |
| V2 pipeline | `src-tauri/src/scoring/pipeline_v2.rs` | Eight-phase pipeline implementation |
| Confirmation gate | `src-tauri/src/scoring/gate.rs` | `count_confirmed_signals()`, `apply_confirmation_gate()` |
| Calibration | `src-tauri/src/scoring/calibration.rs` | `calibrate_score()`, `compute_interest_score()` |
| Semantic scoring | `src-tauri/src/scoring/semantic.rs` | `compute_semantic_ace_boost()`, `compute_taste_embedding()` |
| Affinity | `src-tauri/src/scoring/affinity.rs` | `compute_affinity_multiplier()`, `compute_anti_penalty()` |
| ACE context | `src-tauri/src/scoring/ace_context.rs` | `ACEContext` struct, `get_ace_context()` |
| Dependencies | `src-tauri/src/scoring/dependencies.rs` | `match_dependencies()`, `VersionDelta` |
| Scoring config | `src-tauri/scoring/pipeline.scoring` | DSL-defined configuration constants |
| Context builder | `src-tauri/src/scoring/context.rs` | `build_scoring_context()` |
| Keywords | `src-tauri/src/scoring/keywords.rs` | Keyword interest scoring and specificity |
| Explanation | `src-tauri/src/scoring/explanation.rs` | Human-readable relevance explanations |

---

## Appendix A: Mathematical Notation

### A.1 Signal Extraction

Let $x$ be a content item with embedding $\mathbf{e}_x \in \mathbb{R}^{384}$, title $t_x$, body $b_x$, and extracted topics $T_x = \{t_1, t_2, \ldots, t_n\}$.

Let the scoring context provide:
- Code contexts $\mathcal{C}$ with embeddings
- Interests $\mathcal{I} = \{(topic_i, \mathbf{e}_i, w_i)\}$
- ACE context $\mathcal{A} = (A_{topics}, A_{tech}, A_{affinities}, A_{deps})$
- Taste embedding $\mathbf{e}_{taste} \in \mathbb{R}^{384}$

### A.2 Calibration

$$\sigma(r; c, s) = \frac{1}{1 + e^{(c - r) \cdot s}}$$

For KNN context: $c = 0.49, s = 12.0$

For interest similarity: $c = 0.48, s = 12.0$

### A.3 Signal Confirmation

For each axis $a \in \{\text{context}, \text{interest}, \text{ace}, \text{learned}, \text{dependency}\}$, define:

$$\mathbb{1}_a = \begin{cases} 1 & \text{if signal}_a \geq \theta_a \\ 0 & \text{otherwise} \end{cases}$$

where $\theta_a$ is the axis-specific threshold.

Confirmation count: $k = \sum_{a} \mathbb{1}_a$

### A.4 Gate Application

$$\text{gated}(s, k) = \min\left(s \cdot m_k \cdot g_d, \; c_k\right)$$

where $m_k$ is the confirmation multiplier, $c_k$ is the score ceiling for count $k$, and $g_d$ is the domain gate multiplier.

### A.5 Boost Aggregation

$$B = \text{clamp}\left(\sum_{i} b_i, -0.15, 0.35\right) \cdot d$$

where $b_i$ are individual boosts and $d$ is the dampening factor (0.55 for positive, 0.65 for negative).

### A.6 Complete Pipeline

$$\text{score}(x) = F\left(G\left(\left(R(\sigma(\mathbf{e}_x)) \cdot Q\right) + B, \; k\right)\right)$$

where $R$ is the relevance computation (Phase 4), $Q$ is the quality composite (Phase 5), $B$ is the dampened boost total (Phase 6), $G$ is the gate function (Phase 7), and $F$ applies final adjustments (Phase 8).

---

## Appendix B: Glossary

| Term | Definition |
|:--|:--|
| **ACE** | Autonomous Context Engine. Zero-configuration local project scanner that detects languages, frameworks, dependencies, and development activity. |
| **Affinity** | A learned preference score for a topic, derived from user feedback. Ranges from -1.0 (strong dislike) to +1.0 (strong preference), paired with a confidence value. |
| **Anti-topic** | A topic the user has consistently rejected. Content matching anti-topics receives a multiplicative penalty. |
| **Autophagy** | Self-correction mechanism where the system compares its past predictions against observed behaviour and generates calibration deltas. |
| **Bootstrap mode** | Relaxed scoring for new users with fewer than 10 feedback interactions. Reduces minimum signal requirement from 2 to 1. |
| **Calibration delta** | Per-topic scoring correction derived from autophagy analysis. Clamped to [-0.10, +0.10]. |
| **Confirmation gate** | The mechanism requiring 2+ independent axis confirmations before content can reach the relevance threshold. |
| **Dampening** | Asymmetric scaling applied to quality multipliers. Penalties are applied at 65% strength; boosts at 55% strength. |
| **Domain profile** | Graduated technology identity distinguishing primary, secondary, and peripheral technologies. |
| **Gate ceiling** | Maximum score achievable at a given confirmation count. Structural enforcement of the multi-axis requirement. |
| **KNN** | K-Nearest Neighbours. Used for embedding similarity search via sqlite-vec. |
| **PASIFA** | Privacy-Aware Semantic Intelligence Framework for Analysis. The multi-axis scoring methodology described in this document. |
| **Relevance threshold** | The minimum score required for content to be surfaced. Default: 0.35. Auto-tuning adjusts based on content volume. |
| **ScoringContext** | Immutable per-run context containing all user state needed for scoring decisions. Computed once, shared across all items. |
| **Sigmoid stretch** | Calibration function that maps compressed similarity scores to a wider usable range. |
| **Taste embedding** | A 384-dimensional unit vector representing the user's holistic preference profile, computed as a weighted centroid of topic affinity embeddings. |
| **Topic overlap** | Word-boundary-aware string matching that prevents false positives (e.g., "frustrating" does not match "rust"). |

---

*PASIFA is the scoring methodology of 4DA, a privacy-first developer intelligence system.*
*Reference implementation: github.com/4da-systems/4da | Philosophy: 4da.ai/philosophy*
*Published by 4DA Systems Pty Ltd (ACN 696 078 841). March 2026.*
