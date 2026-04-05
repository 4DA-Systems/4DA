#!/usr/bin/env bash
# Post-release script: calculate checksums and prepare package manager submissions
# Run after tagging a release: ./scripts/post-release-packages.sh v1.0.0
#
# Prerequisites:
#   - GitHub CLI (gh) authenticated
#   - Release already published with artifacts

set -euo pipefail

VERSION="${1:?Usage: $0 <version-tag e.g. v1.0.0>}"
SEMVER="${VERSION#v}"

echo "=== 4DA Post-Release Package Preparation ==="
echo "Version: ${SEMVER}"
echo ""

# Create temp directory for downloads
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "--- Downloading release artifacts ---"

# Download all release artifacts
gh release download "${VERSION}" --dir "${TMPDIR}" --pattern "*.exe" --pattern "*.dmg" --pattern "*.AppImage" 2>/dev/null || true

echo ""
echo "--- SHA256 Checksums ---"
echo ""

# Calculate checksums
for file in "${TMPDIR}"/*; do
    if [ -f "$file" ]; then
        HASH=$(sha256sum "$file" | cut -d' ' -f1)
        NAME=$(basename "$file")
        echo "${NAME}: ${HASH}"
    fi
done

echo ""
echo "--- Package Manager Templates ---"
echo ""

# Find specific files and output ready-to-use commands
NSIS_FILE=$(find "${TMPDIR}" -name "*x64-setup.exe" | head -1)
DMG_ARM=$(find "${TMPDIR}" -name "*aarch64.dmg" | head -1)
DMG_INTEL=$(find "${TMPDIR}" -name "*x86_64.dmg" -o -name "*x64.dmg" | head -1)
APPIMAGE=$(find "${TMPDIR}" -name "*.AppImage" | head -1)

if [ -n "$NSIS_FILE" ]; then
    NSIS_SHA=$(sha256sum "$NSIS_FILE" | cut -d' ' -f1)
    echo "Winget SHA256: ${NSIS_SHA}"
    echo "  wingetcreate update 4DA.4DAHome --version ${SEMVER} --urls https://github.com/runyourempire/4DA/releases/download/${VERSION}/$(basename ${NSIS_FILE}) --submit"
    echo ""
fi

if [ -n "$DMG_ARM" ]; then
    ARM_SHA=$(sha256sum "$DMG_ARM" | cut -d' ' -f1)
    echo "Homebrew (ARM) SHA256: ${ARM_SHA}"
fi

if [ -n "$DMG_INTEL" ]; then
    INTEL_SHA=$(sha256sum "$DMG_INTEL" | cut -d' ' -f1)
    echo "Homebrew (Intel) SHA256: ${INTEL_SHA}"
fi

if [ -n "$APPIMAGE" ]; then
    AI_SHA=$(sha256sum "$APPIMAGE" | cut -d' ' -f1)
    echo "AUR AppImage SHA256: ${AI_SHA}"
fi

echo ""
echo "--- Next Steps ---"
echo "1. Update docs/launch/homebrew-cask.rb with SHA256 values and submit PR"
echo "2. Run wingetcreate command above to submit Winget manifest"
echo "3. Update docs/launch/aur-PKGBUILD with SHA256 and push to AUR"
echo ""
echo "Done!"
