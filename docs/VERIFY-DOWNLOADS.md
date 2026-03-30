# Verifying 4DA Downloads

This guide explains how to verify that a 4DA binary you downloaded is authentic and has not been tampered with. Verification takes a few minutes and requires only built-in OS tools (plus optionally `minisign`).

---

## Why Verify

When you download software from the internet, the file could be altered in transit, mirrored on an unofficial site, or replaced by a compromised build. Verification protects you against all three scenarios.

4DA provides three independent layers of verification:

| Layer | What It Proves | Tool Required |
|---|---|---|
| **SHA-256 checksum** | The file is bit-for-bit identical to what was published | None (built-in) |
| **Code signing** | The binary was produced by 4DA Systems Pty Ltd | None (built-in) |
| **Minisign signature** | The auto-updater payload is authentic and untampered | `minisign` |

Any single layer is sufficient to detect tampering. Using more than one raises confidence further.

---

## 1. Verify Checksums (SHA-256)

Every GitHub release includes a checksum file listing the SHA-256 hash for each artifact. A SHA-256 hash is a unique fingerprint of the file contents. If even one byte changes, the hash changes completely.

### Download the checksum file

Go to the release page at [github.com/runyourempire/4DA/releases](https://github.com/runyourempire/4DA/releases) and download the checksum file alongside your installer.

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

### Windows (EV Code Signing)

4DA Windows installers are signed with an Extended Validation (EV) code signing certificate issued by SSL.com to **4DA Systems Pty Ltd**. EV certificates require rigorous identity verification by the certificate authority, and they provide immediate Microsoft SmartScreen reputation (no "unknown publisher" warnings).

**GUI method:**

1. Right-click the `.exe` installer.
2. Select **Properties**.
3. Open the **Digital Signatures** tab.
4. Select the signature entry and click **Details**.
5. Confirm the signer is **4DA Systems Pty Ltd** and the certificate chain is valid.

**PowerShell method:**

```powershell
Get-AuthenticodeSignature ".\4DA_1.0.0_x64-setup.exe"
```

The output should show:

- **Status:** `Valid`
- **SignerCertificate Subject:** contains `4DA Systems Pty Ltd`
- **Issuer:** `SSL.com`

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
untrusted comment: minisign public key: 19AF42B1B6971703
RWQDF5e2sUKvGYCPxka/KazOY6s/8w85tK7C8rD6IRAb1ucOhVfePRZF
```

- **Key ID:** `19AF42B1B6971703`
- This key is embedded in the application source code at [`src-tauri/tauri.conf.json` (line 48)](https://github.com/runyourempire/4DA/blob/main/src-tauri/tauri.conf.json#L48) as a base64-encoded string. You can decode it yourself to confirm it matches the key above.

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

Each release includes `.sig` files alongside the update artifacts. Download both the artifact and its `.sig` file, then run:

```bash
minisign -Vm 4DA_1.0.0_x64-setup.nsis.zip -P RWQDF5e2sUKvGYCPxka/KazOY6s/8w85tK7C8rD6IRAb1ucOhVfePRZF
```

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
git clone https://github.com/runyourempire/4DA.git
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
| Installers, checksums, and signatures | [GitHub Releases](https://github.com/runyourempire/4DA/releases) |
| Minisign public key (in source) | [`src-tauri/tauri.conf.json`](https://github.com/runyourempire/4DA/blob/main/src-tauri/tauri.conf.json) line 48 |
| This document | [`docs/VERIFY-DOWNLOADS.md`](https://github.com/runyourempire/4DA/blob/main/docs/VERIFY-DOWNLOADS.md) |
| Security policy and vulnerability reporting | [`SECURITY.md`](https://github.com/runyourempire/4DA/blob/main/SECURITY.md) |
| Network transparency audit | [`docs/NETWORK-TRANSPARENCY.md`](NETWORK-TRANSPARENCY.md) |
| How to build from source | [`docs/BUILD-FROM-SOURCE.md`](BUILD-FROM-SOURCE.md) |

---

## 6. What If Verification Fails

1. **Do not run the binary.** A failed verification means the file may have been tampered with or corrupted.
2. **Re-download** directly from the official [GitHub Releases page](https://github.com/runyourempire/4DA/releases). Do not use mirrors or third-party download sites.
3. **Verify again** using the steps above.
4. **Check the repository URL.** Ensure you are downloading from `github.com/runyourempire/4DA` and not a similarly named repository.
5. **If the problem persists,** report the issue to **security@4da.ai** with the following details:
   - Which file you downloaded and its SHA-256 hash
   - Which verification step failed
   - The URL you downloaded from
   - Your operating system and version

---

4DA Systems Pty Ltd (ACN 696 078 841) | FSL-1.1-Apache-2.0
