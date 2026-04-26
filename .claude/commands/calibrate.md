---
description: "Scoring pipeline calibration intelligence — simulation quality, persona accuracy, golden baselines, blind spots, separation analysis, and calibration recommendations"
allowed-tools: ["Read", "Glob", "Grep", "Task", "Bash(cargo test:*)", "Bash(cargo check:*)", "Bash(find:*)", "Bash(wc:*)", "Bash(echo:*)", "Bash(ls:*)", "Bash(date:*)", "Bash(head:*)", "Bash(tail:*)", "Bash(cat:*)", "Bash(sort:*)"]
argument-hint: "[--quick | --deep | --personas | --golden | --corpus | --separation | --blind-spots | --drift]"
---

<objective>
Generate a live calibration report for the 4DA scoring pipeline by running the simulation infrastructure against the current pipeline state. Every metric comes from actual test execution — nothing cached. The report covers per-persona scoring quality, golden snapshot stability, content category blind spots, separation gap analysis, and produces ranked calibration recommendations.

This command never modifies scoring logic or thresholds. It writes only to `.claude/calibration-snapshot.json` for trend tracking across runs.

Scope filter: $ARGUMENTS
- `--quick` — headline metrics only: aggregate F1, worst persona, golden pass/fail (30 seconds)
- `--deep` — full report + parallel subagent deep dives into each simulation system
- `--personas` — per-persona quality breakdown only
- `--golden` — golden snapshot stability check only
- `--corpus` — corpus coverage and category distribution analysis
- `--separation` — separation gap analysis (relevant vs irrelevant score distribution)
- `--blind-spots` — content category failure analysis (what the pipeline misses)
- `--drift` — compare against last calibration snapshot for regression detection
- Default (no flag): full calibration report with recommendations
</objective>

<context>
Pre-load at invocation time:
- Current commit: `! \`git rev-parse --short HEAD\``
- Current branch: `! \`git branch --show-current\``
- Last calibration snapshot: `! \`cat .claude/calibration-snapshot.json 2>/dev/null || echo "no previous snapshot"\``
- Simulation module count: `! \`ls src-tauri/src/scoring/simulation/*.rs 2>/dev/null | wc -l\``
</context>

<reference-architecture>
Before running any phase, read these files to understand the scoring and simulation architecture:

1. `src-tauri/src/scoring/simulation/mod.rs` — simulation hub, shared types (ContentCategory, ExpectedOutcome, LabeledItem, PERSONA_NAMES)
2. `src-tauri/src/scoring/simulation/personas.rs` — 9 developer archetypes (rust_systems, python_ml, fullstack_ts, devops_sre, mobile_dev, bootstrap, power_user, context_switcher, niche_specialist)
3. `src-tauri/src/scoring/simulation/corpus.rs` — 215 labeled test fixtures with per-persona expected outcomes
4. `src-tauri/src/scoring/simulation/metrics.rs` — SimMetrics (TP/FP/TN/FN, precision/recall/F1, separation_gap)
5. `src-tauri/src/scoring/simulation/reality.rs` — System 2 aggregate quality testing
6. `src-tauri/src/scoring/simulation/golden_snapshot.rs` — System 5 canonical item baselines
7. `src-tauri/src/scoring/pipeline.rs` — the actual 22-stage PASIFA scoring pipeline
</reference-architecture>

<process>

## Phase 1: Pipeline Vital Signs (always runs)

Gather these metrics in parallel. Each must produce a concrete number.

### 1A. Simulation Test Suite
```bash
cd src-tauri && cargo test --lib simulation:: 2>&1 | tail -5
```
Extract: total simulation tests, passed, failed, ignored. This is the foundation — if tests fail, the pipeline has regressed.

### 1B. Full Scoring Test Suite
```bash
cd src-tauri && cargo test --lib scoring:: 2>&1 | tail -5
```
Extract: total scoring tests (includes pipeline, benchmark, simulation), pass/fail. Cross-reference with simulation-only count.

### 1C. Simulation Module Inventory
```bash
cd src-tauri && for f in src/scoring/simulation/*.rs; do name=$(basename "$f" .rs); tests=$(grep -c '#\[test\]' "$f" 2>/dev/null || echo 0); lines=$(wc -l < "$f"); echo "$name: $tests tests, $lines lines"; done
```
Extract: per-module test count, total simulation tests, total simulation lines.

### 1D. Corpus Size and Category Distribution
Read `src-tauri/src/scoring/simulation/corpus.rs` and count:
- Total labeled items
- Items per ContentCategory (DirectMatch, AdjacentMatch, CrossDomainNoise, Borderline, CareerNoise, SecurityAdvisory, etc.)
- Items per ExpectedOutcome across all personas
- Any categories with fewer than 5 items (coverage gaps)

Present as:
```
╔══════════════════════════════════════════════════════════════╗
║              SCORING PIPELINE CALIBRATION                   ║
║              Commit: {hash} | {date}                        ║
╠═════════════════════╦════════╦════════╦════════╦════════════╣
║ Metric              ║ Value  ║ Target ║ Status ║ Trend      ║
╠═════════════════════╬════════╬════════╬════════╬════════════╣
║ Simulation Tests    ║   {n}  ║  45+   ║  ✓/✗   ║  ↑/↓/→    ║
║ All Scoring Tests   ║   {n}  ║ 100+   ║  ✓/✗   ║  ↑/↓/→    ║
║ Corpus Items        ║   {n}  ║ 200+   ║  ✓/✗   ║  ↑/↓/→    ║
║ Content Categories  ║   {n}  ║  10+   ║  ✓/✗   ║  ↑/↓/→    ║
║ Persona Coverage    ║  {n}/9 ║   9    ║  ✓/✗   ║  ↑/↓/→    ║
║ Golden Snapshots    ║  {n}   ║  20+   ║  ✓/✗   ║  ↑/↓/→    ║
╚═════════════════════╩════════╩════════╩════════╩════════════╝
```

If `--quick` flag, stop here with the vital signs table.

---

## Phase 2: Per-Persona Quality Breakdown

Read `src-tauri/src/scoring/simulation/reality.rs` to understand how quality is measured.

### 2A. Extract Per-Persona Thresholds
Read reality.rs test assertions to extract current F1/precision/recall thresholds per persona. These are the acceptance criteria.

### 2B. Run Reality Tests
```bash
cd src-tauri && cargo test --lib simulation::reality -- --nocapture 2>&1 | head -100
```
Capture verbose output showing per-persona metrics if available.

### 2C. Analyze Persona Scoring Characteristics
For each of the 9 personas, read personas.rs and determine:
- Number of interests configured
- Number of declared technologies
- Domain profile breadth (domains + keywords)
- ACE context richness (detected tech, languages, frameworks)
- Expected scoring behavior: specialist (narrow + high scores) vs generalist (broad + moderate scores)

Present as:
```
╔═══════════════════════════════════════════════════════════════════════╗
║                    PER-PERSONA QUALITY MATRIX                        ║
╠════════════════╦═══════╦═══════════╦════════╦══════╦════════════════╣
║ Persona        ║  F1   ║ Precision ║ Recall ║ Sep  ║ Assessment     ║
╠════════════════╬═══════╬═══════════╬════════╬══════╬════════════════╣
║ rust_systems   ║ 0.xxx ║   0.xxx   ║ 0.xxx  ║ 0.xx ║ {grade}       ║
║ python_ml      ║ 0.xxx ║   0.xxx   ║ 0.xxx  ║ 0.xx ║ {grade}       ║
║ fullstack_ts   ║ 0.xxx ║   0.xxx   ║ 0.xxx  ║ 0.xx ║ {grade}       ║
║ devops_sre     ║ 0.xxx ║   0.xxx   ║ 0.xxx  ║ 0.xx ║ {grade}       ║
║ mobile_dev     ║ 0.xxx ║   0.xxx   ║ 0.xxx  ║ 0.xx ║ {grade}       ║
║ bootstrap      ║ 0.xxx ║   0.xxx   ║ 0.xxx  ║ 0.xx ║ {grade}       ║
║ power_user     ║ 0.xxx ║   0.xxx   ║ 0.xxx  ║ 0.xx ║ {grade}       ║
║ context_switch ║ 0.xxx ║   0.xxx   ║ 0.xxx  ║ 0.xx ║ {grade}       ║
║ niche_special  ║ 0.xxx ║   0.xxx   ║ 0.xxx  ║ 0.xx ║ {grade}       ║
╠════════════════╬═══════╬═══════════╬════════╬══════╬════════════════╣
║ AGGREGATE      ║ 0.xxx ║   0.xxx   ║ 0.xxx  ║ 0.xx ║ {grade}       ║
╚════════════════╩═══════╩═══════════╩════════╩══════╩════════════════╝
```

Grading scale:
- A (F1 >= 0.70): Production confident
- B (F1 >= 0.50): Acceptable, room for improvement
- C (F1 >= 0.35): Functioning but needs corpus/keyword work
- D (F1 >= 0.20): Keyword-only ceiling — needs embedding activation
- F (F1 < 0.20): Broken — investigate immediately

If `--personas` flag, stop here after Phase 1 + 2.

---

## Phase 3: Golden Snapshot Stability

### 3A. Run Golden Tests
```bash
cd src-tauri && cargo test --lib simulation::golden_snapshot -- --nocapture 2>&1
```
Extract: pass/fail per golden test function, any score drift details from assertion messages.

### 3B. Analyze Golden Coverage
Read `golden_snapshot.rs` and catalog:
- Total golden expectations (item_id + persona_idx pairs)
- Coverage by persona (which personas have golden baselines?)
- Coverage by content category (which categories are baseline-tested?)
- Width of expected ranges (narrow = confident, wide = uncertain)

### 3C. Golden Confidence Assessment
For each golden expectation:
- **Tight range** (span < 0.3): High confidence in pipeline behavior
- **Medium range** (span 0.3-0.6): Moderate confidence
- **Wide range** (span > 0.6): Low confidence — item needs real embedding calibration
- **No relevance assertion**: Untested classification boundary

Present:
```
Golden Snapshot Health:
  Total expectations: {n}
  Tight confidence:   {n} ({pct}%)  ← pipeline behavior locked down
  Medium confidence:  {n} ({pct}%)  ← acceptable
  Wide confidence:    {n} ({pct}%)  ← needs calibration
  Missing relevance:  {n} ({pct}%)  ← classification boundary untested

  Coverage gaps:
  - Personas without golden items: {list}
  - Categories without golden items: {list}
```

If `--golden` flag, stop here after Phase 1 + 3.

---

## Phase 4: Content Category Blind Spots

### 4A. Category-Level Failure Analysis
Read corpus.rs and the expected outcomes per category. For each ContentCategory, determine:
- How many items have `StrongRelevant` or `WeakRelevant` expected for their target persona
- How many items have `NotRelevant` or `Excluded` for non-target personas
- Which categories have the most `MildBorderline` labels (hardest to classify)

### 4B. Identify Scoring Weaknesses
Cross-reference corpus categories with reality.rs test failures (if any) and golden_snapshot test results:
- Categories where false positives cluster (noise leaking through)
- Categories where false negatives cluster (relevant content missed)
- Categories with no test coverage at all

### 4C. Cross-Domain Confusion Matrix
For each persona, identify which non-target categories most often score above the relevance threshold:
- Does the Rust persona pick up Python content? (acceptable cross-domain interest)
- Does the DevOps persona pick up career noise? (unacceptable false positive)
- Does the bootstrap persona reject everything? (too restrictive)

Present:
```
Blind Spot Analysis:
  🔴 Critical: {categories where >50% items misclassified}
  🟡 Warning:  {categories where 25-50% items misclassified}
  🟢 Strong:   {categories where <25% items misclassified}
  ⚫ Untested: {categories with <3 corpus items}

  Top False Positive Sources:
  1. {category} → {persona}: {n} items scored relevant that shouldn't be
  2. ...

  Top False Negative Sources:
  1. {category} → {persona}: {n} items scored irrelevant that should be relevant
  2. ...
```

If `--blind-spots` flag, stop here after Phase 1 + 4.

---

## Phase 5: Separation Gap Analysis

### 5A. Score Distribution Characteristics
Read quality_dashboard.rs and reality.rs to understand separation gap calculation:
- `separation_gap = avg_relevant_score - avg_irrelevant_score`
- Higher is better — means the pipeline creates clear separation between relevant and irrelevant content

### 5B. Per-Persona Separation
For each persona, assess:
- Average score for items expected to be relevant
- Average score for items expected to be irrelevant
- The gap between them
- Whether the gap is sufficient for confident threshold placement

### 5C. Threshold Sensitivity
Read `src-tauri/src/scoring/pipeline.rs` for current threshold logic:
- Base threshold value
- Auto-tuning adjustment
- Bootstrap mode threshold
- How much room exists between the threshold and the separation gap edges

Present:
```
Separation Analysis:
  Pipeline threshold: {value}
  Bootstrap threshold: {value}

  Per-Persona Separation:
  ┌────────────────┬──────────┬──────────┬──────────┬──────────────┐
  │ Persona        │ Avg Rel  │ Avg Irr  │   Gap    │ Confidence   │
  ├────────────────┼──────────┼──────────┼──────────┼──────────────┤
  │ rust_systems   │  0.xxx   │  0.xxx   │  0.xxx   │ {level}      │
  │ ...            │          │          │          │              │
  └────────────────┴──────────┴──────────┴──────────┴──────────────┘

  Threshold Placement Quality:
  - Items within ±0.05 of threshold: {n} ({pct}%) ← borderline zone
  - Items clearly above threshold: {n} ({pct}%) ← confident relevant
  - Items clearly below threshold: {n} ({pct}%) ← confident irrelevant
```

If `--separation` flag, stop here after Phase 1 + 5.

---

## Phase 6: Simulation Systems Inventory

### 6A. System Coverage Matrix
Run each simulation system's tests and check status:

```bash
cd src-tauri && for sys in lifecycle reality first_run differential golden_snapshot tier2_semantic tier3_rerank quality_dashboard; do
  result=$(cargo test --lib "simulation::${sys}" 2>&1 | tail -1)
  echo "${sys}: ${result}"
done
```

### 6B. Feature Flag Status
Check Cargo.toml for simulation-related feature flags:
- `calibrated-sim` — enables real embedding fixtures
- `generate-sim-fixtures` — enables fixture generation
- Are fixtures present in `src-tauri/src/scoring/simulation/fixtures/`?

### 6C. Embedding Mode Impact
Assess the gap between keyword-only (current) and semantic scoring:
- Read tier2_semantic.rs tests to understand what semantic scoring adds
- Read domain_embeddings.rs to understand the synthetic embedding approach
- Determine: are the simulation results representative of production behavior?

Present:
```
Simulation Systems Status:
  ┌─────────────────────┬────────┬────────┬─────────────────────────┐
  │ System              │ Tests  │ Status │ Notes                   │
  ├─────────────────────┼────────┼────────┼─────────────────────────┤
  │ S1: Lifecycle       │  {n}   │  ✓/✗   │                         │
  │ S2: Reality         │  {n}   │  ✓/✗   │ Aggregate F1: {value}   │
  │ S3: First Run       │  {n}   │  ✓/✗   │                         │
  │ S4: Differential    │  {n}   │  ✓/✗   │                         │
  │ S5: Golden Snapshot │  {n}   │  ✓/✗   │ {n} expectations        │
  │ T2: Semantic        │  {n}   │  ✓/✗   │ Synthetic embeddings    │
  │ T3: Rerank          │  {n}   │  ✓/✗   │                         │
  │ Dashboard           │  {n}   │  ✓/✗   │                         │
  └─────────────────────┴────────┴────────┴─────────────────────────┘

  Embedding Mode:
  - Current: {keyword-only | synthetic | calibrated}
  - Fixture files: {present | missing}
  - Impact: keyword-only F1 = {value}, projected semantic F1 = {estimate}
```

---

## Phase 7: Drift Detection (runs with --drift or default)

### 7A. Load Previous Snapshot
Read `.claude/calibration-snapshot.json`. If no previous snapshot exists, skip this phase and note "First calibration run — no drift baseline."

### 7B. Compute Deltas
For each metric in the snapshot:
- Simulation test count delta
- Per-persona F1 delta
- Aggregate F1 delta
- Golden expectation count delta
- Corpus item count delta
- Any new failures vs previous passing

### 7C. Drift Assessment
```
Drift Report (vs {previous_commit} on {previous_date}):
  Simulation tests:  {prev} → {curr} ({delta})
  Aggregate F1:      {prev} → {curr} ({delta})
  Worst persona F1:  {prev} → {curr} ({delta})
  Golden pass rate:  {prev} → {curr} ({delta})
  Corpus coverage:   {prev} → {curr} ({delta})

  Regressions: {list or "none detected"}
  Improvements: {list or "none"}
```

---

## Phase 8: Calibration Recommendations

Based on ALL data gathered, produce a ranked list of calibration actions. Each recommendation must:
1. Reference specific data from this report
2. Identify the expected impact
3. Estimate effort (lines of code / files changed)
4. Target a specific metric improvement

### Priority Categories

**P0 — Fix Now** (pipeline producing wrong results):
- Any golden snapshot test failures
- Any persona with F1 < 0.15
- Separation gap negative for any persona

**P1 — Next Session** (quality ceiling limiters):
- Personas with F1 < 0.30 and no embedding support
- Categories with >50% misclassification
- Wide golden ranges (>0.6 span) on core items

**P2 — Strategic** (long-term quality investments):
- Adding real embedding fixtures (Phase 5 calibration)
- Expanding corpus for underrepresented categories
- Adding golden snapshots for uncovered personas
- Cross-domain confusion reduction

**P3 — Polish** (incremental improvements):
- Tightening golden ranges as confidence grows
- Adding boundary items to corpus for edge cases
- Documenting scoring behavior for each persona

Present:
```
╔══════════════════════════════════════════════════════════════════════╗
║                  CALIBRATION RECOMMENDATIONS                        ║
╠══════╦═══════════════════════════════════════════════════════════════╣
║  P0  ║ {Critical issue description}                                 ║
║      ║ Impact: {metric} → {expected improvement}                    ║
║      ║ Effort: {lines} lines in {files}                             ║
╠══════╬═══════════════════════════════════════════════════════════════╣
║  P1  ║ {Quality ceiling description}                                ║
║      ║ Impact: {metric} → {expected improvement}                    ║
║      ║ Effort: {lines} lines in {files}                             ║
╠══════╬═══════════════════════════════════════════════════════════════╣
║  P2  ║ {Strategic investment description}                           ║
║      ║ Impact: {metric} → {expected improvement}                    ║
║      ║ Effort: {lines} lines in {files}                             ║
╚══════╩═══════════════════════════════════════════════════════════════╝
```

---

## Phase 9: Snapshot and Report

### 9A. Save Calibration Snapshot
Write to `.claude/calibration-snapshot.json`:
```json
{
  "timestamp": "{ISO 8601}",
  "commit": "{hash}",
  "simulation_tests": {count},
  "scoring_tests": {count},
  "corpus_items": {count},
  "content_categories": {count},
  "golden_expectations": {count},
  "per_persona": {
    "rust_systems": { "f1": 0.0, "precision": 0.0, "recall": 0.0, "separation": 0.0 },
    ...
  },
  "aggregate_f1": 0.0,
  "aggregate_precision": 0.0,
  "aggregate_recall": 0.0,
  "embedding_mode": "keyword-only|synthetic|calibrated",
  "golden_pass_rate": 1.0,
  "recommendations_p0": 0,
  "recommendations_p1": 0
}
```

### 9B. Compute Calibration Grade
Overall grade based on weighted factors:
- 30% — Aggregate F1
- 20% — Worst persona F1 (no persona left behind)
- 15% — Golden snapshot pass rate
- 15% — Separation gap quality
- 10% — Corpus coverage completeness
- 10% — Simulation test health

Grade scale:
- **S** (95+): Production-grade scoring with real embeddings
- **A** (80-94): Strong pipeline with minor gaps
- **B** (65-79): Solid keyword-only with clear upgrade path
- **C** (50-64): Functional but limited by embedding mode
- **D** (35-49): Significant blind spots need attention
- **F** (<35): Pipeline unreliable — prioritize P0 fixes

### 9C. Final Report Header
```
╔══════════════════════════════════════════════════════════════════════╗
║                 4DA SCORING CALIBRATION REPORT                      ║
║                                                                      ║
║  Commit: {hash}  |  Date: {date}  |  Grade: {letter}               ║
║  Pipeline: 22-stage PASIFA  |  Mode: {keyword-only|semantic}        ║
║  Corpus: {n} items  |  Personas: 9  |  Sim Tests: {n}              ║
╚══════════════════════════════════════════════════════════════════════╝
```

</process>

<guardrails>
- NEVER modify scoring logic, thresholds, or pipeline code
- NEVER modify test assertions or expectations
- NEVER modify corpus items or persona definitions
- Only write to `.claude/calibration-snapshot.json`
- All metrics must come from live test execution, not cached values
- If a test fails, report the failure accurately — do not explain it away
- If embedding mode is keyword-only, state this clearly as a quality ceiling
- Recommendations must be specific and actionable, not generic advice
</guardrails>
