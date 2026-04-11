# GEP Integration Plan — 4DA

Canonical integration doc: [`D:\runyourempire\glyph\docs\INTEGRATION.md`](../../../runyourempire/glyph/docs/INTEGRATION.md).
This file tracks the 4DA-specific rollout and kill gates.

## Phase status

| Phase | Scope | Status | Kill gate |
|-------|-------|--------|-----------|
| 0 | Spec + real Anthropic tokenizer measurement | ✅ Done — **CONDITIONAL PASS** | Avg ≤ 3 tokens/glyph (batch amortised: 2.53 — PASS). Header vs NL: 28 vs 30 tokens = 6.7% savings — compression claim DOWNGRADED to "parity" |
| 1 | Rust crate, 35 tests passing | ✅ Done | Any red test → quarantine |
| 1.5 | Phase 2 integration harness (SqliteAuditSink, mocks, demo) | ✅ Done at `runyourempire/glyph/crates/glyph-integration-harness` | — |
| 2 | Audit-only mode (4DA Rust integration) | ⏳ Follow-up | If Phase 2 measures <30% categorical glyph coverage OR excessive audit log growth → deprecate |
| 3 | First opt-in agent (`gotcha-detector`) | Future | No measurable routing improvement → stop expansion |
| 4 | Broker routing by glyph | Future | Routing accuracy regressed → rollback |
| 5 | Anti-steg + drift hardening | Future | False positive rate >5% → retune or disable |
| 6 | Compound measurement + AWE feedback | Future | None — steady state |

### Phase 0 real measurement (2026-04-12, claude-opus-4-6)

**Important: the original compression claim from the offline proxy was wrong.**
Real Anthropic tokenizer measurement:

- Batch amortised: **2.53 tokens/glyph** (passes ≤3 threshold)
- Per-message baseline-corrected avg: 3.52 tokens/glyph
- Real 6-glyph header: **28 tokens**
- Real NL metadata equivalent: **30 tokens**
- Savings vs NL: **6.7% — essentially parity**

**The compression pitch is officially downgraded.** GEP is a typed
routing protocol with composable safety gates, not a token-savings
technology. When talking about GEP internally or externally, lead with:

1. **Typed routing** — brokers make decisions without LLM parsing
2. **Composable safety gates** — reversibility, human-ack, anti-steg, drift
3. **Dual-form audit** — wire + compiled NL always stored
4. **Capability enforcement** — per-agent manifest gating

Full measurement dataset: `D:\runyourempire\glyph\dictionary\token-measurement.json`

## Phase 2 — next commit (details)

### New files in 4DA

```
src-tauri/src/glyph_integration/
├── mod.rs                  # public API + feature-flag gate
├── mcp_envelope.rs         # shadows MCP tool calls with GEP envelopes
├── audit_log.rs            # SqliteAuditSink implementation
├── routing.rs              # Phase 4 stub (returns NotImplemented for now)
└── wisdom_bridge.rs        # Phase 3+ stubs (NoopHook / NoopAckProvider for Phase 2)
```

### Phase 2 schema

New SQLite migration — likely `migrations/054_glyph_audit.sql`
(number depends on T-PRELAUNCH-HARDENING landing first):

```sql
CREATE TABLE glyph_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    envelope_id TEXT NOT NULL,
    agent TEXT NOT NULL,
    logged_at TEXT NOT NULL,
    summary TEXT NOT NULL,
    compiled_nl TEXT NOT NULL,
    header_glyphs TEXT NOT NULL,
    verdict TEXT NOT NULL,
    level TEXT NOT NULL,
    payload_bytes INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_glyph_audit_agent ON glyph_audit(agent);
CREATE INDEX idx_glyph_audit_level ON glyph_audit(level);
CREATE INDEX idx_glyph_audit_logged_at ON glyph_audit(logged_at);
```

### Phase 2 Cargo changes

```toml
# src-tauri/Cargo.toml
[dependencies]
glyph-core = { path = "../../runyourempire/glyph/crates/glyph-core" }
glyph-engine = { path = "../../runyourempire/glyph/crates/glyph-engine" }
glyph-compile = { path = "../../runyourempire/glyph/crates/glyph-compile" }
glyph-lift = { path = "../../runyourempire/glyph/crates/glyph-lift" }
glyph-safety = { path = "../../runyourempire/glyph/crates/glyph-safety" }

[features]
default = []
glyph_audit = []  # audit-only mode; off by default
```

With the feature off, `glyph_integration` compiles but emits nothing.
With the feature on, it writes shadow envelopes for each MCP tool call.

### Phase 2 trait impls to write

1. **`SqliteAuditSink`** implementing `glyph_safety::AuditSink`
   - writes to `glyph_audit` table
   - retention policy: 90 days standard, unlimited elevated
   - indexed by agent, level, timestamp

2. **`WisdomGateAckProvider`** implementing `glyph_safety::HumanAckProvider`
   - Phase 2: returns `NotRequired` (audit-only mode)
   - Phase 5: bridges to the real Wisdom Gate UI

3. **`AweMcpHook`** implementing `glyph_safety::AweConsequenceHook`
   - Phase 2: returns `NoScanNeeded`
   - Phase 5: calls `mcp__awe__awe_consequence_scan` via the MCP bridge

### Phase 2 MCP middleware

`glyph_integration::mcp_envelope::wrap_tool_call(tool_name, input, output) -> AuditEntry`

Called from the MCP tool-call dispatcher. It:

1. Lifts the tool metadata to an envelope using
   `glyph_lift::lift(..., LiftPolicy::Defaulted, ...)`
2. Runs `glyph_safety::validate` with the Phase 2 noop hooks
3. Writes the audit entry to SQLite
4. Returns without modifying the tool's behaviour

The wrapper is **non-destructive** — if lift fails or validate rejects,
the tool call still completes normally. The shadow envelope is
measurement infrastructure, not behaviour modification.

## Phase 2 kill-gate measurements

After Phase 2 has run against real 4DA agent traffic for one week:

1. **Envelope rate** — expected 1k-10k/day
2. **Compression ratio** — target ≥30% on metadata tokens vs NL equivalent
3. **Categorical coverage** — target ≥50% of dictionary glyphs in use
4. **Per-gate rejection rate** — should be <1% for pure gates (else dict is wrong)
5. **Audit log growth** — target <100MB/month at typical rates
6. **Query latency** — 90th percentile <10ms for `recent_for_agent`

**Abandon criteria:** if any of (2), (3), (5) fails, Phase 3 does not
start. The integration is reverted and the glyph crate stays as
reference material only.

**Continue criteria:** if all pass, Phase 3 proceeds with
`gotcha-detector` as the first opt-in agent.

## Phase 3 — first opt-in agent

**Target:** `gotcha-detector` (source: `.claude/agents/4da-gotcha-detector.md`)

**Why this one:**
- Narrow, well-typed input/output (catches specific technical issues)
- Low frequency (not thousands of emissions/day)
- Easy to eyeball correctness — can manually verify envelope quality
- Failure is low-impact — wrong glyph on a gotcha doesn't cause harm

**Scope:**
- Agent emits a real envelope alongside its NL output
- Capability manifest lives at `.claude/agents/4da-gotcha-detector.glyph-caps.toml`
- Broker routes the envelope but does not yet gate on it
- Audit log records the dual form

**Success signal:**
- Gotcha-detector envelopes route to the correct downstream agents
  ≥ 90% of the time (measured against human review)
- No envelopes rejected by pure gates
- Anti-steg monitor shows `Ok` or `Warming` (never `Flagged`)

## Phase 4+ — deferred

Phase 4-6 happen only after Phase 3 validates the core idea. Design is
documented in the canonical integration doc (link at top).

## Why we built Phase 0-1 standalone

During this session, multiple 4DA terminals were active:

- **T-WAR-ROOM** was committing source-reliability + AWE wisdom changes
- **T-PRELAUNCH-HARDENING** was modifying `src-tauri/src/db/migrations.rs`

Building glyph crates inside 4DA would have risked conflicts on
migrations.rs (for the Phase 2 migration file) and Cargo.toml. By
building the standalone glyph repo first and adding 4DA docs only,
this session is:

- **Zero-conflict** with in-flight 4DA work
- **Fully reversible** — the glyph repo is independent; abandoning it
  touches no 4DA code
- **Architecturally cleaner** — the glyph crate is a general library,
  not a 4DA-coupled module

Phase 2 (the Rust integration in 4DA) is a follow-up commit once the
concurrent terminals release their claims.
