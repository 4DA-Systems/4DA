# Phase 0: Relevance Microscope

> **STATUS: COMPLETED** (2026-01-25)
> Hypothesis validated. Embedding similarity works for relevance scoring.
> 16/30 → 25/30+ relevant items achieved with PASIFA improvements.

**Version:** 1.0.0
**Status:** ~~Active~~ **COMPLETED**
**Purpose:** Scientific instrument, not prototype

---

## Single Objective

**Prove:** "A narrow, intentional local context can surface non-obvious, externally relevant items using embedding similarity alone."

This is the ONLY thing Phase 0 must demonstrate.

---

## What Phase 0 Is

- A relevance microscope
- A scientific instrument
- An experiment with observable outcomes
- A test of the core hypothesis

## What Phase 0 Is NOT

- A product
- A UX demo
- A performance benchmark
- A privacy showcase
- A learning system

---

## Deliverables

### 1. Tauri Shell
- Bare window with matte black background
- Title: "4DA - Phase 0"
- No polish, no animations
- Functional, not beautiful

### 2. File Indexer
- **Single hardcoded directory** (test context)
- File types: `.md`, `.txt`, `.rs`, `.ts`, `.js`, `.py`
- Extract raw text content
- Generate embeddings per file (chunked if >500 words)
- Store in memory (`Vec<EmbeddedChunk>`)

### 3. Embedding Model
- **MiniLM (all-MiniLM-L6-v2)** via ONNX runtime
- 384 dimensions
- Local only, no API calls
- Deterministic outputs

### 4. Hacker News Adapter
- Fetch top 30 stories from HN API
- For each story: fetch title, URL, scrape content
- Generate embeddings for title + content
- Cache responses for deterministic reruns

### 5. Similarity Engine
- Brute-force cosine similarity
- Compare each HN item against all context chunks
- Return top-N matching chunks per item
- No threshold filtering in Phase 0 - show everything ranked

### 6. Output (Console + Basic UI)

**Console output (required):**
```
=== HN Item: "SQLite as a document database" ===
URL: https://...
Similarity Scores:
  0.847 | sqlite-notes.md:23-45 | "SQLite supports..."
  0.721 | vector-search.md:12-30 | "Embedding similarity..."
  0.654 | rust-ideas.md:5-15 | "Database abstraction..."
Decision: RELEVANT (top score > 0.7)
Matched on: ["sqlite", "database", "vector"]

=== HN Item: "New Pokemon game announced" ===
URL: https://...
Similarity Scores:
  0.234 | random-notes.md:1-10 | "..."
  0.198 | ...
Decision: NOT RELEVANT (top score < 0.4)
```

**UI output (minimal):**
- Ranked list of all HN items
- Show similarity score prominently
- Show "why it matched" (top keywords/chunks)
- Show items regardless of relevance (no filtering)

---

## Explicitly Deferred

| Feature | Reason | Phase |
|---------|--------|-------|
| sqlite-vss | Build friction, opaque debugging | 0.5 |
| Activity tracking | Not needed for hypothesis | 2 |
| Cost controls | No API costs in Phase 0 | 1 |
| Learning engine | Relevance must work first | 2 |
| Notifications | Delivery is separate concern | 1 |
| Multiple sources | One source validates hypothesis | 1 |
| Cloud embeddings | Introduces variance | 1 |
| Polished UX | Clarity over aesthetics | 3 |

---

## Test Context Directory

Create `/mnt/d/4DA/test-context/` with intentional content:

```
test-context/
├── sqlite-notes.md       # Notes about SQLite
├── vector-search.md      # Notes about embeddings
├── tauri-thoughts.md     # Notes about Tauri/Rust
├── local-first.md        # Notes about local-first software
├── typescript-patterns.md # Notes about TS patterns
├── random-cooking.md     # NEGATIVE CONTROL - cooking recipes
└── random-sports.md      # NEGATIVE CONTROL - sports news
```

**Why negative controls?**
- If the system scores cooking/sports content highly against tech HN posts, something is wrong
- Controls tell us if the system is actually discriminating

---

## Success Criteria

### Must Have
- [ ] System runs without crashing
- [ ] Embeddings are generated for test context
- [ ] HN items are fetched and embedded
- [ ] Similarity scores are computed and displayed
- [ ] At least ONE "holy shit, that's relevant" moment

### Validation Questions
1. Do tech-related HN posts score higher against tech context files?
2. Do non-tech HN posts score low against all context?
3. Can we see WHY something matched (which chunks, which terms)?
4. Are scores stable across runs (determinism)?

### Failure Indicators
- All scores cluster around same value (no discrimination)
- Negative controls score as high as positive content
- Cannot explain why matches occurred
- Non-deterministic results

---

## Technical Decisions

### Embedding Model: MiniLM via ONNX

**Why MiniLM:**
- 384 dimensions (smaller, faster)
- Well-documented
- Deterministic
- Free
- Matches sqlite-vss expectations for Phase 0.5

**Why ONNX:**
- Rust bindings available (`ort` crate)
- Cross-platform
- No Python dependency
- Reasonable bundle size

### Vector Storage: In-Memory

**Why not SQLite yet:**
- Transparency: can inspect Vec directly
- Debugging: println!("{:?}", vector)
- No schema friction
- Clear failure attribution

**Structure:**
```rust
struct EmbeddedChunk {
    source_file: PathBuf,
    line_start: usize,
    line_end: usize,
    text: String,
    embedding: Vec<f32>,  // 384 dimensions
}

struct EmbeddedHNItem {
    id: u64,
    title: String,
    url: String,
    content: String,
    embedding: Vec<f32>,
}
```

### Similarity: Brute Force Cosine

```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b)
}
```

No ANN, no indexing, no optimization. Clarity over speed.

---

## First Commit Target

The absolute minimum to validate we're on the right track:

1. Tauri window opens (matte black)
2. Reads files from `test-context/`
3. Prints file contents to console
4. Fetches HN top stories
5. Prints titles to console

No embeddings yet. Just prove the data flows.

---

## Deterministic Runs

For tuning and debugging, we need reproducibility:

1. **Freeze HN data:** Save fetched stories to `test-data/hn-snapshot.json`
2. **Replay mode:** Load from snapshot instead of live API
3. **Same context:** Don't change test files mid-experiment

This allows:
- Threshold tuning without API variance
- A/B testing embedding approaches
- Debugging specific failures

---

## Timeline

Phase 0 is complete when:
- We can run the system
- We see ranked results
- We understand why matches occur
- We have at least one "holy shit" moment

No time estimate. Done when the hypothesis is answered.

---

*This document supersedes ARCHITECTURE.md for Phase 0 scope. ARCHITECTURE.md remains the long-term vision.*
