#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# 4DA Version Bump
# Updates version in package.json, tauri.conf.json, and Cargo.toml
# Usage: ./scripts/bump-version.sh 1.0.1
# ─────────────────────────────────────────────────────────────────────────────

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

if [ $# -ne 1 ]; then
  echo "Usage: $0 <new-version>"
  echo "Example: $0 1.0.1"
  exit 1
fi

NEW_VERSION="$1"

# Validate version format (semver)
if ! echo "$NEW_VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'; then
  echo "Error: Version must be semver format (e.g., 1.0.1 or 1.1.0-beta.1)"
  exit 1
fi

echo "Bumping version to $NEW_VERSION..."

# 1. package.json
node -e "
  const fs = require('fs');
  const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
  pkg.version = '$NEW_VERSION';
  fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
"
echo "  [OK] package.json"

# 2. tauri.conf.json
node -e "
  const fs = require('fs');
  const conf = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
  conf.version = '$NEW_VERSION';
  fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(conf, null, 2) + '\n');
"
echo "  [OK] tauri.conf.json"

# 3. Cargo.toml
sed -i "s/^version = \".*\"/version = \"$NEW_VERSION\"/" src-tauri/Cargo.toml
echo "  [OK] Cargo.toml"

# 4. Verify consistency
CARGO_VER=$(grep '^version' src-tauri/Cargo.toml | head -1 | cut -d'"' -f2)
TAURI_VER=$(node -e "process.stdout.write(require('./src-tauri/tauri.conf.json').version)" 2>/dev/null)
PKG_VER=$(node -e "process.stdout.write(require('./package.json').version)" 2>/dev/null)

if [ "$CARGO_VER" = "$NEW_VERSION" ] && [ "$TAURI_VER" = "$NEW_VERSION" ] && [ "$PKG_VER" = "$NEW_VERSION" ]; then
  echo ""
  echo "All versions updated to $NEW_VERSION"
  echo ""
  echo "Next steps:"
  echo "  1. Update docs/RELEASE-NOTES-v${NEW_VERSION}.md"
  echo "  2. git add -A && git commit -m 'Bump version to ${NEW_VERSION}'"
  echo "  3. git tag v${NEW_VERSION}"
  echo "  4. git push origin main --tags"
  echo "  5. GitHub Actions will build, sign, and release automatically"
else
  echo ""
  echo "ERROR: Version mismatch after update!"
  echo "  Cargo.toml:      $CARGO_VER"
  echo "  tauri.conf.json: $TAURI_VER"
  echo "  package.json:    $PKG_VER"
  exit 1
fi
