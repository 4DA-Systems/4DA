# 4DA Expert Team — Agent Relationship Map

## Expert Team (New)

| Agent | Domain | Invocation |
|-------|--------|-----------|
| `4da-chief` | Issue routing, triage, multi-expert coordination | `/expert <issue>` or direct |
| `4da-rust-expert` | Backend Rust — async, lifetimes, compilation, modules | Via Chief or direct |
| `4da-react-expert` | Frontend React — components, hooks, state, i18n, a11y | Via Chief or direct |
| `4da-data-expert` | Database — SQLite, sqlite-vec, migrations, queries | Via Chief or direct |
| `4da-ipc-expert` | IPC boundary — Tauri commands, ghost commands, serialization | Via Chief or direct |
| `4da-scoring-expert` | PASIFA scoring, embeddings, ACE, relevance, calibration | Via Chief or direct |
| `4da-security-expert` | Cross-cutting security, invariants, privacy, attack surface | Via Chief or direct |

## Relationship to Existing Agents

### Superseded (use Expert Team instead)

| Old Agent | Superseded By | Reason |
|-----------|--------------|--------|
| `4da-source-debugger` | **Rust Systems Expert** | Source debugging is a subset of Rust backend investigation. Expert reads knowledge manifests for current source info. |
| `4da-relevance-debugger` | **Scoring & ML Expert** | Relevance debugging is the core use case for the scoring expert, with deeper pipeline knowledge. |
| `4da-contract-validator` | **IPC Bridge Expert** | Contract validation is now automated via manifests + pre-commit. Expert handles the investigation side. |

### Complementary (use alongside Expert Team)

| Existing Agent | Relationship | When to Use |
|----------------|-------------|-------------|
| `4da-quality-gate` | **Gatekeeper** — runs mechanically before commits | Use for fast pass/fail checks. Security Expert does deep analysis. |
| `4da-immune-system` | **Post-fix** — creates antibodies after bug fixes | Use AFTER an expert fixes a bug to prevent recurrence. |
| `4da-war-room` | **Escalation target** — Chief escalates here when experts can't resolve | Use for multi-system crises that span 3+ domains. |
| `4da-ops-conductor` | **Operations** — sovereignty score, cadences, compound metrics | Orthogonal to expert team. Manages overall system health. |
| `4da-drift-detector` | **Strategic** — detects architectural drift | Complements Security Expert's invariant checking. |

### Independent (no overlap)

| Existing Agent | Purpose |
|----------------|---------|
| `4da-changelog-writer` | Release documentation |
| `4da-digest-enhancer` | Content digest quality |
| `4da-docs-generator` | Documentation generation |
| `4da-explain-enhancer` | Explanation quality |
| `4da-insight-synthesizer` | Content synthesis |
| `4da-knowledge-mapper` | Knowledge graph |
| `4da-mcp-server-dev` | MCP server development |
| `4da-mcp-tester` | MCP server testing |
| `4da-test-generator` | Test creation |
| `4da-trend-analyzer` | Trend analysis |
| `4da-frontend-splitter` | Component splitting |
| `4da-config-validator` | Config validation |
| `4da-context-optimizer` | Context window optimization |
| `4da-decision-replay` | Decision review |

## How to Invoke

```
# Route through Chief (recommended for unclear issues)
/expert <issue description>

# Direct expert (when you know the domain)
Read D:/4DA/.claude/agents/4da-[domain]-expert.md and investigate: <issue>
```

## Knowledge System

All experts read auto-generated manifests from `.claude/knowledge/`:
- Regenerated at session start via hook (if stale >1 hour)
- Manual refresh: `pnpm run generate:knowledge`
- Ghost command gate: pre-commit hook runs `pnpm run validate:commands`
