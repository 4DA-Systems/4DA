#!/bin/bash
# Install the the external verifier local pre-push gate for THIS repo. Idempotent; run once per clone.
# Sets core.hooksPath=.githooks (applies to the repo AND all its git worktrees — they share .git/config).
# The hook + gate spec are tracked (.githooks/pre-push, .external-verifier/gate.json), so every worktree on a commit that
# has them will gate. Uninstall: git config --unset core.hooksPath.
set -e
ROOT="$(git rev-parse --show-toplevel)"
git -C "$ROOT" config core.hooksPath .githooks
echo "[external-verifier-gate] installed: core.hooksPath=.githooks (this repo + all worktrees)."
echo "  spec:    .external-verifier/gate.json"
echo "  bypass:  git push --no-verify   (or EXTVERIFIER_SKIP_GATE=1)"
echo "  remove:  git -C \"$ROOT\" config --unset core.hooksPath"
