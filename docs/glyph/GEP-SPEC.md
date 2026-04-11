# Glyph Envelope Protocol — 4DA Reference

This is the 4DA-facing overview. The canonical specification lives in
the glyph repo at [`D:\runyourempire\glyph\docs\SPEC.md`](../../../runyourempire/glyph/docs/SPEC.md).

## What GEP is, for 4DA

A **typed semantic envelope** that wraps every inter-agent message with
a fixed 6-glyph header:

- **Source** — where the information came from (web, news, file,
  inference, user, agent, sensor, archive, research, forum)
- **Confidence** — how certain the claim is (unverified → verified)
- **Action** — what the envelope asserts or requests (implies,
  executes, asks-human, etc.)
- **Domain** — subject area (infra, security, legal, financial, bug,
  architecture, …)
- **Reversibility** — can the action be undone (idempotent →
  irreversible)
- **Risk** — severity / attention level (ok → blocked)

The payload remains plain natural language. The glyphs are compressed
*metadata*, not compressed *content*.

```
⟦🌐·◉·➜·⚙·⟲·🟡⟧
Vite dependency update detected while fourda.exe is running.
Restart before next route load.
⟦id:a3f1-… ts:2026-04-11T10:00:00Z agent:gotcha-detector v:1.0.0⟧
```

Every header glyph has a canonical, deterministic natural-language
expansion. The example above compiles to:

> *[web] [verified] [implies] in domain [infra] with reversibility [partially-reversible] and risk [caution].*

## Why 4DA wants this

Three concrete wins:

1. **Machine-parseable routing.** The broker decides which agent gets
   a message by reading 6 glyphs, not parsing English.
2. **Dual-form audit log.** Every routed envelope is stored in both
   wire and compiled-NL forms. Forensic review never sees opaque
   output.
3. **Safety gate composition.** Destructive or high-stakes envelopes
   route through AWE consequence scans and human ACK prompts *before*
   the downstream agent sees them. The gates plug into 4DA's existing
   Wisdom Gate infrastructure.

## Not ML, not compression

GEP does *not* try to make agents "think in glyphs." LLM agents
continue to reason in natural language. Glyphs exist at the envelope
layer — above the reasoning, above the payload. This is the
**strong-form** design choice. See
[`SAFETY.md`](GEP-SAFETY.md) for why the weak form is dangerous.

## Where GEP lives

- **Library code:** `D:\runyourempire\glyph` (standalone repo, Apache 2.0)
- **Spec:** `D:\runyourempire\glyph\docs\SPEC.md` (CC-BY-4.0)
- **4DA integration:** `src-tauri/src/glyph_integration/` (Phase 2 follow-up)
- **Audit log DB:** `glyph_audit` table in `data/4da.db` (Phase 2)

## Phasing (how we roll it out)

```
Phase 0 — Spec + tokenizer measurement      ← done (this session)
Phase 1 — Rust crate with 30+ tests         ← done (this session)
Phase 2 — Audit-only mode in 4DA            ← next commit
Phase 3 — One opt-in agent emits envelopes  ← later
Phase 4 — Broker routing by glyph           ← later
Phase 5 — Safety hardening (AWE, UI bridge) ← later
Phase 6 — Compound measurement + feedback   ← later
```

Each phase has an explicit **kill gate** — see
[`GEP-INTEGRATION.md`](GEP-INTEGRATION.md).
