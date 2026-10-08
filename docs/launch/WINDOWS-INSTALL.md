# Installing 4DA on Windows

4DA runs on Windows 10 and 11. The installer is about 280 MB — most of that is the local embedding model, which ships inside it so semantic search works offline from the first run.

## Publisher verification

The Windows installer is currently **not Authenticode code-signed**. Windows therefore cannot tell you who published it: SmartScreen shows **"Windows protected your PC"** and the publisher appears as **"Unknown publisher"**. That is expected for every download of the current release, and it is a real gap, not a formality.

Until code signing ships (it is planned, but has no date yet), the trust anchor on your side is the published **SHA-256 checksum** and the **minisign signature**. Verify one of them (step 2 below) before you run the installer.

## Installation steps

### 1. Download

Download the latest `4DA_<version>_x64-setup.exe` from the [Releases page](https://github.com/4DA-Systems/4DA/releases/latest). The release also lists:

- **`SHASUMS256.txt`** — a single canonical file listing the SHA-256 of every artifact in the release. Download this alongside the installer.
- **`<installer>.exe.sha256`** — a per-file sidecar with just the hash for your installer, convenient for one-line verification.
- **`<installer>.exe.sig`** — a minisign signature from the 4DA release key (see [Verifying the signature](#verifying-the-signature-advanced)).

### 2. Verify the download (do this before running it)

Download `SHASUMS256.txt` from the Releases page into the same folder as the installer, then run one of these:

```powershell
# Option A — PowerShell: compute the hash and compare it yourself
Get-FileHash -Algorithm SHA256 .\4DA_1.0.3_x64-setup.exe
# Compare the output with the line for this file in SHASUMS256.txt (case does not matter).
```

```bash
# Option B — Git Bash or WSL: verify every downloaded file at once
sha256sum -c SHASUMS256.txt --ignore-missing
# Each line prints `<file>: OK` on a match. Any `FAILED` means a corrupt or tampered file.
```

If the hash matches, the file is byte-for-byte what was published. If it does not match, **do not run it** — re-download from the Releases page and verify again.

### 3. Run the installer

Double-click the `.exe`. SmartScreen will show **"Windows protected your PC"** with **Publisher: Unknown publisher**. Only once you have verified the SHA-256 (or the minisign signature) in step 2, click **More info → Run anyway**. Follow the installer prompts and install to the default location.

### 4. First launch

Launch **4DA** from the Start menu. On first run, 4DA will scan your local projects to learn your stack — nothing is uploaded; your project file contents stay on your machine. Your first results appear once the initial scan and scoring finish.

## Code signing

Windows builds are **not yet Authenticode-signed**; code signing is planned. Until it ships:

1. **SmartScreen will warn** on first run, and the publisher shows as unknown.
2. **Checksums and minisign signatures are published** alongside every release — verify them before running the installer.
3. **Auto-updates are minisign-verified** — see below. Updates do not carry an Authenticode signature either.

## Auto-updates

4DA uses the Tauri updater. Updates are:

- **Signed** with the 4DA minisign release key. The public key is pinned inside the binary — an attacker who wanted to push a malicious update would need to break the signature, not just host a fake endpoint.
- **Delivered from GitHub Releases**, the same channel you downloaded from.
- **Verified before installation** — a failed signature check aborts the update.

You do not need to do anything to receive updates. When a new version is available, 4DA will prompt you on next launch.

## If the installer still won't run

- **"Windows protected your PC" / "Unknown publisher"** — expected for the current, unsigned release. Verify the SHA-256 against `SHASUMS256.txt` first; if it matches, click **More info → Run anyway**. If it does not match, do not run it — re-download from the official Releases page.
- **"This app can't run on your PC"** — you are likely on 32-bit Windows. 4DA requires 64-bit Windows 10 or later.
- **Antivirus quarantine** — occasionally aggressive antivirus heuristics flag new Rust binaries, and unsigned ones more often. If your antivirus quarantines the installer, restore it from quarantine and verify the SHA-256 matches the Releases page before running. If the hash matches, the file is genuine; you can submit it to your antivirus vendor as a false-positive report to improve detection for all users.
- **Nothing happens when you double-click** — right-click the `.exe` → **Properties** → check the **"Unblock"** box at the bottom → **OK**. Then double-click again.

## Verifying the signature (advanced)

For stronger assurance than the checksum, verify the minisign signature published alongside each release. The public key is the one pinned in the app's updater config ([`src-tauri/tauri.conf.json`](https://github.com/4DA-Systems/4DA/blob/main/src-tauri/tauri.conf.json), `plugins.updater.pubkey`, base64-encoded):

```
untrusted comment: minisign public key: 46ECE1D6A97849EF
RWTvSXip1uHsRurO5vC0TOSEVlipUqSzsHinODnu7DX5Tw11guDvHQQZ
```

The release `.sig` file is a minisign signature **wrapped in base64** (the format the Tauri updater reads), so decode it first:

```powershell
# Install minisign (once)
scoop install minisign

# Decode the base64-wrapped signature into a plain minisign signature file
[IO.File]::WriteAllBytes("$PWD\installer.minisig", [Convert]::FromBase64String((Get-Content .\4DA_1.0.3_x64-setup.exe.sig -Raw).Trim()))

# Verify
minisign -Vm .\4DA_1.0.3_x64-setup.exe -x .\installer.minisig -P RWTvSXip1uHsRurO5vC0TOSEVlipUqSzsHinODnu7DX5Tw11guDvHQQZ
```

A successful verification confirms the file was signed with the 4DA release key and has not been modified since.

## Privacy note

The installer performs no network activity. The application itself is local-first:

- All analysis runs on your machine.
- API keys you configure for AI providers are stored in your OS keychain.
- No telemetry or crash reporting is sent — ever. If something breaks, Settings → Privacy → Export diagnostics writes a scrubbed local report you choose whether to share.

See the [Privacy Policy](https://4da.ai/privacy) for full detail.

## Questions

Open an issue at [github.com/4DA-Systems/4DA/issues](https://github.com/4DA-Systems/4DA/issues) or email `support@4da.ai`.
