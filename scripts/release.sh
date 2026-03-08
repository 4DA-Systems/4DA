#!/usr/bin/env bash
set -euo pipefail

VERSION="1.0.0"
REPO="runyourempire/4DA"

echo "=== 4DA Release v${VERSION} ==="

# Step 1: Verify clean state
echo "[1/6] Checking git state..."
if [ -n "$(git status --porcelain)" ]; then
  echo "ERROR: Working directory not clean. Commit or stash changes first."
  exit 1
fi

# Step 2: Run full test suite
echo "[2/6] Running tests..."
cd src-tauri && cargo test --lib 2>&1 | tail -3
cd ..
pnpm run test -- --run 2>&1 | tail -3

# Step 3: Build release
echo "[3/6] Building release..."
pnpm run tauri build 2>&1 | tail -5

# Step 4: Check installer exists
INSTALLER="src-tauri/target/release/bundle/nsis/4DA Home_${VERSION}_x64-setup.exe"
if [ ! -f "$INSTALLER" ]; then
  echo "ERROR: Installer not found at: $INSTALLER"
  exit 1
fi
echo "Installer: $(ls -lh "$INSTALLER" | awk '{print $5}')"

# Step 5: Verify versions match
echo "[4/6] Verifying version consistency..."
CARGO_VER=$(grep '^version' src-tauri/Cargo.toml | head -1 | cut -d'"' -f2)
TAURI_VER=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | tr -d ' ",' | cut -d: -f2)
PKG_VER=$(grep '"version"' package.json | head -1 | tr -d ' ",' | cut -d: -f2)

if [ "$CARGO_VER" != "$VERSION" ] || [ "$TAURI_VER" != "$VERSION" ] || [ "$PKG_VER" != "$VERSION" ]; then
  echo "ERROR: Version mismatch!"
  echo "  Cargo.toml: $CARGO_VER"
  echo "  tauri.conf.json: $TAURI_VER"
  echo "  package.json: $PKG_VER"
  echo "  Expected: $VERSION"
  exit 1
fi
echo "All versions: $VERSION ✓"

# Step 6: Summary
echo ""
echo "[5/6] Release artifacts ready:"
echo "  Installer: $INSTALLER"
echo "  Release notes: docs/RELEASE-NOTES-v${VERSION}.md"
echo "  Updater manifest: docs/latest.json"
echo ""
echo "[6/6] Next steps (manual):"
echo "  1. git tag v${VERSION}"
echo "  2. git push origin v${VERSION}"
echo "  3. gh release create v${VERSION} --title 'v${VERSION} — All signal. No feed.' --notes-file docs/RELEASE-NOTES-v${VERSION}.md '${INSTALLER}'"
echo "  4. Sign the NSIS zip with minisign"
echo "  5. Update docs/latest.json with signature"
echo "  6. Upload latest.json to the release"
echo ""
echo "=== Done ==="
