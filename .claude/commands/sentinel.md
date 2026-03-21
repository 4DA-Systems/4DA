---
description: "Run autonomous bug detection and deploy expert team for any issues found"
allowed-tools: Agent, Read, Glob, Grep, Bash
---

# Sentinel — Autonomous Bug Detection

Run a full sentinel scan and deploy the expert team for any critical issues found.

## Process

1. **Run the sentinel scanner:**
   ```bash
   node scripts/sentinel-scan.cjs
   ```

2. **Read the results:** `.claude/wisdom/sentinel-state.json`

3. **For each CRITICAL signal:**
   - Read the recommended expert's spec from `.claude/agents/[expert].md`
   - Read the expert's knowledge manifest from `.claude/knowledge/[domain].md`
   - Spawn the expert as a `general-purpose` agent with the signal details as context
   - Collect the expert's findings

4. **For each WARNING signal:**
   - Note it for the user but don't deploy an expert unless asked

5. **Synthesize results:**
   - Present all expert findings in a unified report
   - If any expert found a fixable issue, propose the fix
   - If an expert fixed something, recommend immune system activation for antibody creation

6. **Update state:** The scanner automatically writes to `sentinel-state.json`

## Output Format

```
SENTINEL SCAN COMPLETE

CRITICAL: [count]
  [domain] — [issue] → [expert deployed] → [finding]

WARNINGS: [count]
  [domain] — [issue]

ALL CLEAR: [count] checks passed

Recommendations:
  - [actionable next steps]
```

## Expert Team Reference

| Expert | Spec | Knowledge |
|--------|------|-----------|
| Rust Systems | `4da-rust-expert.md` | `rust-systems.md` |
| React UI | `4da-react-expert.md` | `react-ui.md` |
| Data Layer | `4da-data-expert.md` | `data-layer.md` |
| IPC Bridge | `4da-ipc-expert.md` | `ipc-contracts.md` |
| Scoring & ML | `4da-scoring-expert.md` | `scoring-ml.md` |
| Security | `4da-security-expert.md` | `security-surface.md` |
