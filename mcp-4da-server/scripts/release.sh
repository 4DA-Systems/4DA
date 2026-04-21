#!/usr/bin/env bash
# Release @4da/mcp-server — bump version, build, test, tag, push.
# CI publishes to npm automatically when the tag lands on GitHub.
#
# Usage:
#   ./scripts/release.sh patch    # 4.1.0 → 4.1.1
#   ./scripts/release.sh minor    # 4.1.0 → 4.2.0
#   ./scripts/release.sh major    # 4.1.0 → 5.0.0
#   ./scripts/release.sh 4.2.0    # explicit version
#
# Prerequisites:
#   - NPM_TOKEN secret set in GitHub repo settings (granular, publish-only)
#   - Working tree clean in mcp-4da-server/

set -euo pipefail

cd "$(dirname "$0")/.."

# ── Argument parsing ─────────────────────────────────────────────────────────

BUMP="${1:-}"
if [ -z "$BUMP" ]; then
  echo "Usage: ./scripts/release.sh <patch|minor|major|X.Y.Z>"
  exit 1
fi

CURRENT=$(node -p "require('./package.json').version")

case "$BUMP" in
  patch|minor|major)
    IFS='.' read -r MAJ MIN PAT <<< "$CURRENT"
    case "$BUMP" in
      patch) PAT=$((PAT + 1)) ;;
      minor) MIN=$((MIN + 1)); PAT=0 ;;
      major) MAJ=$((MAJ + 1)); MIN=0; PAT=0 ;;
    esac
    NEW_VERSION="${MAJ}.${MIN}.${PAT}"
    ;;
  *)
    NEW_VERSION="$BUMP"
    ;;
esac

echo "╔══════════════════════════════════════════╗"
echo "║  @4da/mcp-server release                ║"
echo "║  $CURRENT → $NEW_VERSION"
echo "╚══════════════════════════════════════════╝"
echo ""

# ── Pre-flight checks ────────────────────────────────────────────────────────

echo "[1/6] Pre-flight checks..."
if ! git diff --quiet -- . ; then
  echo "ERROR: mcp-4da-server/ has uncommitted changes. Commit first."
  exit 1
fi

# ── Version bump ─────────────────────────────────────────────────────────────

echo "[2/6] Bumping version to $NEW_VERSION..."

# package.json version
node -e "
const fs = require('fs');
const pkg = JSON.parse(fs.readFileSync('package.json', 'utf-8'));
pkg.version = '$NEW_VERSION';
fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
"

# Sync version in src/index.ts (Server constructor + help text + console.error)
sed -i "s/version: \"$CURRENT\"/version: \"$NEW_VERSION\"/g" src/index.ts
sed -i "s/@4da\/mcp-server $CURRENT/@4da\/mcp-server $NEW_VERSION/g" src/index.ts
sed -i "s/4DA MCP Server v$CURRENT/4DA MCP Server v$NEW_VERSION/g" src/index.ts

# ── Build & test ─────────────────────────────────────────────────────────────

echo "[3/6] Building..."
pnpm run build

echo "[4/6] Testing..."
pnpm test

# ── Commit & tag ─────────────────────────────────────────────────────────────

echo "[5/6] Committing version bump..."
cd ..
git add mcp-4da-server/package.json mcp-4da-server/src/index.ts
git commit -m "release(mcp): @4da/mcp-server v$NEW_VERSION"

echo "[6/6] Tagging mcp-v$NEW_VERSION..."
git tag "mcp-v$NEW_VERSION"

echo ""
echo "════════════════════════════════════════════"
echo "  Ready to publish. Run:"
echo ""
echo "    git push origin main mcp-v$NEW_VERSION"
echo ""
echo "  CI will build, test, and publish to npm."
echo "════════════════════════════════════════════"
