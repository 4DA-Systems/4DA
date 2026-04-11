# GEP Alphabet (v1.0.0) — 4DA Quick Reference

Full reference: [`D:\runyourempire\glyph\docs\ALPHABET.md`](../../../runyourempire/glyph/docs/ALPHABET.md).
This is the short version for day-to-day 4DA use.

## Cheat sheet

### Source (where did this come from?)
| Glyph | Name | 4DA usage example |
|-------|------|-------------------|
| 🌐 | web | HN/RSS scrape, web search result |
| 📰 | news | Editorial news feed |
| 💬 | forum | HN comment, Reddit thread, Discord |
| 📂 | file | Local project artifact, scanned file |
| 🧠 | inference | Agent synthesis without direct observation |
| 🔬 | research | arXiv, peer-reviewed literature |
| 👤 | user | Direct user input |
| 🤖 | agent | Another 4DA agent's output |
| 📡 | sensor | Telemetry, metrics, ops-state.json |
| 📜 | archive | Decision log, AWE wisdom, memory |

### Confidence (how certain?)
| Glyph | Name | When to use |
|-------|------|-------------|
| ◌ | unverified | Single weak signal, likely a guess |
| ◐ | partial | Some support, not corroborated |
| ◑ | corroborated | Multiple independent sources agree |
| ● | high | Strong cross-signal evidence |
| ◉ | verified | Independently tested or authoritative |

### Action (what does this envelope assert or request?)
| Glyph | Name | When to use |
|-------|------|-------------|
| ➜ | implies | "This means we should do X" |
| ⇄ | correlates | "This relates to Y" |
| ⊕ | adds | "This adds context to the prior claim" |
| ⊗ | contradicts | "This conflicts with the prior claim" |
| ▶ | executes | "Do this action now" |
| ⏸ | defers | "Hold for later" |
| ✋ | asks-human | "Requires user decision" (triggers Gate 6) |
| ✓ | confirms | "Validates the prior claim" |
| ✗ | refutes | "Invalidates the prior claim" |
| ⚡ | triggers | "Fires a downstream event" |
| 🔍 | investigates | "Needs deeper research" |
| ↻ | retries | "Retry with new context" |

### Reversibility (can this be undone?)
| Glyph | Name | Gates fired |
|-------|------|-------------|
| ∞ | idempotent | — (safe, no gates) |
| ↶ | reversible | — (clean undo path) |
| ⟲ | partially-reversible | **Gate 5 — AWE consequence scan** |
| 🔒 | irreversible | **Gate 5 + Gate 6 — AWE scan + human ACK** |

### Domain (subject area)
| Glyph | Name |
|-------|------|
| ⚙ | infra |
| 🎨 | design |
| 📊 | data |
| 🔐 | security ← triggers security-review routing |
| 💡 | idea |
| ⚖ | legal ← triggers security-review routing |
| 💰 | financial ← triggers security-review routing |
| 🧪 | experimental |
| 📖 | docs |
| 🧩 | integration |
| 🎯 | strategy |
| 🐛 | bug |
| ⚗ | research-domain |
| 👥 | community |
| 🏗 | architecture |

### Risk (severity)
| Glyph | Name | Severity | Gates fired |
|-------|------|----------|-------------|
| 🟢 | ok | 0 | — |
| 🟡 | caution | 1 | — |
| 🟠 | warning | 2 | **Gate 5 — AWE consequence scan** |
| 🔴 | alert | 3 | **Gate 5 + Gate 6** |
| ⬛ | blocked | 4 | **Gate 6 — human ACK** |
| ⚠ | flag | 1 | — |

### Connectors (chain-link)
| Glyph | Name |
|-------|------|
| ∴ | therefore |
| ∵ | because |
| ⇔ | equivalent |

### Protocol meta
| Glyph | Name |
|-------|------|
| ⊙ | session-start |
| ⊚ | session-end |
| ✅ | ack |
| ❌ | nack |
| ⏱ | heartbeat |

## Typical 4DA envelopes

**Gotcha detector catches Vite/fourda.exe issue:**
```
⟦🌐·◉·➜·⚙·⟲·🟡⟧
Vite dependency update detected while fourda.exe is running.
Restart before next route load.
⟦id:… ts:… agent:gotcha-detector v:1.0.0⟧
```

**Security expert flags a secret leak:**
```
⟦📂·●·⚡·🔐·🔒·🔴⟧
Found API key in src/sources/github.rs:142 — commit history shows it's been there for 3 commits.
⟦id:… ts:… agent:4da-security-expert v:1.0.0⟧
```
→ Routes through Gate 5 (AWE scan) + Gate 6 (human ACK) because of `🔒` and `🔴`.

**Scoring expert corroborates a relevance score:**
```
⟦🧠·◑·⊕·📊·∞·🟢⟧
PASIFA V2 gate pass rate for HN content is 0.67 this week, consistent with prior 4 weeks.
⟦id:… ts:… agent:4da-scoring-expert v:1.0.0⟧
```
→ Pure read, no gates fire. Fully safe.

**War room makes a strategic decision:**
```
⟦👤·●·▶·🎯·🔒·🟠⟧
We will launch with Signal tier only. Team/Enterprise deferred to post-launch.
⟦id:… ts:… agent:4da-war-room v:1.0.0⟧
```
→ Gates 5 + 6 fire (irreversible, warning). Routes through AWE + human ACK.
