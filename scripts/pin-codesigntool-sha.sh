#!/usr/bin/env bash
# Compute the SHA-256 of the current SSL.com CodeSignTool release and
# refresh the pinned hash in .github/workflows/release.yml.
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

if ! grep -Eq '\$expected = "[0-9a-fA-F]{64}"' "$WORKFLOW"; then
    echo "ERROR: could not find the pinned CodeSignTool SHA-256 in $WORKFLOW" >&2
    echo "       Expected a line like: \$expected = \"<64 hex chars>\"" >&2
    exit 1
fi

# Replace the existing pinned hash.
# Use a portable sed invocation (works on Linux + macOS + Git Bash).
sed -i.bak \
    -E "s|\\\$expected = \"[0-9a-fA-F]{64}\"|\\\$expected = \"$SHA_LC\"|" \
    "$WORKFLOW"
rm -f "$WORKFLOW.bak"

echo "Pinned. Diff:"
git diff "$WORKFLOW" | head -30

echo ""
echo "Next: git add $WORKFLOW && git commit -m \"ci(release): pin CodeSignTool SHA-256 ($SHA_LC)\""
