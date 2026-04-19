#!/usr/bin/env bash
# Compute the SHA-256 of the current SSL.com CodeSignTool release and
# write it into .github/workflows/release.yml in place of the
# PLACEHOLDER_SHA256_FILL_IN sentinel. Also flips the commented-out
# verification `if` to active.
#
# Why this exists: SSL.com ships CodeSignTool over HTTPS but does not
# publish a companion checksum — the safe posture is to pin the hash
# ourselves, snapshot the binary, and update the pin whenever we
# intentionally upgrade. This script is the minimal-friction way to
# do that: download → hash → substitute → print diff.
#
# Usage:
#   ./scripts/pin-codesigntool-sha.sh        # compute + apply
#   ./scripts/pin-codesigntool-sha.sh --dry  # compute + print only
#
# Exits non-zero on any failure so CI can call this safely.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

WORKFLOW=".github/workflows/release.yml"
URL="https://www.ssl.com/download/codesigntool-for-windows/"
TMP_ZIP="$(mktemp -t codesigntool-XXXX.zip)"
trap 'rm -f "$TMP_ZIP"' EXIT

DRY_RUN=false
[[ "${1:-}" == "--dry" ]] && DRY_RUN=true

echo "Downloading CodeSignTool from $URL ..."
if command -v curl > /dev/null 2>&1; then
    curl -fsSL -o "$TMP_ZIP" "$URL"
elif command -v wget > /dev/null 2>&1; then
    wget -qO "$TMP_ZIP" "$URL"
else
    echo "ERROR: need curl or wget" >&2
    exit 1
fi

echo "Computing SHA-256 ..."
if command -v sha256sum > /dev/null 2>&1; then
    SHA=$(sha256sum "$TMP_ZIP" | awk '{print $1}')
elif command -v shasum > /dev/null 2>&1; then
    SHA=$(shasum -a 256 "$TMP_ZIP" | awk '{print $1}')
else
    echo "ERROR: need sha256sum or shasum" >&2
    exit 1
fi

# Lowercase. PowerShell's Get-FileHash returns uppercase; the workflow
# normalizes with ToLowerInvariant(). Pin the lowercase form.
SHA_LC=$(echo "$SHA" | tr '[:upper:]' '[:lower:]')

echo ""
echo "CodeSignTool SHA-256 (lowercase, ready to pin):"
echo "  $SHA_LC"
echo ""

if [ "$DRY_RUN" = true ]; then
    echo "(--dry mode: workflow unchanged)"
    exit 0
fi

if ! grep -q "PLACEHOLDER_SHA256_FILL_IN" "$WORKFLOW"; then
    echo "WARNING: $WORKFLOW no longer has a PLACEHOLDER_SHA256_FILL_IN sentinel."
    echo "         It may already be pinned. If you want to update the pin,"
    echo "         edit the workflow manually."
    exit 0
fi

# Replace placeholder + un-comment the verification `if`.
# Use a portable sed invocation (works on Linux + macOS + Git Bash).
sed -i.bak \
    -e "s|\$expected = \"PLACEHOLDER_SHA256_FILL_IN\"|\$expected = \"$SHA_LC\"|" \
    -e 's|# if (\$expected -ne "PLACEHOLDER_SHA256_FILL_IN" -and |if (|' \
    "$WORKFLOW"
rm -f "$WORKFLOW.bak"

echo "Pinned. Diff:"
git diff "$WORKFLOW" | head -30

echo ""
echo "Next: git add $WORKFLOW && git commit -m \"ci(release): pin CodeSignTool SHA-256 ($SHA_LC)\""
