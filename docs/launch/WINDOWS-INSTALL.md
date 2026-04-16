# Installing 4DA on Windows

4DA runs on Windows 10 and 11. The app is under 100 MB and installs in seconds.

## Why Windows may warn you on first run

4DA is a new application. Microsoft's **SmartScreen** filter is a reputation system — it learns to trust an application only after thousands of people have downloaded and run it without issue. Because 4DA is newly released, SmartScreen has not yet built that reputation, so Windows will show a blue dialog titled **"Windows protected your PC"** the first time you run the installer.

This is expected, and you have the right tools to verify the download is genuine before you proceed.

## Installation steps

### 1. Download

Download the latest `4DA-Setup-*.exe` from the [Releases page](https://github.com/runyourempire/4DA/releases/latest). The download page also lists:

- **`SHASUMS256.txt`** — a single canonical file listing the SHA-256 of every artifact in the release. Download this alongside the installer.
- **`<installer>.exe.sha256`** — a per-file sidecar with just the hash for your installer, convenient for one-line verification.
- **`<installer>.exe.sig`** — a minisign signature you can verify against the project's public key for even stronger assurance.

### 2. Verify the download (recommended)

Download `SHASUMS256.txt` from the Releases page into the same folder as the installer, then run one of these in PowerShell:

```powershell
# Option A — compute and compare yourself
Get-FileHash -Algorithm SHA256 .\4DA-Setup-1.0.0.exe
# Then visually compare the output hash to the line for this file in SHASUMS256.txt.
```

```bash
# Option B — if you have Git Bash or WSL, verify every file at once
sha256sum -c SHASUMS256.txt --ignore-missing
# Each line prints `<file>: OK` on a match. Any `FAILED` means a corrupt or tampered file.
```

If the hash matches byte-for-byte, the file is genuine. If it doesn't match, **do not run it** — re-download from the Releases page.

### 3. Run the installer

Double-click the `.exe`. When SmartScreen appears:

1. Click **"More info"** in the dialog.
2. Click **"Run anyway"**.
3. Follow the installer prompts. Install to the default location.

### 4. First launch

Launch **4DA** from the Start menu. On first run, 4DA will scan your local projects to learn your stack — nothing is uploaded; everything stays on your machine. Within 60 seconds you will see your first results.

## Why we ship without EV code signing at launch

Extended Validation (EV) code signing certificates remove the SmartScreen warning but require a hardware token, organisational validation, and a lead time that does not align with our shipping cadence. Rather than delay the launch, we chose to:

1. **Publish checksums and signatures for every release** so any user can verify the build integrity themselves.
2. **Build reputation transparently** via the auto-updater — as downloads and successful runs accumulate, SmartScreen's reputation system will recognise 4DA automatically.
3. **Ship signed builds via the built-in updater.** When EV signing is in place, the update is delivered silently. You will not need to re-download or re-install.

This approach keeps the release on schedule and gives technically-inclined users the stronger verification path (hash + signature) that most applications never expose.

## Auto-updates

4DA uses the Tauri updater. Updates are:

- **Signed** with a minisign key that ships with the application. The public key is pinned inside the binary — an attacker who wanted to push a malicious update would need to break the signature, not just host a fake endpoint.
- **Delivered from GitHub Releases**, the same channel you downloaded from.
- **Verified before installation** — a failed signature check aborts the update.

You do not need to do anything to receive updates. When a new version is available, 4DA will prompt you on next launch.

## If the installer still won't run

- **"Unknown publisher"** — expected for new builds. Proceed via "More info → Run anyway" as above.
- **"This app can't run on your PC"** — you are likely on 32-bit Windows. 4DA requires 64-bit Windows 10 or later.
- **Antivirus quarantine** — occasionally aggressive antivirus heuristics flag new Rust binaries. If your antivirus quarantines the installer, restore it from quarantine and verify the SHA-256 matches the Releases page before running. If the hash matches, the file is genuine; you can submit it to your antivirus vendor as a false-positive report to improve detection for all users.
- **Nothing happens when you double-click** — right-click the `.exe` → **Properties** → check the **"Unblock"** box at the bottom → **OK**. Then double-click again.

## Verifying the signature (advanced)

For maximum assurance, verify the minisign signature published alongside each release:

```powershell
# Install minisign (once)
scoop install minisign

# Fetch the public key (already in the app; also published at 4da.ai/keys/updater.pub)
# Verify
minisign -Vm 4DA-Setup-1.0.0.exe -p updater.pub
```

A successful verification confirms the file was signed by 4DA Systems Pty Ltd and has not been modified since.

## Privacy note

The installer performs no network activity. The application itself is local-first:

- All analysis runs on your machine.
- API keys you configure for AI providers are stored in your OS keychain.
- Telemetry, if you opt in, is anonymous and aggregate-only.

See the [Privacy Policy](https://4da.ai/privacy) for full detail.

## Questions

Open an issue at [github.com/runyourempire/4DA/issues](https://github.com/runyourempire/4DA/issues) or email `support@4da.ai`.
