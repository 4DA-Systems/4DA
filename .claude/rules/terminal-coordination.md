# Terminal Coordination Rule

Multiple Claude Code terminals run simultaneously on this repo.

## Before ANY file edits:
1. Read `.claude/TERMINALS.md`
2. Check no other terminal claims the files you need
3. Add your claim with the files you'll modify
4. Only then start editing

## Commit Lock Protocol
- Only ONE terminal may commit at a time
- Before committing: add `**Commit Lock**: HELD` to your terminal entry
- After committing: remove the lock and your claim
- If another terminal holds the lock: WAIT — do not stage or commit
- The lock prevents cross-terminal commit contamination

## Commit-Per-Wave Rule
- **NEVER accumulate more than one wave of changes before committing**
- After each logical batch completes and passes verification, commit immediately
- Pattern: Wave N agents finish → verify → commit → Wave N+1 agents start
- The git hygiene hook warning at 20+ files means you've already waited too long
- Each commit gets its own descriptive message matching the actual work done

## After committing:
1. Remove your claim from `.claude/TERMINALS.md`
2. Release the Commit Lock
3. If working tree has changes from OTHER terminals, do NOT revert them

## If you see uncommitted changes you didn't make:
- Another terminal is actively working. Do NOT `git checkout`, `git stash`, or revert those files.
- Only touch files you've claimed in TERMINALS.md.

## Forbidden without explicit user request:
- `git checkout HEAD -- <file>` on files you didn't modify
- `git stash` (affects all terminals)
- `git reset` (affects all terminals)
- Deleting or moving files not in your claim
- Committing files not in your claim (even if they show up in `git status`)
