# Worktree Hygiene

Subagents spawned with `isolation: "worktree"` create a new worktree under `.claude/worktrees/agent-<hash>/` and a matching branch `worktree-agent-<hash>`. After the subagent's commits are merged into main, the worktree directory and branch remain — neither the subagent nor the orchestrator cleans up. Over time these accumulate and trigger sentinel alarms.

## Prevention

`node scripts/cleanup-orphaned-worktrees.cjs` — dry-run by default, shows what would be removed. Add `--execute` to apply. Safe by design: refuses to remove any branch whose tip is NOT reachable from main, or any worktree with uncommitted changes. Reflog preserves everything for 90 days. Suggested cadence: run nightly or via a pre-push hook.
