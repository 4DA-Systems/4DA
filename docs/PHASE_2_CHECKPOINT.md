# Phase 2: Natural Language Query System - Checkpoint

> **Scope**: Natural language queries, hybrid search, time/sentiment filters
> **Status**: Core Complete (80%) - Query system working, hybrid search implemented
> **Updated**: 2026-02-05

---

## Summary

Phase 2 adds natural language query capabilities:
- **Query Parser** - Keyword-based intent detection (Find, Summarize, Compare, Timeline, Count)
- **Hybrid Search** - Vector similarity + keyword matching
- **Time Filters** - "last week", "yesterday", "last month", etc.
- **Sentiment Filters** - "stressed", "happy", "frustrated", etc.
- **File Type Filters** - "pdf", "docx", "xlsx", etc.

---

## What's Implemented

### Query Module Architecture
**Location**: `src-tauri/src/query/`

| File | Status | Lines | Purpose |
|------|--------|-------|---------|
| `mod.rs` | Complete | 22 | Module exports |
| `parser.rs` | Complete | 206 | NL → structured query |
| `executor.rs` | Complete | 460 | Hybrid keyword + vector search |
| `filters.rs` | Complete | 340 | Time, entity, sentiment filters |

### Query Parser Features
- **Intent Detection**: Find, Summarize, Compare, Timeline, Count
- **Keyword Extraction**: Removes stopwords, extracts meaningful terms
- **Time Range Detection**: today, yesterday, last week/month/year
- **Sentiment Detection**: stressed, happy, frustrated, anxious, excited
- **File Type Detection**: pdf, docx, xlsx, image

### Hybrid Search
1. **Vector Search** (if embedding available)
   - Generates embedding for query text
   - KNN search on `context_vec` table
   - Returns semantically similar content
2. **Keyword Search**
   - LIKE queries on `indexed_documents` and `context_chunks`
   - Matches literal keywords
3. **Result Merging**
   - Deduplicates by ID
   - Sorts by relevance score
   - Returns match reasons

### Database Schema (Phase 2)
**Location**: `src-tauri/src/db.rs` (migrate_to_phase_2)

New tables:
- `query_cache` - Caches parsed queries for performance
- `query_history` - Tracks user queries for learning
- `chunk_sentiment` - Caches sentiment analysis results

### Frontend UI
**Location**: `src/components/NaturalLanguageSearch.tsx`

- Search input with example queries
- Intent icon display
- Filter badges (time range, file types)
- Result list with relevance scores
- Match reason display
- Execution time stats

### Tauri Command
**Location**: `src-tauri/src/lib.rs:3171`

```rust
#[tauri::command]
async fn natural_language_query(query_text: String) -> Result<serde_json::Value, String>
```

Returns:
- Parsed query info (keywords, intent, time_range, sentiment)
- Result items with relevance scores
- Execution time
- Summary (for summarize intent)

---

## Remaining for Full Phase 2

### Month 5: Entity + Temporal (Deferred)
- [ ] Local NER via `rust-bert` (privacy-first)
- [ ] Advanced temporal parsing with `chrono-english`
- [ ] Entity filters in SQL

### Month 6: Sentiment + Integration (Partially Complete)
- [x] Sentiment keyword detection (basic)
- [ ] Local sentiment model (DistilBERT)
- [x] Hybrid vector + SQL execution
- [x] Query history tracking (schema ready)

### Optional Enhancements
- [ ] LLM-powered query parsing (for complex queries)
- [ ] Query autocomplete suggestions
- [ ] Saved searches

---

## Verification

### Build Status
```bash
cargo check      # 0 warnings
cargo test       # 132 passed, 0 failed
npm run typecheck # Clean
```

### Manual Testing
1. Open app: `npm run tauri dev`
2. Find "Natural Language Search" panel
3. Try queries:
   - "files about rust"
   - "pdfs from last week"
   - "what did I work on yesterday"
   - "show me stressed about deadlines"

---

## Architecture Notes

### Query Flow
```
User Input → Parser → ParsedQuery → Executor → QueryResult
                ↓                       ↓
           Keywords               Vector Search
           Intent                 Keyword Search
           TimeRange             Merge & Rank
           Sentiment             Deduplicate
```

### Relevance Scoring
- Vector search: `1.0 - (distance / 2.0)` (L2 distance → similarity)
- Keyword search: `match_ratio * 0.6 + weight_factor * 0.4`
- Longer keywords weighted higher (more specific)

---

## Performance

- Query parsing: <10ms
- Vector search: <100ms (if API configured)
- Keyword search: <50ms
- Total target: <500ms (90th percentile)

---

## Dependencies

No new crates required for core functionality.

Future (for full Phase 2):
```toml
rust-bert = "0.22"     # Local NER + sentiment
tokenizers = "0.15"
chrono-english = "0.1" # "last month" → DateTime
```

---

## Quick Reference

### Files Modified
- `src-tauri/src/query/` - New module (4 files)
- `src-tauri/src/db.rs` - Phase 2 migration
- `src-tauri/src/lib.rs:3171` - Tauri command
- `src/components/NaturalLanguageSearch.tsx` - Frontend
- `src/App.tsx` - Integration

### Test Queries
```
"files about authentication"
"pdfs from last month"
"summarize my notes on rust"
"what did I work on last week"
"when was I stressed about deadlines"
```

---

## Next Steps

1. **Test with real data** - Index some documents and test queries
2. **Add NER** - Extract entities (people, projects) from queries
3. **Sentiment analysis** - Use local model for accurate sentiment
4. **Query learning** - Track which results users click

---

**Last Checkpoint**: 2026-02-05 - Core Query System Complete
**Next Milestone**: NER Integration (Month 5)
