# Installing 4DA on macOS

4DA runs on macOS 10.15 (Catalina) and later, on both Apple Silicon and Intel Macs.

## Publisher verification

4DA is signed with an **Apple Developer ID** certificate issued to the developer and notarized by Apple. macOS Gatekeeper recognises the app immediately — no "unidentified developer" warnings on launch.

## Installation steps

### 1. Download

Download the latest `4DA-*.dmg` from the [Releases page](https://github.com/runyourempire/4DA/releases/latest). Choose the correct architecture:

- **Apple Silicon** (M1/M2/M3/M4): `4DA-*_aarch64.dmg`
- **Intel**: `4DA-*_x64.dmg`

The download page also lists:

- **`SHASUMS256.txt`** — SHA-256 of every artifact in the release.
- **`<installer>.dmg.sha256`** — per-file sidecar for your installer.

### 2. Verify the download (recommended)

```bash
# Compute the SHA-256 and compare to SHASUMS256.txt
shasum -a 256 4DA-*.dmg
```

If the hash matches, the file is genuine. If it doesn't, **do not open it** — re-download from the Releases page.

### 3. Install

1. Double-click the `.dmg` to mount it.
2. Drag **4DA** into your **Applications** folder.
3. Eject the disk image.

### 4. First launch

Open **4DA** from Applications or Spotlight. On first run, 4DA scans your local projects to learn your stack — nothing is uploaded; everything stays on your machine. Within 60 seconds you will see your first results.

## Code signing and notarization

Every release of 4DA is:

1. **Signed** with a Developer ID Application certificate using the hardened runtime.
2. **Notarized** by Apple — the app is submitted to Apple's notary service and stapled with an approval ticket before distribution.
3. **Gatekeeper-approved** — macOS allows the app to run without any security prompts.

Auto-updates are additionally signed with a minisign key pinned inside the binary.

## If the app still won't open

- **"4DA can't be opened because Apple cannot check it for malicious software"** — should not appear on notarized releases. If you see this, re-download from the official Releases page and verify the SHA-256.
- **Right-click → Open** — if macOS still blocks the app (e.g., on older macOS), right-click the app icon and choose **Open**. This bypasses the first-run Gatekeeper check.
- **`xattr -cr /Applications/4DA.app`** — if you downloaded via curl or a browser that adds quarantine flags, run this in Terminal to clear the extended attributes.

## Verifying the signature (advanced)

```bash
# Verify the code signature
codesign --verify --deep --strict /Applications/4DA.app

# Verify Gatekeeper acceptance
spctl --assess --verbose --type execute /Applications/4DA.app
```

A successful verification confirms the app was signed with a valid Developer ID and notarized by Apple.

## Privacy note

The installer performs no network activity. The application itself is local-first:

- All analysis runs on your machine.
- API keys you configure for AI providers are stored in your macOS Keychain.
- Telemetry, if you opt in, is anonymous and aggregate-only.

See the [Privacy Policy](https://4da.ai/privacy) for full detail.

## Questions

Open an issue at [github.com/runyourempire/4DA/issues](https://github.com/runyourempire/4DA/issues) or email `support@4da.ai`.
