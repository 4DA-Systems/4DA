# 4DA: Privacy-Aware Semantic Intelligence Implementation Plan

> **Note**: This roadmap covers the NEXT 18 months of development.
> The initial launch features (7 source adapters, PASIFA, Score Autopsy) are COMPLETE.
> See `MISSION_ACCOMPLISHED.md` for that work.
>
> **Current Status**: Phase 2 Core Complete (2026-02-05). Query system working, hybrid search implemented.

## Executive Summary

Transform 4DA from a relevance filtering tool into a complete privacy-aware semantic intelligence system over 18 months through 4 major phases:

1. **Phase 1 (Months 1-3)**: Multi-format file support (PDF, Office, OCR, Audio)
2. **Phase 2 (Months 4-6)**: Natural language query system
3. **Phase 3 (Months 7-12)**: Knowledge graph construction
4. **Phase 4 (Months 13-18)**: Proactive intelligence engine

**Current State**: Solid foundation with local embeddings, PASIFA scoring, ACE context engine, multi-source integration
**Target State**: Private cognitive mirror that understands, connects, and proactively surfaces relevant information

---

## PHASE 1: Multi-Format File Support (Months 1-3)

### Objective
Enable 4DA to read and index PDFs, Office documents, images (OCR), audio files, and archives.

### Implementation Approach

#### 1.1 Architecture
Create new extractor module: `src-tauri/src/extractors/`

```
extractors/
├── mod.rs           # Unified DocumentExtractor trait
├── pdf.rs           # PDF text extraction
├── office.rs        # DOCX, XLSX extraction
├── image.rs         # OCR via Tesseract
├── audio.rs         # Whisper transcription
└── archive.rs       # ZIP, TAR recursive extraction
```

**Unified Interface**:
```rust
pub trait DocumentExtractor {
    fn supported_extensions(&self) -> &[&str];
    fn extract(&self, path: &Path) -> Result<ExtractedDocument, String>;
}

pub struct ExtractedDocument {
    pub text: String,
    pub metadata: HashMap<String, String>,
    pub pages: Vec<PageContent>,
    pub confidence: f32,  // For OCR quality
}
```

#### 1.2 Database Schema Updates

```sql
-- Extend context_chunks for multi-format metadata
ALTER TABLE context_chunks ADD COLUMN source_type TEXT DEFAULT 'text';
ALTER TABLE context_chunks ADD COLUMN page_number INTEGER;
ALTER TABLE context_chunks ADD COLUMN confidence REAL DEFAULT 1.0;
ALTER TABLE context_chunks ADD COLUMN extracted_at TEXT;

-- Track extraction jobs for async processing
CREATE TABLE extraction_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    file_type TEXT NOT NULL,
    status TEXT NOT NULL,  -- pending, processing, completed, failed
    error TEXT,
    started_at TEXT,
    completed_at TEXT,
    extracted_chunks INTEGER DEFAULT 0
);

-- Cache file metadata to avoid re-processing
CREATE TABLE file_metadata_cache (
    file_path TEXT PRIMARY KEY,
    file_hash TEXT NOT NULL,
    file_type TEXT NOT NULL,
    page_count INTEGER,
    word_count INTEGER,
    extracted_at TEXT NOT NULL
);
```

#### 1.3 Implementation Order

**Month 1: PDF + Office**
- Add `pdf-extract` crate for PDF text extraction
- Add `docx-rs` for Word documents
- Add `calamine` for Excel spreadsheets
- Integrate with existing file watcher

**Month 2: OCR + Audio**
- Add `tesseract` crate + local Tesseract binary
- Add `whisper-rs` + download local Whisper models (tiny/base)
- Implement confidence scoring for OCR
- Background job queue for slow operations

**Month 3: Archives + Polish**
- Add `zip` and `tar` crates
- Recursive extraction with depth limits
- Testing across 100+ diverse files
- Performance tuning (batch processing, caching)

#### 1.4 New Dependencies

```toml
pdf-extract = "0.7"
lopdf = "0.32"
docx-rs = "0.4"
calamine = "0.25"
tesseract = "0.14"
image = "0.25"
whisper-rs = "0.11"
zip = "0.6"
tar = "0.4"
```

#### 1.5 Privacy Guarantee
All extraction happens locally - no cloud APIs required.

---

## PHASE 2: Natural Language Query System (Months 4-6)

### Objective
Enable queries like "show me files where I was stressed about money" instead of keyword search.

### Implementation Approach

#### 2.1 Architecture
Create query parsing module: `src-tauri/src/query/`

```
query/
├── mod.rs           # Query coordinator
├── parser.rs        # LLM-powered NL → structured query
├── planner.rs       # Query execution planning
├── executor.rs      # Hybrid vector + SQL search
└── filters.rs       # Time, entity, sentiment filters
```

**Query Pipeline**:
```rust
pub struct ParsedQuery {
    pub intent: QueryIntent,           // Find, Summarize, Compare, Timeline
    pub entities: Vec<Entity>,         // "John", "money", "Project X"
    pub temporal: Option<TimeRange>,   // "last month", "2023"
    pub sentiment: Option<Sentiment>,  // "stressed", "excited"
    pub semantic_embedding: Vec<f32>,
}
```

#### 2.2 Database Schema Updates

```sql
-- Sentiment analysis cache
CREATE TABLE chunk_sentiment (
    chunk_id INTEGER PRIMARY KEY,
    sentiment TEXT NOT NULL,  -- positive, negative, neutral
    confidence REAL NOT NULL,
    analyzed_at TEXT NOT NULL,
    FOREIGN KEY (chunk_id) REFERENCES context_chunks(id)
);

-- Query cache for performance
CREATE TABLE query_cache (
    query_hash TEXT PRIMARY KEY,
    natural_language TEXT NOT NULL,
    parsed_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Query history for learning
CREATE TABLE query_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    query TEXT NOT NULL,
    results_count INTEGER NOT NULL,
    user_clicked BOOLEAN,
    timestamp TEXT NOT NULL
);
```

#### 2.3 Implementation Order

**Month 4: Query Parser**
- LLM-powered parsing (Haiku/GPT-4o-mini)
- Cost optimization: cache parsed queries
- Structured query builder

**Month 5: Entity + Temporal**
- Local NER via `rust-bert` (privacy-first)
- Temporal parsing with `chrono-english`
- Entity filters in SQL

**Month 6: Sentiment + Integration**
- Local sentiment model (DistilBERT)
- Hybrid vector + SQL execution
- Query history tracking

#### 2.4 New Dependencies

```toml
rust-bert = "0.22"          # Local NER + sentiment
tokenizers = "0.15"
chrono-english = "0.1"      # "last month" → DateTime
```

#### 2.5 Performance Target
Query latency: <500ms (90th percentile)

---

## PHASE 3: Knowledge Graph (Months 7-12)

### Objective
Build semantic relationships between entities (people, orgs, projects, topics) across all files.

### Implementation Approach

#### 3.1 Architecture
Create graph module: `src-tauri/src/graph/`

```
graph/
├── mod.rs              # Graph coordinator
├── extractor.rs        # Entity + relationship extraction
├── resolver.rs         # Coreference resolution
├── storage.rs          # Graph database layer
└── query.rs            # Graph traversal queries
```

#### 3.2 Database Schema Updates

```sql
-- Entities (people, organizations, projects, topics)
CREATE TABLE entities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    type TEXT NOT NULL,  -- person, org, project, topic, location
    aliases TEXT,        -- JSON array
    embedding BLOB,      -- 384-dim
    first_seen TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    mention_count INTEGER DEFAULT 1,
    confidence REAL DEFAULT 1.0
);

-- Relationships
CREATE TABLE relationships (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_entity_id INTEGER NOT NULL,
    target_entity_id INTEGER NOT NULL,
    relation_type TEXT NOT NULL,  -- works_with, mentioned_in, part_of
    strength REAL DEFAULT 0.5,
    evidence_count INTEGER DEFAULT 1,
    first_seen TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    FOREIGN KEY (source_entity_id) REFERENCES entities(id),
    FOREIGN KEY (target_entity_id) REFERENCES entities(id)
);

-- Entity mentions in specific chunks
CREATE TABLE entity_mentions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id INTEGER NOT NULL,
    chunk_id INTEGER NOT NULL,
    context TEXT NOT NULL,
    position INTEGER NOT NULL,
    confidence REAL NOT NULL,
    mentioned_at TEXT NOT NULL,
    FOREIGN KEY (entity_id) REFERENCES entities(id),
    FOREIGN KEY (chunk_id) REFERENCES context_chunks(id)
);

-- Vector index for entity embeddings
CREATE VIRTUAL TABLE entity_vec USING vec0(
    embedding float[384]
);
```

#### 3.3 Implementation Order

**Months 7-8: Entity Extraction**
- NER model integration (rust-bert)
- Entity extraction from all indexed files
- Populate entities table

**Month 9: Coreference Resolution**
- Resolve "he", "the engineer", "John" → same entity
- Fuzzy matching + embedding similarity
- Entity deduplication

**Months 10-11: Relationship Extraction**
- Pattern-based extraction ("X works at Y")
- Populate relationships table
- Evidence tracking

**Month 12: Graph Queries**
- BFS/DFS traversal
- Shortest path between entities
- Entity timeline views

#### 3.4 New Dependencies

```toml
strsim = "0.11"             # String similarity for entity matching
petgraph = "0.6"            # Graph algorithms
```

#### 3.5 Performance Target
Graph query (depth 3): <150ms

---

## PHASE 4: Proactive Intelligence (Months 13-18)

### Objective
Surface insights before being asked - become a "cognitive mirror" that anticipates needs.

### Implementation Approach

#### 4.1 Architecture
Create proactive module: `src-tauri/src/proactive/`

```
proactive/
├── mod.rs              # Proactive coordinator
├── intent_predictor.rs # Predict user's current intent
├── surfacer.rs         # Smart timing + delivery
├── patterns.rs         # Long-term pattern detection
└── quiet_mode.rs       # Detect deep work
```

**Intent Types**:
```rust
pub enum PredictedIntent {
    ResearchingTopic(String),        // Opening many PDFs
    WritingDocument(String),         // Editing markdown
    DebuggingCode(String),           // Opening logs/errors
    PreparingPresentation(String),   // Opening slides
    ReviewingFinances,               // Opening statements
}
```

#### 4.2 Database Schema Updates

```sql
-- File access patterns for intent prediction
CREATE TABLE file_access_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    access_type TEXT NOT NULL,  -- open, edit, close
    duration_seconds INTEGER,
    timestamp TEXT NOT NULL
);

-- Detected patterns
CREATE TABLE detected_patterns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_type TEXT NOT NULL,
    pattern_data TEXT NOT NULL,  -- JSON
    confidence REAL NOT NULL,
    first_detected TEXT NOT NULL,
    last_confirmed TEXT NOT NULL
);

-- Proactive insights queue
CREATE TABLE proactive_insights (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    relevance_score REAL NOT NULL,
    surfaced_at TEXT,
    dismissed_at TEXT,
    clicked BOOLEAN DEFAULT 0,
    expires_at TEXT,
    created_at TEXT NOT NULL
);

-- Focus sessions for quiet mode
CREATE TABLE focus_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    primary_app TEXT,
    files_touched INTEGER DEFAULT 0,
    deep_work_score REAL
);
```

#### 4.3 Implementation Order

**Months 13-14: Intent Prediction**
- File activity pattern analysis
- Intent classification
- Context caching

**Month 15: Proactive Surfacing**
- Natural break detection (don't interrupt deep work)
- Insight generation
- UI notification integration

**Months 16-17: Pattern Detection**
- Time-series analysis (FFT for cycles)
- Collaboration patterns
- Learning journeys

**Month 18: Polish + Quiet Mode**
- Deep work detection
- Timing intelligence
- User feedback loop

#### 4.4 New Dependencies

```toml
rustfft = "6.1"              # Cycle detection in patterns
```

#### 4.5 Performance Target
Intent prediction: <100ms (background thread)

---

## CROSS-CUTTING ENHANCEMENTS

### Privacy Modes

Add tiered privacy settings to `settings.rs`:

```rust
pub enum PrivacyMode {
    Paranoid,      // 100% local, no cloud ever
    Balanced,      // Local + BYOK (user's API keys)
    Convenience,   // Allow managed cloud APIs
}
```

**User can switch modes at any time.**

### Temporal Context Weighting

Modify PASIFA scoring to weight recent work higher:

```rust
fn temporal_weight(age: Duration) -> f32 {
    // Exponential decay: recent = 1.0, 30 days old = 0.5
    (-age.num_days() as f32 / 30.0).exp()
}
```

### Enhanced Explanations

Add natural language explanations to `lib.rs`:

```rust
impl RelevanceExplainer {
    pub fn generate_explanation(&self, item: &SourceItem, breakdown: &ScoreBreakdown) -> String {
        // "This relates to your work in auth.rs (opened yesterday).
        //  It matches your interest in Rust authentication.
        //  Your project uses tokio which is mentioned here."
    }
}
```

---

## CRITICAL FILES TO MODIFY

### Phase 1 (Multi-Format)
1. **New**: `src-tauri/src/extractors/mod.rs` - Extractor registry
2. **New**: `src-tauri/src/extractors/{pdf,office,image,audio,archive}.rs` - Implementations
3. **Modify**: `src-tauri/src/db.rs` - Schema migrations (lines 104-206)
4. **Modify**: `src-tauri/src/ace/watcher.rs` - Integrate new extractors (lines 334-356)
5. **Modify**: `src-tauri/Cargo.toml` - Add 9 new dependencies

### Phase 2 (NL Queries)
1. **New**: `src-tauri/src/query/mod.rs` - Query coordinator
2. **New**: `src-tauri/src/query/{parser,planner,executor,filters}.rs` - Query pipeline
3. **Modify**: `src-tauri/src/llm.rs` - Add query parsing methods (lines 353-547)
4. **Modify**: `src-tauri/src/lib.rs` - Add query Tauri commands (lines 1729-2175)
5. **Modify**: `src-tauri/Cargo.toml` - Add 3 new dependencies

### Phase 3 (Knowledge Graph)
1. **New**: `src-tauri/src/graph/mod.rs` - Graph coordinator
2. **New**: `src-tauri/src/graph/{extractor,resolver,storage,query}.rs` - Graph pipeline
3. **Modify**: `src-tauri/src/db.rs` - Add graph tables
4. **Modify**: `src-tauri/src/lib.rs` - Add graph query commands
5. **Modify**: `src-tauri/Cargo.toml` - Add 2 new dependencies

### Phase 4 (Proactive)
1. **New**: `src-tauri/src/proactive/mod.rs` - Proactive coordinator
2. **New**: `src-tauri/src/proactive/{intent_predictor,surfacer,patterns,quiet_mode}.rs`
3. **Modify**: `src-tauri/src/ace/mod.rs` - Integrate intent prediction (lines 1-100)
4. **Modify**: `src-tauri/src/lib.rs` - Add proactive check commands
5. **Modify**: `src-tauri/Cargo.toml` - Add 1 new dependency

---

## TESTING & VERIFICATION

### Unit Tests (Each Phase)
- Target: >80% code coverage
- Run: `cargo test --all-features`

### Integration Tests
```bash
# Phase 1: Multi-format extraction
cargo test test_pdf_extraction
cargo test test_ocr_confidence

# Phase 2: NL queries
cargo test test_query_parsing
cargo test test_sentiment_filter

# Phase 3: Knowledge graph
cargo test test_entity_extraction
cargo test test_coreference

# Phase 4: Proactive
cargo test test_intent_prediction
cargo test test_quiet_mode_detection
```

### Performance Benchmarks
```bash
cargo bench --bench vector_search
cargo bench --bench query_parsing
cargo bench --bench graph_traversal
```

### End-to-End Verification

**Phase 1 Verification**:
1. Index a PDF file: `cargo run -- index test.pdf`
2. Verify in database: Check `context_chunks` table has PDF content
3. Search PDF content: Query should return relevant chunks
4. Expected: PDF text extracted with confidence=1.0

**Phase 2 Verification**:
1. Natural language query: "show me files about authentication from last week"
2. Check parsing: Query should extract entities=["authentication"], temporal=[7 days]
3. Execute search: Should return auth-related files from past week
4. Expected: Results sorted by relevance

**Phase 3 Verification**:
1. Graph query: "find all entities connected to Project X"
2. Check graph: Should traverse relationships
3. Display entity network
4. Expected: BFS results with relationship types

**Phase 4 Verification**:
1. Simulate research activity: Open 5 PDFs about "Rust"
2. Check intent prediction: Should detect "ResearchingTopic(Rust)"
3. Surface insight: Should suggest related arXiv papers
4. Expected: Proactive notification with relevant content

---

## RISK MITIGATION

### Technical Risks
- **OCR quality**: Implement confidence scoring, allow user review
- **Whisper speed**: Use "tiny" model, async processing, show progress
- **Graph memory**: Lazy loading, prune low-confidence entities
- **Query accuracy**: Fallback to keyword search if parsing fails

### Privacy Risks
- **Data leakage**: Audit all network calls, add privacy mode selection
- **Model telemetry**: Use offline models only
- **Metadata exposure**: Keep all processing local

### Performance Risks
- **Index time**: Batch processing, incremental updates
- **Memory usage**: Lazy model loading, 5min timeout for unloading
- **Query latency**: Aggressive caching, async background jobs

---

## SUCCESS METRICS

### Technical Metrics
- Query latency: <500ms (90th percentile)
- Index speed: >100 files/minute
- Memory usage: <1.5GB peak
- Disk growth: <2GB per 100k files

### User Metrics
- Daily engagement rate: >30%
- Query success rate: >70%
- Proactive insight relevance: >60%
- User satisfaction: >4.5/5 (beta feedback)

### Privacy Metrics
- Zero unintended network calls
- 100% local processing in Paranoid mode
- BYOK adoption: >80% of users

---

## DEPENDENCIES SUMMARY

**Total new crates: 16**

Phase 1 (9): pdf-extract, lopdf, docx-rs, calamine, tesseract, image, whisper-rs, zip, tar
Phase 2 (3): rust-bert, tokenizers, chrono-english
Phase 3 (2): strsim, petgraph
Phase 4 (1): rustfft
Cross-cutting (1): Already have chrono

**External binaries required**:
- Tesseract OCR (Phase 1, Month 2)
- Whisper models (Phase 1, Month 2)

---

## MIGRATION STRATEGY

### Schema Migrations
Each phase adds new tables, never drops old ones. Migrations run on first launch:

```rust
pub fn migrate_to_phase_1(conn: &Connection) -> Result<(), String> {
    conn.execute("ALTER TABLE context_chunks ADD COLUMN source_type TEXT DEFAULT 'text'")?;
    // ... more migrations
    Ok(())
}
```

### Feature Flags
Enable new features via settings.json:

```json
{
  "features": {
    "multi_format_indexing": true,
    "natural_language_queries": false,  // Not ready yet
    "knowledge_graph": false,
    "proactive_insights": false
  }
}
```

### User Communication
In-app notifications for each phase release:
- "Phase 1: PDF & Office support now available!"
- "Re-scan directories to enable: Settings > Context > Re-index"

---

## TIMELINE SUMMARY

**Month 1-3**: Multi-format file support
**Month 4-6**: Natural language queries
**Month 7-12**: Knowledge graph (6 months - most complex)
**Month 13-18**: Proactive intelligence

**Total: 18 months to full vision**

Intermediate milestones can be released incrementally (Phase 1 after Month 3, Phase 2 after Month 6, etc.).

---

## EXECUTION READINESS

✅ All dependencies identified
✅ All database schemas designed
✅ All module structures planned
✅ All critical files mapped
✅ All tests specified
✅ All risks assessed
✅ Performance targets set
✅ Privacy guarantees defined

**Ready to execute Phase 1 immediately upon plan approval.**
