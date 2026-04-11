# GEP Safety Gates — 4DA Perspective

The canonical document lives at
[`D:\runyourempire\glyph\docs\SAFETY.md`](../../../runyourempire/glyph/docs/SAFETY.md).
This file describes how each gate integrates with 4DA specifically.

## Why 9 gates, non-negotiable

GEP is a shared communication protocol across autonomous agents. Any
protocol powerful enough to coordinate agents is also powerful enough
to create new failure modes — specifically:

- **Steganography** — agents hiding reasoning in symbol patterns
- **Capability creep** — agents using glyphs they weren't authorised for
- **Opaque audit trails** — machine-only logs
- **Silent drift** — dictionary changes that quietly change meaning
- **Irreversible action without consequence review**

The 9 gates exist to close each of these failure modes. Removing any
one re-opens a mode that cannot be closed by the others.

## The gates, mapped to 4DA infrastructure

### Gate 1 — Roundtrip invariant
**Enforced by:** `glyph-core::Dictionary::from_toml_str` at load time +
`glyph-safety::roundtrip::check` at envelope time.

**4DA wiring:** none needed. The glyph crate self-enforces.

### Gate 2 — Payload invariant (no glyphs in payload)
**Enforced by:** `glyph-safety::payload::check`.

**4DA wiring:** the MCP envelope wrapper refuses to transmit any
envelope whose payload contains dictionary glyphs.

### Gate 3 — Anti-steganography monitor
**Enforced by:** `glyph-safety::anti_steg::AntiStegMonitor` (stateful).

**4DA wiring:** one monitor instance per 4DA session, shared by all
agents. Status surfaced in the Ops Dashboard. Flagged agents generate
a Wisdom Gate alert.

**Measurement baseline:** starts from `anti_steg_baseline` in
`gep-v1.0.0.toml`. After Phase 2 measurement, 4DA recalibrates the
baseline using actual traffic and publishes an updated dictionary
(v1.1.0) if needed.

### Gate 4 — Capability declaration
**Enforced by:** `glyph-safety::capability::check`.

**4DA wiring:** each agent in `.claude/agents/` grows a companion file
`.glyph-caps.toml` listing which glyphs it's authorised to emit. The
capability registry is loaded at 4DA startup.

**Example manifest:**

```toml
agent_id = "gotcha-detector"

[[emits]]
position = "source"
glyph = "🌐"

[[emits]]
position = "confidence"
glyph = "◉"

# ... etc
```

The broker refuses any envelope whose glyphs aren't in the manifest.

### Gate 5 — Reversibility gate
**Enforced by:** `glyph-safety::reversibility::check`.

**4DA wiring:** the broker supplies an `AweConsequenceHook`
implementation in `glyph_integration::wisdom_bridge`. It bridges
reversibility-gated envelopes (`⟲` partially-reversible, `🔒`
irreversible) to `mcp__awe__awe_consequence_scan`. The existing AWE
infrastructure does the work.

**Existing integration:** AWE is already running as an MCP server with
7 tools. The glyph bridge is just a new caller.

### Gate 6 — Mandatory human ACK
**Enforced by:** `glyph-safety::human_ack::check`.

**4DA wiring:** `glyph_integration::wisdom_bridge::WisdomGateAckProvider`
bridges to the existing Wisdom Gate 2 UI. When an envelope with `🔒`,
`🔴`, `⬛`, or `✋` arrives, the Wisdom Gate surfaces a modal and
records the user's decision.

**Integration with existing hooks:** the wisdom-gate hook in
`.claude/hooks/pre-tool-use.cjs` already handles destructive-action
warnings. The glyph version routes through the same UI.

### Gate 7 — Dual-form audit log
**Enforced by:** `glyph-safety::audit::AuditSink` trait +
`glyph_integration::audit_log::SqliteAuditSink`.

**4DA wiring:** new table `glyph_audit` in `data/4da.db`. Schema in
[`GEP-INTEGRATION.md`](GEP-INTEGRATION.md#phase-2-schema).

**Retention:** unlimited for elevated entries (`🔒`, `🔐`, `⚖`, `💰`,
`⬛`, `🔴`). 90 days for standard entries. Configurable per deployment.

### Gate 8 — Dictionary version gate
**Enforced by:** `glyph-safety::version::check`.

**4DA wiring:** the dictionary version is pinned at 4DA build time.
Upgrading requires an explicit `pnpm run migrate:glyph-dict` step that
re-verifies all gates before switching.

### Gate 9 — Semantic drift detector
**Enforced by:** `glyph-safety::drift::DriftDetector`.

**4DA wiring:** weekly scheduled task in `glyph_integration::routing`
that scans `glyph_audit` for drift. Drifted entries surface in the
Compound Intelligence dashboard alongside `compound_advantage` metrics.

## The validation pipeline in 4DA

```
4DA agent → emits envelope → glyph_integration::mcp_envelope
                                     │
                                     ▼
                           glyph_safety::validate(env, dict, ctx)
                                     │
   ┌─────────────────────────────────┼─────────────────────────────────┐
   │                                                                   │
Gate 1 (roundtrip)                                               Gate 3 (anti-steg, non-blocking)
Gate 2 (payload)                                                 Gate 7 (audit log, always-on)
Gate 4 (capability)
Gate 8 (version)
   │                                                                   │
   ▼                                                                   │
Gate 5 (reversibility) → mcp__awe__awe_consequence_scan ───────────────┤
   │                                                                   │
   ▼                                                                   │
Gate 6 (human ACK)    → Wisdom Gate UI modal ──────────────────────────┤
   │                                                                   │
   ▼                                                                   │
ValidationReport ──────────────────────────────────────────────────────┘
   │
   ▼
Broker routes to downstream agent (or rejects with forensic audit entry)
```

Gate 7 always runs. Even rejected envelopes get audit entries so we can
forensically review attempted misuse.

## What the 9 gates do NOT cover

Deliberately out of scope:

- **Rate limiting** — belongs in the broker / MCP layer
- **PII redaction** — context-specific, implemented per sink
- **Input sanitization** — payloads are free-form NL by design
- **Cross-envelope causality** — chain links carry structure, but no gate
  enforces "every consequence has a cause"

These are 4DA-level policy concerns, not protocol-level safety concerns.
