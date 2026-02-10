# 4DA Roadmap

> **Last updated**: 2026-02-11
> **Current version**: 1.0.0 (ship-ready)

---

## What's Built (Complete)

Everything below is implemented, tested, and shipping.

### Core Intelligence Loop
- **8 source adapters**: HN, arXiv, Reddit, GitHub, RSS, YouTube, Twitter/X, Product Hunt
- **PASIFA scoring**: Unified pipeline — KNN context similarity + interest embeddings + semantic ACE boost + affinity multipliers + anti-topic penalties + temporal freshness + feedback signals
- **Signal classifier**: 6 signal types, 4 priority levels, heartbeat-aware
- **Differential analysis**: Only scores new items since last run
- **Background monitoring**: Scheduler silently pushes new items via events
- **Retry with backoff**: 3 attempts (1s/3s/9s) on API failures

### Context Engine (ACE)
- Autonomous directory discovery (zero-config)
- 77+ technology manifest detection
- README indexing into semantic search
- File watcher with incremental re-indexing
- Behavior learning (affinities + anti-topics from interactions)
- PASIFA semantic topic matching via embeddings

### Multi-Format File Support
- **PDF**: `pdf-extract` + `lopdf` — text extraction with page tracking
- **Office**: `docx-rs` (Word) + `calamine` (Excel)
- **Images/OCR**: `ocrs` (pure Rust) + `rten` — no system deps
- **Archives**: `zip` + `tar` + `flate2` — recursive extraction
- **Audio**: Stub extractor (Whisper deferred — needs LLVM/system libs)
- Unified `DocumentExtractor` trait, `ExtractorRegistry`, background job queue

### Natural Language Query (Partial)
- `query/parser.rs`: LLM-powered NL-to-structured-query parsing
- `query/executor.rs`: Hybrid vector + SQL search execution
- `query/filters.rs`: Time range + entity + keyword filters
- Tauri command `natural_language_query` wired up
- **Not implemented**: NER (rust-bert rejected — 500MB dep), sentiment analysis, chrono-english temporal parsing

### Frontend
- React 18 + TypeScript + Tailwind — matte black minimalist design
- Zustand store (11 slices, centralized state)
- Onboarding wizard (6 steps, Ollama auto-pull)
- Settings modal, source filter bar, keyboard shortcuts
- Score Autopsy UI, confidence indicators, signals panel
- Void Engine heartbeat (WebGL2 + CSS fallback)
- Immediate feedback loop (save/dismiss adjusts scores in real-time)
- Render limit pagination (50 items + "Show More")

### Infrastructure
- MCP server v3.2.0 (13 tools, 14 resources)
- System tray + background monitoring
- Digest system (text/HTML/markdown) with signal sections
- Ollama auto-pull during onboarding (nomic-embed-text + llama3.2)
- 0 warnings (ESLint + Clippy), 217 tests, 373KB bundle

---

## Next Up: High-Value Improvements

These are concrete, achievable improvements that would make 4DA meaningfully better. Ordered by impact-to-effort ratio.

### 1. Smarter Temporal Queries
**Effort**: Small | **Impact**: Medium

The query parser exists but doesn't handle "last week", "this month", "yesterday" natively. Currently relies on the LLM to extract dates.

- Add `chrono-english` crate (~5KB) for natural date parsing
- Wire into `query/filters.rs` `TimeRange` extraction
- Fallback: LLM parsing (already works, just slower)

### 2. Source-Specific Refresh Rates
**Effort**: Small | **Impact**: Medium

HN and Reddit move fast (refresh every 15min makes sense). arXiv and GitHub move slow (daily is fine). Currently all sources share one interval.

- Add per-source `refresh_interval_minutes` to settings
- Fast sources: HN, Reddit, Twitter (15min default)
- Slow sources: arXiv, GitHub, YouTube, Product Hunt (6hr default)
- RSS: user-configurable per feed

### 3. Export & Sharing
**Effort**: Small | **Impact**: Medium

`export_results` command exists but only does JSON. Users want to share findings.

- Add markdown export (readable briefing format)
- Add CSV export (for spreadsheet users)
- Add "copy link list" (just URLs, for Slack/email)

### 4. Digest Scheduling
**Effort**: Medium | **Impact**: High

Digest generation exists but is manual. The "ambient intelligence" promise means it should arrive automatically.

- Morning digest at user-configured time (default 8am)
- System notification with digest summary
- "Quiet hours" respect (no notifications during focus time)
- Desktop notification with top 3 items

### 5. Multi-Device Sync (Settings Only)
**Effort**: Medium | **Impact**: Medium

Settings, interests, and exclusions are local to one machine. Users with multiple computers repeat setup.

- Export/import settings as encrypted JSON file
- No cloud — file-based sync (Dropbox, iCloud, USB)
- Covers: interests, exclusions, tech stack, source configs, API keys (encrypted)
- Does NOT sync: embeddings, cached items, analysis results (too large, privacy risk)

---

## Future Vision (Research Phase)

These are ambitious features that need design work before implementation. Not committed to a timeline.

### Knowledge Graph
Build semantic relationships between entities across all indexed files. Would enable queries like "show me everything connected to Project X" or "who works on authentication?"

**Key challenges**:
- Entity extraction without rust-bert (LLM-based? regex patterns?)
- Coreference resolution ("he", "the engineer", "John" = same entity)
- Graph storage in SQLite (adjacency list vs dedicated graph tables)
- UI for graph visualization (force-directed layout?)

**Prerequisite**: Validate that users actually want this. The current search + signals may be sufficient.

### Proactive Intelligence
Surface insights before being asked. Detect that you're researching a topic and proactively fetch related content.

**Key challenges**:
- Intent prediction from file access patterns (need OS-level file monitoring)
- "Don't interrupt deep work" — quiet mode detection
- Balancing proactive notifications vs noise (the whole point of 4DA is less noise)
- Privacy implications of tracking file access patterns

**Prerequisite**: Solid usage data showing users want proactive surfacing rather than on-demand analysis.

### Audio Transcription
Whisper integration for meeting notes, podcasts, voice memos.

**Key challenges**:
- `whisper-rs` requires LLVM and system-level deps (failed on Windows)
- Alternative: `whisper.cpp` via FFI, or Ollama's whisper support
- Large model files (tiny=75MB, base=142MB, small=466MB)
- Real-time vs batch processing

**Prerequisite**: Find a pure-Rust or easily-bundled Whisper solution.

---

## Rejected Ideas

| Idea | Why Not |
|------|---------|
| rust-bert for NER/sentiment | 500MB+ dep, marginal improvement over LLM-based approach |
| Electron migration | Tauri is 10x smaller, 5x faster — no reason to switch |
| Cloud sync of embeddings | Violates privacy-first principle |
| Full AST parsing of source code | Over-engineering — README + manifest scanning is sufficient |
| Separate embedding spaces per source | Adds complexity without improving relevance |
| Three.js 3D universe visualization | 908KB bundle for unclear UX value — heartbeat is better |

---

## Architecture Reference

```
src-tauri/src/
├── lib.rs              # ~2720 lines — app setup, embedding, utilities
├── scoring.rs          # ~1079 lines — unified PASIFA scoring pipeline
├── analysis.rs         # ~750 lines — 3 analysis paths + differential + background
├── source_fetching.rs  # ~792 lines — all 8 source adapters
├── db.rs               # ~1150 lines — SQLite + sqlite-vec
├── settings.rs         # ~710 lines — settings management
├── signals.rs          # ~300 lines — signal classification
├── void_engine.rs      # ~850 lines — heartbeat + universe (gated)
├── ace/mod.rs          # ~1734 lines — autonomous context engine
├── ace_commands.rs     # ~1240 lines — ACE Tauri commands
├── context_engine.rs   # identity, interests, interactions
├── llm.rs              # LLM integration (Anthropic/OpenAI/Ollama)
├── extractors/         # PDF, Office, Image/OCR, Audio, Archive
├── query/              # NL parser, executor, filters
├── digest.rs           # text/HTML/markdown digest generation
├── monitoring.rs       # background scheduler
└── [8 command modules]  # Tauri command handlers

src/
├── App.tsx             # ~830 lines — main app shell
├── store/index.ts      # ~1193 lines — Zustand store (11 slices)
├── components/         # UI components
├── hooks/              # React hooks
└── types.ts            # shared TypeScript types
```

**Key numbers**: 86+ Tauri commands, 217 tests, 373KB bundle, 0 warnings.
