#!/bin/bash
# Install the local pre-push gate — WITHOUT clobbering an existing hook system.
#
# The gate LOGIC lives in the tracked `.githooks/pre-push`. This script wires a thin DELEGATING pre-push hook
# into whatever hook directory the repo ALREADY uses (Husky `.husky/`, a custom `core.hooksPath`, or the default
# `.git/hooks`), and CHAINS any pre-push already there. It never changes which directory is active, so existing
# hooks (pre-commit lint/fmt, Husky's CADE checks, etc.) keep working. Idempotent. Uninstall: remove the
# "local gate" block from the active pre-push (printed below).
set -e
ROOT="$(git rev-parse --show-toplevel)"
GATE_REL=".githooks/pre-push"
[ -f "$ROOT/$GATE_REL" ] || { echo "[local-gate] no $GATE_REL in this repo — nothing to install"; exit 1; }

# Pick the ACTIVE hook directory (do NOT change it).
cur="$(git config --get core.hooksPath || true)"
if [ -d "$ROOT/.husky" ]; then
  HOOKDIR="$ROOT/.husky"; KIND="husky"
elif [ -n "$cur" ]; then
  case "$cur" in /*|[A-Za-z]:*) HOOKDIR="$cur" ;; *) HOOKDIR="$ROOT/$cur" ;; esac  # relative → repo-rooted
  KIND="custom ($cur)"
else
  HOOKDIR="$ROOT/.git/hooks"; KIND="default"
fi
mkdir -p "$HOOKDIR"
HOOK="$HOOKDIR/pre-push"
MARK="# >>> local gate >>>"
END="# <<< local gate <<<"

# The delegate block. STDIN IS THE WHOLE PROBLEM HERE.
#
# git hands a pre-push hook the pushed refs on stdin, and stdin can be consumed
# exactly once. When this block is CHAINED onto an existing hook, that hook has
# almost always already drained stdin in its own `while read` loop (4DA's
# .husky/pre-push does, to scan the push range for secrets). The delegate then
# received an empty stdin, .githooks/pre-push's `[ -n "$PUSH_REFS" ]` guard was
# false, and GATE 1 — the committed-tree coherence check, the entire reason the
# gate exists — never ran, while the hook printed success. Silently installing a
# gate that cannot fire is worse than not installing one.
#
# So the block FORWARDS the refs explicitly instead of hoping stdin survived:
# a host that captured them exports GATE_PUSH_REFS (see .husky/pre-push) and we
# pipe them in; a host that did not, or a freshly created hook where this block
# is the first thing to read stdin, falls through to the plain call and stdin
# still works. Both shapes are covered, neither silently degrades.
# Unquoted heredoc: `$GATE_REL` is substituted NOW, `\$` survives to run time.
DELEGATE="$(cat <<GATE_BLOCK
if [ -n "\${GATE_PUSH_REFS:-}" ]; then
  echo "\$GATE_PUSH_REFS" | bash "\$(git rev-parse --show-toplevel)/$GATE_REL" "\$@" || exit 1
else
  bash "\$(git rev-parse --show-toplevel)/$GATE_REL" "\$@" || exit 1
fi
GATE_BLOCK
)"

if [ -f "$HOOK" ] && grep -qF "$MARK" "$HOOK"; then
  # An OLDER installation is already present. Do not stop here — every machine
  # that ran this script before carries the stdin-losing single-line delegate,
  # and an installer that refuses to upgrade its own broken output leaves the
  # fix stranded in the repo. Compare, and rewrite the block IN PLACE if stale.
  installed="$(awk -v s="$MARK" -v e="$END" 'index($0,s){f=1;next} index($0,e){f=0} f' "$HOOK")"
  if [ "$installed" = "$DELEGATE" ]; then
    echo "[local-gate] already installed and up to date in $KIND hook ($HOOK)."
    exit 0
  fi
  TMP="$(mktemp)"
  awk -v s="$MARK" -v e="$END" -v repl="$DELEGATE" '
    index($0,s) { print; print repl; skip=1; next }
    index($0,e) { skip=0 }
    !skip       { print }
  ' "$HOOK" > "$TMP"
  cat "$TMP" > "$HOOK"   # preserve the file (and its mode/inode), not just the content
  rm -f "$TMP"
  chmod +x "$HOOK" 2>/dev/null || true
  echo "[local-gate] UPGRADED the stale gate block in the $KIND pre-push ($HOOK)."
  echo "  the previous block did not forward the pushed refs, so GATE 1 never ran."
  exit 0
fi

if [ -f "$HOOK" ]; then
  # CHAIN: keep the existing pre-push, append our delegating block.
  { echo ""; echo "$MARK"; echo "$DELEGATE"; echo "$END"; } >> "$HOOK"
  echo "[local-gate] CHAINED the gate onto the existing $KIND pre-push ($HOOK)."
else
  # No existing pre-push: create one (with a shebang for non-husky dirs).
  { [ "$KIND" = husky ] || echo "#!/bin/bash"; echo "$MARK"; echo "$DELEGATE"; echo "$END"; } > "$HOOK"
  echo "[local-gate] installed a new $KIND pre-push ($HOOK)."
fi
chmod +x "$HOOK" 2>/dev/null || true
echo "  gate spec: tools/gate-jobs.json   ·   bypass: git push --no-verify (or SKIP_GATE=1)"
echo "  remove:    delete the '$MARK ... $END' block from $HOOK"
