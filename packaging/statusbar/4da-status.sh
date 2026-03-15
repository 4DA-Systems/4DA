#!/bin/bash
# Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
# Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.
#
# 4DA status module for waybar/polybar/i3status
#
# Outputs a single-line status string suitable for status bar modules.
# Uses sqlite3 directly — no dependency on the 4da binary.
#
# Usage:
#   waybar:  Add as custom module (see waybar-config.json)
#   polybar: custom/script module with `exec = /path/to/4da-status.sh`
#   i3status: pipe through i3status-rs or similar wrapper
#
# Environment:
#   FOURDA_DB_PATH — override database path
#   XDG_DATA_HOME  — respected for default db location (defaults to ~/.local/share)

set -euo pipefail

# Resolve database path
if [ -n "${FOURDA_DB_PATH:-}" ] && [ -f "${FOURDA_DB_PATH}" ]; then
    DB_PATH="${FOURDA_DB_PATH}"
else
    DB_PATH="${XDG_DATA_HOME:-$HOME/.local/share}/4da/data/4da.db"
fi

# Check if database exists
if [ ! -f "${DB_PATH}" ]; then
    echo '{"text": "4DA: --", "tooltip": "Database not found", "class": "disconnected"}'
    exit 0
fi

# Check if sqlite3 is available
if ! command -v sqlite3 &>/dev/null; then
    echo '{"text": "4DA: ??", "tooltip": "sqlite3 not installed", "class": "error"}'
    exit 0
fi

# Query signal count from the most recent analysis
# Count items from the last 24 hours that match signal patterns
SIGNAL_COUNT=$(sqlite3 "${DB_PATH}" "
    SELECT COUNT(*)
    FROM source_items
    WHERE created_at >= datetime('now', '-24 hours');
" 2>/dev/null || echo "0")

# Get the last fetch timestamp
LAST_FETCH=$(sqlite3 "${DB_PATH}" "
    SELECT COALESCE(
        strftime('%H:%M', MAX(created_at), 'localtime'),
        'never'
    )
    FROM source_items;
" 2>/dev/null || echo "never")

# Get briefing status
BRIEFING_AGE=$(sqlite3 "${DB_PATH}" "
    SELECT COALESCE(
        CAST((julianday('now') - julianday(MAX(created_at))) * 24 AS INTEGER),
        -1
    )
    FROM briefings;
" 2>/dev/null || echo "-1")

# Build tooltip
if [ "${BRIEFING_AGE}" -ge 0 ]; then
    TOOLTIP="${SIGNAL_COUNT} items (24h) | Last fetch: ${LAST_FETCH} | Briefing: ${BRIEFING_AGE}h ago"
else
    TOOLTIP="${SIGNAL_COUNT} items (24h) | Last fetch: ${LAST_FETCH} | No briefing"
fi

# Determine CSS class based on signal count
if [ "${SIGNAL_COUNT}" -gt 50 ]; then
    CLASS="high"
elif [ "${SIGNAL_COUNT}" -gt 10 ]; then
    CLASS="normal"
elif [ "${SIGNAL_COUNT}" -gt 0 ]; then
    CLASS="low"
else
    CLASS="idle"
fi

# Output JSON for waybar (also parseable by other bars)
# Plain text: extract with jq '.text' or just pipe through cut
echo "{\"text\": \"4DA: ${SIGNAL_COUNT} signals\", \"tooltip\": \"${TOOLTIP}\", \"class\": \"${CLASS}\"}"
