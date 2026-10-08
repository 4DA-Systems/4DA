# Verifying 4DA Downloads

This guide explains how to verify that a 4DA binary you downloaded is authentic and has not been tampered with. Verification takes a few minutes and requires only built-in OS tools (plus optionally `minisign`).

---

## Why Verify

When you download software from the internet, the file could be altered in transit, mirrored on an unofficial site, or replaced by a compromised build. Verification protects you against all three scenarios.

4DA provides three independent layers of verification:

| Layer | What It Proves | Tool Required |
|---|---|---|
| **SHA-256 checksum** | The file is bit-for-bit identical to what was published | None (built-in) |
| **Code signing** | The binary was produced by 4DA Systems Pty Ltd (macOS; Windows builds are not yet signed) | None (built-in) |
| **Minisign signature** | The installer / update payload was signed with the 4DA release key | `minisign` |

Any single layer is sufficient to detect tampering. Using more than one raises confidence further.

---

## 1. Verify Checksums (SHA-256)

Every GitHub release includes a checksum file listing the SHA-256 hash for each artifact. A SHA-256 hash is a unique fingerprint of the file contents. If even one byte changes, the hash changes completely.

### Download the checksum file

Go to the release page at [github.com/4DA-Systems/4DA/releases](https://github.com/4DA-Systems/4DA/releases) and download the checksum file alongside your installer.

### Compute the hash of your downloaded file

#### Windows (Command Prompt)

```
certutil -hashfile 4DA_1.0.0_x64-setup.exe SHA256
```

#### Windows (PowerShell)

```powershell
Get-FileHash .\4DA_1.0.0_x64-setup.exe -Algorithm SHA256
```

#### macOS

```bash
shasum -a 256 4DA_1.0.0_x64.dmg
```

#### Linux

```bash
sha256sum 4DA_1.0.0_amd64.AppImage
```

### Compare

The hash output must match the corresponding line in the checksum file exactly. If it does not match, do **not** run the file — see [What If Verification Fails](#6-what-if-verification-fails).

---

## 2. Verify Code Signing

Code signing uses a cryptographic certificate issued by a trusted certificate authority. It proves two things: the identity of the publisher and that the binary has not been modified after signing.

### Windows (not yet code-signed)

4DA Windows installers are currently **not Authenticode-signed**; code signing is planned. `Get-AuthenticodeSignature` on the installer reports `NotSigned`, the file's Properties have no **Digital Signatures** tab, and SmartScreen shows **"Windows protected your PC" / "Unknown publisher"** on first run.

On Windows, verify the **SHA-256 checksum** (section 1) and/or the **minisign signature** (section 3) before running the installer. Click **More info → Run anyway** only after one of them checks out.

### macOS (Apple Developer ID + Notarization)

4DA macOS builds are signed with an Apple Developer ID certificate (Team ID: **HVZS8TM5C5**) and notarized by Apple, which means Apple has scanned the binary and confirmed it is free of known malware.

**Verify the code signature:**

```bash
codesign --verify --deep --strict --verbose=2 /Applications/4DA.app
```

Expected output includes: `valid on disk`, `satisfies its Designated Requirement`.

**Verify Gatekeeper acceptance:**

```bash
spctl --assess --type exec /Applications/4DA.app
```

Expected output: `accepted` with `source=Developer ID`.

**Verify notarization:**

```bash
stapler validate /Applications/4DA.app
```

Expected output: `The validate action worked!`

The signing identity should read: **Developer ID Application: 4DA Systems Pty Ltd (HVZS8TM5C5)**

### Linux

There is no universal OS-level code signing standard for Linux. Verification relies on SHA-256 checksums (see section 1) and Minisign signatures (see section 3).

- **AppImage:** Verify with checksums and Minisign.
- **DEB / RPM:** Verify with checksums and Minisign. GPG package signing may be added in a future release.

---

## 3. Verify Minisign Signatures (Update Integrity)

[Minisign](https://jedisct1.github.io/minisign/) is a simple and robust signature scheme. The Tauri auto-updater uses Minisign to verify every update payload before applying it. You can also verify signatures manually.

### The public key

```
untrusted comment: minisign public key: 46ECE1D6A97849EF
RWTvSXip1uHsRurO5vC0TOSEVlipUqSzsHinODnu7DX5Tw11guDvHQQZ
```

- **Key ID:** `46ECE1D6A97849EF`
- This key is embedded in the application source code at [`src-tauri/tauri.conf.json`](https://github.com/4DA-Systems/4DA/blob/main/src-tauri/tauri.conf.json) (`plugins.updater.pubkey`) as a base64-encoded string. You can decode it yourself to confirm it matches the key above.

### Install minisign

```bash
# Using Cargo (any platform)
cargo install minisign

# macOS (Homebrew)
brew install minisign

# Ubuntu / Debian
apt install minisign

# Windows (Scoop)
scoop install minisign
```

### Verify a signature

Each release includes `.sig` files alongside the installers and update artifacts. A `.sig` file is a minisign signature **wrapped in base64** (the format the Tauri updater reads), so decode it first. Download both the artifact and its `.sig` file, then run:

```bash
base64 -d 4DA_1.0.3_x64-setup.exe.sig > installer.minisig
minisign -Vm 4DA_1.0.3_x64-setup.exe -x installer.minisig -P RWTvSXip1uHsRurO5vC0TOSEVlipUqSzsHinODnu7DX5Tw11guDvHQQZ
```

(On Windows PowerShell, decode with `[IO.File]::WriteAllBytes("$PWD\installer.minisig", [Convert]::FromBase64String((Get-Content .\4DA_1.0.3_x64-setup.exe.sig -Raw).Trim()))`.)

Replace the filename with the actual artifact you downloaded. If the signature is valid, minisign prints:

```
Signature and comment signature verified
```

If it prints an error, do **not** use the file — see [What If Verification Fails](#6-what-if-verification-fails).

### How the auto-updater uses this

When 4DA checks for updates, it downloads the update manifest (`latest.json`) and the signed update payload. Before applying the update, the embedded Minisign public key is used to verify the payload signature automatically. If verification fails, the update is rejected. No user action is required for this process.

---

## 4. Verify Source Code Matches Release

If you want the highest level of assurance, you can build 4DA from source and compare the result to the published binary. Every release corresponds to a git tag.

```bash
# Clone the repository
git clone https://github.com/4DA-Systems/4DA.git
cd 4DA

# Checkout the release tag
git checkout v1.0.0

# Build from source
pnpm install
pnpm run tauri build
```

See [BUILD-FROM-SOURCE.md](BUILD-FROM-SOURCE.md) for full instructions and prerequisites.

The `Cargo.lock` and `pnpm-lock.yaml` files are committed to the repository and pin every dependency to an exact version, ensuring reproducible builds.

---

## 5. Where to Find Verification Materials

| Material | Location |
|---|---|
| Installers, checksums, and signatures | [GitHub Releases](https://github.com/4DA-Systems/4DA/releases) |
| Minisign public key (in source) | [`src-tauri/tauri.conf.json`](https://github.com/4DA-Systems/4DA/blob/main/src-tauri/tauri.conf.json) `plugins.updater.pubkey` |
| This document | [`docs/VERIFY-DOWNLOADS.md`](https://github.com/4DA-Systems/4DA/blob/main/docs/VERIFY-DOWNLOADS.md) |
| Security policy and vulnerability reporting | [`SECURITY.md`](https://github.com/4DA-Systems/4DA/blob/main/SECURITY.md) |
| Network transparency audit | [`docs/NETWORK-TRANSPARENCY.md`](NETWORK-TRANSPARENCY.md) |
| How to build from source | [`docs/BUILD-FROM-SOURCE.md`](BUILD-FROM-SOURCE.md) |

---

## 6. What If Verification Fails

1. **Do not run the binary.** A failed verification means the file may have been tampered with or corrupted.
2. **Re-download** directly from the official [GitHub Releases page](https://github.com/4DA-Systems/4DA/releases). Do not use mirrors or third-party download sites.
3. **Verify again** using the steps above.
4. **Check the repository URL.** Ensure you are downloading from `github.com/4DA-Systems/4DA` and not a similarly named repository.
5. **If the problem persists,** report the issue to **security@4da.ai** with the following details:
   - Which file you downloaded and its SHA-256 hash
   - Which verification step failed
   - The URL you downloaded from
   - Your operating system and version

---

4DA Systems Pty Ltd (ACN 696 078 841) | FSL-1.1-Apache-2.0
