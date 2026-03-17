# AWE ↔ 4DA Context Integration Plan

## Objective
Inject AWE wisdom (principles, anti-patterns, decision patterns) into 4DA's
context system so PASIFA scoring is informed by the user's decision history.

## Three Integration Points (minimal, surgical)

### 1. AWE Wisdom as Context Chunks
Inject validated principles and anti-patterns as weighted context chunks.
These flow through the EXISTING scoring pipeline — no new scoring logic needed.

### 2. AWE Status in Context Panel
Show wisdom health (decisions, principles, calibration) in the UI sidebar.

### 3. AWE Scan on Context Index
When context is re-indexed, also scan git repos for new decisions.

## Implementation: 4DA Side

### A. New Tauri command: `sync_awe_wisdom`
File: src-tauri/src/commands/context_commands.rs
- Calls AWE CLI: `awe wisdom --domain software-engineering --json`
- Parses principles and anti-patterns
- Embeds each as a context chunk with source_file = "awe://wisdom"
- Weight = 1.5 (higher than regular context, wisdom is more valuable)

### B. Context panel AWE status
File: src/components/context-panel.tsx
- Add small AWE status line below "N files indexed"
- Calls AWE status via MCP or reads from cached state
- Shows: "AWE: N decisions, M principles"

### C. Auto-scan on index
File: src-tauri/src/commands/context_commands.rs
- In index_context(), after indexing files, call `awe scan --repo <project_dir>`
- Detects new decision-shaped commits automatically
