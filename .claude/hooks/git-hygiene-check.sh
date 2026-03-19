#!/bin/bash
# Git hygiene monitor — terminal-aware commit discipline enforcement
# Runs on UserPromptSubmit to catch buildup early
#
# Reads TERMINALS.md to understand which files belong to which terminal,
# so it only warns about YOUR uncommitted files, not other terminals'.

cd "$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0

TERMINALS_FILE=".claude/TERMINALS.md"

# Get all changed files (modified + untracked, excluding .claude/ internals)
all_changed=$(git status --porcelain 2>/dev/null | grep -v '^\?\? \.claude/' | sed 's/^...//')
total_count=$(echo "$all_changed" | grep -c . 2>/dev/null || echo "0")

# If nothing changed, exit silently
[ "$total_count" -eq 0 ] && exit 0

# Count net lines changed
line_stat=$(git diff --shortstat 2>/dev/null)
insertions=$(echo "$line_stat" | sed -n 's/.*\([0-9][0-9]*\) insertion.*/\1/p')
deletions=$(echo "$line_stat" | sed -n 's/.*\([0-9][0-9]*\) deletion.*/\1/p')
total_lines=$(( ${insertions:-0} + ${deletions:-0} ))

# Parse TERMINALS.md to find files claimed by active terminals
# Only look at lines AFTER "## Active Terminals" and skip HTML comments
claimed_patterns=""
if [ -f "$TERMINALS_FILE" ]; then
    claimed_patterns=$(awk '
        /^## Active Terminals/,0 {
            # Skip comment blocks
            if (/^<!--/) { in_comment=1 }
            if (in_comment && /-->/) { in_comment=0; next }
            if (in_comment) next
            # Extract file lists from "- **Files**:" lines
            if (/^\- \*\*Files\*\*:/) {
                sub(/.*Files\*\*: */, "")
                gsub(/, */, "\n")
                print
            }
        }
    ' "$TERMINALS_FILE" 2>/dev/null)
fi

# Check for active commit lock (outside comments only)
lock_active=false
if [ -f "$TERMINALS_FILE" ]; then
    lock_count=$(awk '
        /^## Active Terminals/,0 {
            if (/^<!--/) { in_comment=1 }
            if (in_comment && /-->/) { in_comment=0; next }
            if (in_comment) next
            if (/Commit Lock.*HELD/) print
        }
    ' "$TERMINALS_FILE" 2>/dev/null | wc -l | tr -d ' ')
    [ "$lock_count" -gt 0 ] && lock_active=true
fi

# Categorize changed files as claimed or unclaimed
unclaimed_count=0
claimed_count=0

for file in $all_changed; do
    is_claimed=false

    if [ -n "$claimed_patterns" ]; then
        while IFS= read -r pattern; do
            [ -z "$pattern" ] && continue
            # Handle glob patterns: src/locales/* matches src/locales/en.json
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

# --- Build the message ---
msg=""

# Commit lock warning
if [ "$lock_active" = true ]; then
    msg="${msg}COMMIT LOCK ACTIVE — another terminal is committing. Do NOT commit or stage files until the lock is released.\n"
fi

# Tiered warnings for UNCLAIMED files only
if [ "$unclaimed_count" -ge 20 ]; then
    msg="${msg}GIT HYGIENE WARNING: ${unclaimed_count} UNCLAIMED uncommitted files (~${total_lines} lines). These aren't claimed in TERMINALS.md — commit NOW or claim them to prevent cross-terminal contamination."
elif [ "$unclaimed_count" -ge 10 ]; then
    msg="${msg}GIT HYGIENE NOTICE: ${unclaimed_count} unclaimed uncommitted files (~${total_lines} lines). Claim in TERMINALS.md or commit to avoid another terminal scooping them up."
elif [ "$unclaimed_count" -ge 5 ] && [ "$total_lines" -ge 100 ]; then
    msg="${msg}GIT HYGIENE NOTE: ${unclaimed_count} unclaimed files (~${total_lines} lines) uncommitted. Consider claiming in TERMINALS.md or committing."
fi

# Quiet info when everything is properly claimed
if [ "$unclaimed_count" -eq 0 ] && [ "$claimed_count" -gt 0 ] && [ "$total_count" -ge 8 ]; then
    msg="${msg}GIT STATUS: ${claimed_count} uncommitted files, all claimed by terminals in TERMINALS.md. No action needed from this terminal."
fi

# Output
if [ -n "$msg" ]; then
    echo -e "$msg"
fi
