#!/bin/bash
# Git hygiene monitor — terminal-aware commit discipline enforcement
# Runs on UserPromptSubmit to catch buildup early
#
# Reads TERMINALS.md to understand which files are claimed by terminals.
# Only warns about UNCLAIMED files — claimed files belong to their terminal.

cd "$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0

TERMINALS_FILE=".claude/TERMINALS.md"

# Get all changed files (exclude .claude/ internals — those are session artifacts)
all_changed=$(git status --porcelain 2>/dev/null | grep -v '^\?\? \.claude/' | grep -v ' \.claude/' | sed 's/^...//')
total_count=$(echo "$all_changed" | grep -c . 2>/dev/null || echo "0")

[ "$total_count" -eq 0 ] && exit 0

# Count net lines changed
line_stat=$(git diff --shortstat 2>/dev/null)
insertions=$(echo "$line_stat" | sed -n 's/.*\([0-9][0-9]*\) insertion.*/\1/p')
deletions=$(echo "$line_stat" | sed -n 's/.*\([0-9][0-9]*\) deletion.*/\1/p')
total_lines=$(( ${insertions:-0} + ${deletions:-0} ))

# Parse claimed file patterns from TERMINALS.md (outside HTML comments only)
claimed_patterns=""
if [ -f "$TERMINALS_FILE" ]; then
    claimed_patterns=$(awk '
        /^## Active Terminals/,0 {
            if (/^<!--/) { in_comment=1 }
            if (in_comment && /-->/) { in_comment=0; next }
            if (in_comment) next
            if (/^\- \*\*Files\*\*:/) {
                sub(/.*Files\*\*: */, "")
                gsub(/, */, "\n")
                print
            }
        }
    ' "$TERMINALS_FILE" 2>/dev/null)
fi

# Check for active commit lock
lock_active=false
if [ -f "$TERMINALS_FILE" ]; then
    lock_count=$(awk '
        /^## Active Terminals/,0 {
            if (/^<!--/) { in_comment=1 }
            if (in_comment && /-->/) { in_comment=0; next }
            if (in_comment) next
            if (/Commit Lock.*HELD/) count++
        }
        END { print count+0 }
    ' "$TERMINALS_FILE" 2>/dev/null)
    [ "$lock_count" -gt 0 ] && lock_active=true
fi

# Categorize files
unclaimed_count=0
claimed_count=0

for file in $all_changed; do
    is_claimed=false
    if [ -n "$claimed_patterns" ]; then
        while IFS= read -r pattern; do
            [ -z "$pattern" ] && continue
            case "$file" in
                $pattern) is_claimed=true; break ;;
            esac
        done <<< "$claimed_patterns"
    fi

    if [ "$is_claimed" = true ]; then
        claimed_count=$((claimed_count + 1))
    else
        unclaimed_count=$((unclaimed_count + 1))
    fi
done

# --- Build message ---
msg=""

# Commit lock
if [ "$lock_active" = true ]; then
    msg="${msg}COMMIT LOCK ACTIVE — another terminal is committing. Wait for lock release before committing.\n"
fi

# Unclaimed file warnings (these are the dangerous ones)
if [ "$unclaimed_count" -ge 15 ]; then
    msg="${msg}GIT HYGIENE CRITICAL: ${unclaimed_count} UNCLAIMED files uncommitted (~${total_lines} lines). These aren't claimed in TERMINALS.md — any terminal could commit them with a wrong message. Commit NOW or claim them."
elif [ "$unclaimed_count" -ge 8 ]; then
    msg="${msg}GIT HYGIENE WARNING: ${unclaimed_count} unclaimed uncommitted files (~${total_lines} lines). Claim in TERMINALS.md or commit to prevent cross-terminal contamination."
elif [ "$unclaimed_count" -ge 3 ]; then
    msg="${msg}GIT HYGIENE NOTICE: ${unclaimed_count} unclaimed files uncommitted. Claim in TERMINALS.md or commit when ready."
fi

# Quiet status when all files are properly claimed
if [ "$unclaimed_count" -eq 0 ] && [ "$claimed_count" -gt 0 ] && [ "$total_count" -ge 5 ]; then
    msg="${msg}GIT STATUS: ${claimed_count} uncommitted files, all claimed by terminals in TERMINALS.md."
fi

[ -n "$msg" ] && echo -e "$msg"
