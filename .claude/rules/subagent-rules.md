# Subagent Rules

Spawn a subagent when ANY of these are true:

1. **3+ files** to modify → implementer subagent
2. **Searching** for unknown code location → explorer subagent
3. **Debugging** with logs/traces or build failures → debugger subagent
4. **Running tests** with potential failures → test subagent
5. **Reviewing** changes across multiple files → reviewer subagent

**Don't use subagent for:** single file edits, quick lookups, tasks already understood from current context.

## Multi-Wave Operations (Large Batches)

When running multiple parallel agents that modify different file sets:

### Isolation Rule
- Use `isolation: "worktree"` for ANY agent that modifies 5+ files
- Worktree agents get their own copy of the repo — zero cross-contamination
- Changes return as a branch that the orchestrator merges with a clean commit message
- Non-worktree agents edit the shared working tree (small batches only)

### Sequential Commit Rule
- **NEVER launch Wave N+1 agents until Wave N is committed**
- After wave agents complete: verify → commit with wave-specific message → next wave
- This prevents accumulation of 50+ uncommitted files across waves
- Exception: waves with ZERO file overlap can run in parallel IF the orchestrator
  commits each wave's results immediately upon completion (first-done, first-committed)

### Agent File Claims
- Before launching agents, the orchestrating terminal MUST claim ALL files
  that will be modified in `.claude/TERMINALS.md`
- Group claims by wave: `Wave 1: file1.rs, file2.rs, file3.rs`
- After committing a wave, update claims to remove committed files

### Commit Message Discipline
- Each wave gets its own commit with a message describing THAT wave's work
- Never bundle multiple waves into one commit
- Format: `Wave description — specific changes summary`
