#!/bin/bash
# Git hygiene monitor v2 — intelligent file categorization + stale claim detection
# Runs on UserPromptSubmit. Categorizes dirty files into 4 buckets:
#   1. Safe (build artifacts, screenshots, session state) — excluded from counts
#   2. Claimed (matched by a TERMINALS.md entry) — informational only
#   3. Needs attention (unclaimed, not safe) — the real problem
#   4. Stale claims (TERMINALS.md entries with zero matching files)
#
# Supports --json flag for sentinel consumption.

cd "$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0

TERMINALS_FILE=".claude/TERMINALS.md"
JSON_MODE=false
[[ "$1" == "--json" ]] && JSON_MODE=true

# ---------------------------------------------------------------------------
# 1. Get all dirty files (modified + untracked)
# ---------------------------------------------------------------------------
all_files=()
while IFS= read -r line; do
    [ -z "$line" ] && continue
    file="${line:3}"  # Strip git status prefix (3 chars: XY + space)
    all_files+=("$file")
done < <(git status --porcelain 2>/dev/null)

total=${#all_files[@]}
[ "$total" -eq 0 ] && {
    $JSON_MODE && echo '{"total":0,"safe_excluded":0,"needs_attention":0,"stale_done":[],"stale_empty":[],"unclaimed_files":[]}'
    exit 0
}

# ---------------------------------------------------------------------------
# 2. Safe pattern filter — artifacts that never need committing
# ---------------------------------------------------------------------------
is_safe() {
    case "$1" in
        # Build artifacts
        *.tsbuildinfo) return 0 ;;
        src-tauri/bindings/*) return 0 ;;

        # Runtime data (screenshots, tokens)
        data/screenshot-*.png) return 0 ;;
        data/signal_terminal_token.txt) return 0 ;;

        # Claude session infrastructure
        .claude/worktrees/*) return 0 ;;
        .claude/wisdom/*) return 0 ;;
        .claude/sessions/*) return 0 ;;
        .claude/knowledge/*) return 0 ;;
        .claude/backups/*) return 0 ;;
        .claude/analyzer-state.json) return 0 ;;
        .claude/memory.db) return 0 ;;

        # Internal Claude config
        .claude/settings.local.json) return 0 ;;
    esac
    return 1
}

safe_files=()
relevant_files=()
for file in "${all_files[@]}"; do
    if is_safe "$file"; then
        safe_files+=("$file")
    else
        relevant_files+=("$file")
    fi
done

safe_count=${#safe_files[@]}

# ---------------------------------------------------------------------------
# 3. Parse TERMINALS.md — extract claims per terminal
# ---------------------------------------------------------------------------
declare -A terminal_claims    # terminal_id -> "pattern1|pattern2|..."
declare -A terminal_status    # terminal_id -> "working|done|..."
declare -A terminal_claim_count  # terminal_id -> number of patterns

current_terminal=""
if [ -f "$TERMINALS_FILE" ]; then
    in_comment=false
    while IFS= read -r line; do
        # Track HTML comments
        [[ "$line" == *'<!--'* ]] && in_comment=true
        if $in_comment; then
            [[ "$line" == *'-->'* ]] && in_comment=false
            continue
        fi

        # Terminal header: ### T1 — description
        if [[ "$line" =~ ^###[[:space:]]+(T[0-9]+) ]]; then
            current_terminal="${BASH_REMATCH[1]}"
            terminal_claims[$current_terminal]=""
            terminal_status[$current_terminal]=""
            terminal_claim_count[$current_terminal]=0
        fi

        # Status line
        if [[ "$line" =~ ^\-[[:space:]]\*\*Status\*\*:[[:space:]]*(.*) ]] && [ -n "$current_terminal" ]; then
            terminal_status[$current_terminal]="${BASH_REMATCH[1]}"
        fi

        # Files line
        if [[ "$line" =~ ^\-[[:space:]]\*\*Files\*\*:[[:space:]]*(.*) ]] && [ -n "$current_terminal" ]; then
            raw_files="${BASH_REMATCH[1]}"
            IFS=',' read -ra patterns <<< "$raw_files"
            count=0
            for p in "${patterns[@]}"; do
                p=$(echo "$p" | sed 's/^ *//;s/ *$//')  # trim whitespace
                [ -z "$p" ] && continue
                if [ -n "${terminal_claims[$current_terminal]}" ]; then
                    terminal_claims[$current_terminal]="${terminal_claims[$current_terminal]}|$p"
                else
                    terminal_claims[$current_terminal]="$p"
                fi
                count=$((count + 1))
            done
            terminal_claim_count[$current_terminal]=$count
        fi
    done < "$TERMINALS_FILE"
fi

# ---------------------------------------------------------------------------
# 4. Categorize relevant files
# ---------------------------------------------------------------------------
claimed_files=()
unclaimed_files=()

for file in "${relevant_files[@]}"; do
    is_claimed=false
    for tid in "${!terminal_claims[@]}"; do
        IFS='|' read -ra patterns <<< "${terminal_claims[$tid]}"
        for pattern in "${patterns[@]}"; do
            case "$file" in
                $pattern) is_claimed=true; break 2 ;;
            esac
        done
    done

    if $is_claimed; then
        claimed_files+=("$file")
    else
        unclaimed_files+=("$file")
    fi
done

unclaimed_count=${#unclaimed_files[@]}
claimed_count=${#claimed_files[@]}

# ---------------------------------------------------------------------------
# 5. Detect stale claims
# ---------------------------------------------------------------------------
stale_done=()
stale_empty=()

for tid in "${!terminal_status[@]}"; do
    status="${terminal_status[$tid]}"

    # "done" entries that haven't been cleaned up
    if [[ "$status" == "done" ]]; then
        stale_done+=("$tid")
        continue
    fi

    # Working entries with zero matching files
    if [[ "$status" == "working" ]] && [ "${terminal_claim_count[$tid]:-0}" -gt 0 ]; then
        match_count=0
        IFS='|' read -ra patterns <<< "${terminal_claims[$tid]}"
        for pattern in "${patterns[@]}"; do
            for file in "${relevant_files[@]}"; do
                case "$file" in
                    $pattern) match_count=$((match_count + 1)); break ;;
                esac
            done
        done
        if [ "$match_count" -eq 0 ]; then
            stale_empty+=("$tid (${terminal_claim_count[$tid]} claimed, 0 match)")
        fi
    fi
done

# ---------------------------------------------------------------------------
# 6. Check commit lock
# ---------------------------------------------------------------------------
lock_msg=""
if [ -f "$TERMINALS_FILE" ]; then
    # Only detect lock in active terminal entries (outside comments and protocol section)
    lock_count=$(awk '
        /^## Active Terminals/,0 {
            if (/^<!--/) { in_comment=1 }
            if (in_comment && /-->/) { in_comment=0; next }
            if (in_comment) next
            if (/^## Protocol/ || /^[0-9]+\./) next
            if (/Commit Lock.*HELD/) count++
        }
        END { print count+0 }
    ' "$TERMINALS_FILE" 2>/dev/null)
    [ "$lock_count" -gt 0 ] && lock_msg="COMMIT LOCK ACTIVE — another terminal is committing. Wait for lock release."
fi

# ---------------------------------------------------------------------------
# 7. Output
# ---------------------------------------------------------------------------

if $JSON_MODE; then
    # JSON output for sentinel consumption
    unclaimed_json=$(printf '%s\n' "${unclaimed_files[@]}" | head -20 | \
        awk 'BEGIN{printf "["} NR>1{printf ","} {gsub(/"/,"\\\""); printf "\"%s\"",$0} END{printf "]"}')
    stale_done_json=$(printf '%s\n' "${stale_done[@]}" | \
        awk 'BEGIN{printf "["} NR>1{printf ","} {gsub(/"/,"\\\""); printf "\"%s\"",$0} END{printf "]"}')
    stale_empty_json=$(printf '%s\n' "${stale_empty[@]}" | \
        awk 'BEGIN{printf "["} NR>1{printf ","} {gsub(/"/,"\\\""); printf "\"%s\"",$0} END{printf "]"}')
    [ ${#unclaimed_files[@]} -eq 0 ] && unclaimed_json="[]"
    [ ${#stale_done[@]} -eq 0 ] && stale_done_json="[]"
    [ ${#stale_empty[@]} -eq 0 ] && stale_empty_json="[]"

    echo "{\"total\":$total,\"safe_excluded\":$safe_count,\"claimed\":$claimed_count,\"needs_attention\":$unclaimed_count,\"stale_done\":$stale_done_json,\"stale_empty\":$stale_empty_json,\"unclaimed_files\":$unclaimed_json}"
    exit 0
fi

# Human-readable output
msg=""

# Commit lock
[ -n "$lock_msg" ] && msg="${msg}${lock_msg}\n"

# Build directory-grouped file list
build_file_list() {
    local -n files_ref=$1
    local max_show=${2:-10}
    local count=0
    declare -A dir_groups

    for file in "${files_ref[@]}"; do
        dir=$(dirname "$file")
        # Collapse deep paths to 2 levels
        short_dir=$(echo "$dir" | cut -d/ -f1-2)
        base=$(basename "$file")
        dir_groups["$short_dir"]+="${base}, "
        count=$((count + 1))
        [ "$count" -ge "$max_show" ] && break
    done

    for dir in $(echo "${!dir_groups[@]}" | tr ' ' '\n' | sort); do
        items="${dir_groups[$dir]}"
        items="${items%, }"  # trim trailing comma
        item_count=$(echo "$items" | tr ',' '\n' | wc -w)
        echo "    ${dir}/ (${item_count}): ${items}"
    done

    if [ ${#files_ref[@]} -gt "$max_show" ]; then
        echo "    ... and $((${#files_ref[@]} - max_show)) more"
    fi
}

# Stale claim summary
stale_summary=""
if [ ${#stale_done[@]} -gt 0 ] || [ ${#stale_empty[@]} -gt 0 ]; then
    stale_parts=()
    for s in "${stale_done[@]}"; do stale_parts+=("$s (done)"); done
    for s in "${stale_empty[@]}"; do stale_parts+=("$s"); done
    stale_summary=$(IFS=', '; echo "${stale_parts[*]}")
fi

# Thresholds: 0=OK, 1-4=NOTICE, 5-9=WARNING, 10+=CRITICAL
if [ "$unclaimed_count" -ge 10 ]; then
    msg="${msg}GIT HYGIENE CRITICAL: ${unclaimed_count} unclaimed files — commit discipline breakdown\n"
    msg="${msg}$(build_file_list unclaimed_files 15)\n"
    [ -n "$stale_summary" ] && msg="${msg}  Stale claims: ${stale_summary}\n"
    msg="${msg}  Action: STOP feature work. Commit or discard these files. Clean TERMINALS.md.\n"
    msg="${msg}  Safe excluded: ${safe_count} | Claimed by terminals: ${claimed_count}"
elif [ "$unclaimed_count" -ge 5 ]; then
    msg="${msg}GIT HYGIENE WARNING: ${unclaimed_count} unclaimed files accumulating\n"
    msg="${msg}$(build_file_list unclaimed_files 10)\n"
    [ -n "$stale_summary" ] && msg="${msg}  Stale claims: ${stale_summary}\n"
    msg="${msg}  Action: commit per-directory batches or claim in TERMINALS.md"
elif [ "$unclaimed_count" -ge 1 ]; then
    file_list=$(printf '%s, ' "${unclaimed_files[@]}")
    file_list="${file_list%, }"
    msg="${msg}GIT HYGIENE NOTICE: ${unclaimed_count} unclaimed file(s): ${file_list}"
elif [ "$claimed_count" -gt 0 ]; then
    msg="${msg}GIT STATUS: ${claimed_count} files claimed by terminals, ${safe_count} safe artifacts excluded."
fi

# Always mention stale claims even if unclaimed count is low
if [ "$unclaimed_count" -lt 5 ] && [ -n "$stale_summary" ]; then
    msg="${msg}\n  Stale TERMINALS.md entries: ${stale_summary}"
fi

[ -n "$msg" ] && echo -e "$msg"
