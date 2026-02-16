# System Invariants
## What Must ALWAYS or NEVER Happen in 4DA

**Version:** 1.0.0
**Source:** Extracted from ACE-STONE-TABLET.md and ARCHITECTURE.md
**Authority:** These are non-negotiable constraints. Violating an invariant is a critical bug.

---

## The ACE Guarantees (Canonical)

These are the five inviolable guarantees from the ACE specification:

### INV-001: ACE Always Hits Its Mark
- **Precision MUST be >85%** or an alert MUST be triggered
- Every relevance decision MUST be explainable
- Confidence scores MUST accurately reflect actual certainty
- **Violation Detection:** Track precision metrics, alert on degradation

### INV-002: ACE Never Requires User Input
- System MUST work from first launch with zero configuration
- User input MUST enhance results but MUST NOT be required
- Basic functionality MUST be available without any setup
- **Violation Detection:** Test cold start scenario, verify output without config

### INV-003: ACE Never Fails Silently
- ALL errors MUST be logged with full context
- Graceful degradation MUST be preferred over crashes
- Health status MUST always be visible/queryable
- **Violation Detection:** Error handler coverage, health endpoint tests

### INV-004: ACE Respects Privacy Absolutely
- NO data leaves the machine without explicit user consent
- Activity tracking MUST be OFF by default
- User MUST be able to delete ALL data at any time
- **Violation Detection:** Network audit, data flow tracing

### INV-005: ACE Learns But Doesn't Creep
- User MUST always understand why items are shown
- NO unexplainable "magic" recommendations
- Learning signals MUST be transparent and inspectable
- **Violation Detection:** Explanation generation for all recommendations

---

## Performance Invariants

### INV-010: Latency Bounds
- Context lookup MUST complete in <100ms
- Recovery from any single failure MUST complete in <5s
- UI MUST remain responsive during background operations
- **Verification:** Performance benchmarks in CI

### INV-011: Memory Bounds
- ACE overhead MUST NOT exceed 100MB
- No unbounded growth in any data structure
- **Verification:** Memory profiling, leak detection

### INV-012: Cold Start Performance
- System MUST provide useful results within 5 user interactions
- Initial scan MUST complete within reasonable time for typical project sizes
- **Verification:** Cold start test suite

---

## Data Integrity Invariants

### INV-020: Confidence Thresholds
- Signals with confidence <0.3 MUST be rejected (not stored)
- No unvalidated data may enter the interest model
- **Code Pattern:**
```rust
if confidence < 0.3 {
    return None;  // MANDATORY rejection
}
```

### INV-021: Idempotent Database Writes
- All database write operations MUST be idempotent
- Duplicate requests MUST NOT corrupt state
- **Verification:** Replay tests, concurrent write tests

### INV-022: Embedding Consistency
- Same input text MUST always produce same embedding
- Embedding model changes MUST trigger full re-embedding
- **Verification:** Determinism tests

### INV-023: Three-Layer Context Weights
- Static Identity weight: 1.0 (explicit user input)
- Active Context weight: 0.8 (real-time detection)
- Learned Behavior weight: 0.6 (implicit learning)
- These weights are CANONICAL and MUST NOT be changed without spec update
- **Code Pattern:**
```rust
const STATIC_LAYER_WEIGHT: f32 = 1.0;
const ACTIVE_LAYER_WEIGHT: f32 = 0.8;
const LEARNED_LAYER_WEIGHT: f32 = 0.6;
```

---

## Security Invariants

### INV-030: API Keys Never Logged
- API keys MUST NEVER appear in logs, errors, or debug output
- Credential fields MUST be redacted in all serialization
- **Verification:** Log audit, grep for key patterns

### INV-031: BYOK Integrity
- User API keys MUST be stored locally only
- NO transmission of API keys to any remote service (except the intended API)
- Users own their keys entirely
- **Verification:** Network traffic analysis

### INV-032: Local-First Architecture
- Core functionality MUST work completely offline (with Ollama)
- External API calls MUST gracefully degrade when unavailable
- **Verification:** Offline mode test suite

---

## Architectural Invariants

### INV-040: Tauri IPC Boundary
- All Rust↔Frontend communication MUST go through Tauri IPC
- No direct file system access from frontend
- Commands MUST be typed and validated
- **Verification:** IPC audit, type coverage

### INV-041: SQLite as Single Source of Truth
- All persistent state MUST live in SQLite database
- No state split across multiple storage mechanisms
- **Verification:** State audit

### INV-042: Error Handling Hierarchy
- Use `thiserror` for all custom error types
- Errors MUST propagate context (not just messages)
- **Code Pattern:**
```rust
#[derive(Error, Debug)]
pub enum MyError {
    #[error("Failed to {action}: {source}")]
    Contextual {
        action: String,
        #[source] source: SomeError,
    },
}
```

---

## UI/UX Invariants

### INV-050: Matte Black Theme
- Primary background: #0A0A0A
- These colors are CANONICAL design system values
- Gold accent (#D4AF37) used sparingly for highlights only
- **Verification:** Visual regression tests

### INV-051: Accessible Contrast
- All text MUST meet WCAG AA contrast ratios
- Interactive elements MUST be clearly visible
- **Verification:** Accessibility audit

---

## Exclusion Strength Invariants

### INV-060: Exclusion Application
- Absolute exclusion: Score = 0 (NEVER show)
- Hard exclusion: Score reduced by 90%
- Soft exclusion: Score reduced by 50%
- These percentages are CANONICAL
- **Code Pattern:**
```rust
match self.strength {
    ExclusionStrength::Absolute => 0.0,
    ExclusionStrength::Hard => base_score * 0.1,
    ExclusionStrength::Soft => base_score * 0.5,
}
```

---

## Behavioral Invariants

### INV-070: Temporal Decay
- Learned behavior has 30-day half-life
- Active context decays over 7 days
- **Code Pattern:**
```rust
// 30-day half-life for learned behavior
let decay = 0.5_f32.powf(days_since / 30.0);
```

### INV-071: Minimum Data for Learning
- Topic affinity MUST have ≥5 exposures before contributing
- No learning from insufficient data
- **Code Pattern:**
```rust
if self.total_exposures < 5 {
    return 0.0;  // Not enough data
}
```

---

## Validation Invariants

### INV-080: Multi-Source Confidence Boost
- Single source: base confidence
- Two sources agreeing: +10% confidence
- Three sources agreeing: +20% confidence (cap at 0.95 for inferred)
- Explicit user input: confidence = 1.0 (always wins)
- **Verification:** Cross-validation tests

### INV-090: File Size Limits
- New TypeScript/TSX files MUST stay under 500 lines
- New Rust files MUST stay under 1000 lines
- Files approaching limits (TS: 350, RS: 600) trigger warnings
- Files exceeding limits MUST be split or added to exception list with justification
- **Exception list:** `scripts/check-file-sizes.cjs` EXCEPTIONS constant
- **Enforcement:** Pre-commit hook, CI pipeline, `pnpm run validate:sizes`

---

## How to Use This File

1. **Before modifying code:** Check if any invariants apply to the area you're touching
2. **During code review:** Verify no invariants are violated
3. **After incidents:** Check which invariant was violated, add detection
4. **When adding features:** Consider if new invariants are needed

---

## Invariant Violation Protocol

If you discover an invariant violation:

1. **STOP** - Do not proceed with the current task
2. **DOCUMENT** - Record the violation and how it was discovered
3. **ASSESS** - Determine impact and scope
4. **FIX** - Create minimal fix that restores the invariant
5. **VERIFY** - Add test to prevent regression
6. **REPORT** - Update FAILURE_MODES.md if this was a new failure pattern

---

*These invariants are extracted from the canonical specifications. Changes require spec updates first.*
