# Security Policy

4DA Systems Pty Ltd (ACN 696 078 841) takes the security of 4DA and its users extremely seriously. This document describes our security policy, vulnerability disclosure process, and the security properties of the application.

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 1.x     | Yes                |
| < 1.0   | No                 |

Only the latest release in the 1.x series receives security updates. Users should always run the most recent version.

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Report privately through either channel:

- **GitHub private vulnerability reporting:** open the repository's **Security** tab and choose **Report a vulnerability**. This creates a private advisory that only you and the maintainers can see.
- **Email:** **security@4da.ai**

Include:

- A clear description of the vulnerability
- Steps to reproduce the issue
- Your assessment of the impact (e.g., data exfiltration, privilege escalation, code execution)
- Any proof-of-concept code or screenshots
- Your preferred attribution name (if you want credit)

### Response Timeline

| Stage          | SLA                |
|----------------|--------------------|
| Acknowledgment | Within 48 hours    |
| Triage         | Within 5 business days |
| Fix (critical) | Best effort, typically within 30 days |
| Fix (other)    | Prioritized in the next release cycle |

We will keep you informed of our progress throughout the process.

### Safe Harbor

4DA Systems will not pursue legal action against security researchers who:

- Make a good-faith effort to avoid privacy violations, data destruction, and service disruption
- Report vulnerabilities promptly and provide reasonable time for remediation before any public disclosure
- Do not exploit vulnerabilities beyond what is necessary to demonstrate the issue

### Credit

With your permission, we will acknowledge your contribution in the release notes of the version that addresses the vulnerability. If you prefer to remain anonymous, we will respect that.

## Security Architecture

4DA is a local-first desktop application. By design, it minimizes attack surface by keeping data and computation on the user's machine.

### Backend (Rust)

- **Memory safety.** The Rust backend eliminates entire classes of vulnerabilities: buffer overflows, use-after-free, data races.
- **Credential storage.** API keys and the licence key are stored in the OS credential store (Windows Credential Manager, macOS Keychain, Linux Secret Service). A key is removed from `settings.json` (and its `settings.json.bak`) only after a read through a fresh credential-store handle returns the identical value; on first start after upgrading, keys found in plaintext are migrated all-or-nothing. If the credential store is unavailable (headless CI, locked keychain, no keyring service on some Linux setups), the key stays in `settings.json`, which is restricted to the current user (owner-only ACL on Windows, mode 0600 elsewhere), and Settings → AI Provider says the key is kept in settings.json instead of claiming secure storage. An empty value or a failed read never deletes or overwrites a stored key. With `FOURDA_DATA_DIR` set, profile secrets use a separate per-profile credential-store service.
- **Path canonicalization.** All file system operations canonicalize paths to prevent symlink and directory traversal attacks.
- **SSRF prevention.** Outbound HTTP requests on content-fetch and updater paths validate the URL and block private/internal IP ranges via `validate_url_safe_for_request`. Webhook registration validation is being hardened in v1.1; user-supplied webhook URLs should be treated as trusted until then.

### Frontend (React/TypeScript)

- **Content Security Policy.** Strict CSP enforcement: no inline scripts, no iframes, no `eval()`.
- **HTML sanitization.** All rendered HTML is sanitized with DOMPurify before insertion into the DOM.

### Update Mechanism and Release Signing

- **Signed updates.** Application updates are signed with Ed25519 (minisign). The updater verifies the signature against the public key built into the app before applying any update, and you click to apply it.
- **Signing, per release file** (as built by `.github/workflows/release.yml`; v1.0.3 is the current release):

  | File | Platform code signature | minisign `.sig` | In `SHASUMS256.txt` |
  |---|---|---|---|
  | macOS `.dmg` | The app inside is signed with an Apple Developer ID and notarized | No (the updater uses the `.app.tar.gz` below) | Yes |
  | macOS `.app.tar.gz` (update bundle) | Same signed, notarized app | Yes | No |
  | Windows `-setup.exe` | **Not Authenticode-signed.** Releases ship unsigned only under a per-tag waiver (`ALLOW_UNSIGNED_WINDOWS`), and the release notes then say so; v1.0.3 is unsigned. Signing via Azure Artifact Signing is wired but not yet configured | Yes | Yes |
  | Linux `.AppImage`, `.deb`, `.rpm` | None (Linux has no platform signing step here) | Yes | Yes |
  | `4da-cli-*` binaries (all platforms) | **None** — built with `cargo build` and uploaded as-is, not signed or notarized on any platform | No | Yes |
  | SBOMs (`sbom-*.json`) | Sigstore cosign keyless bundle (`.sig.bundle`) | — | — |

- **Checksums.** `SHASUMS256.txt` and the per-file `.sha256` sidecars are published on the GitHub release but are not themselves signed, so they prove a download matches what was published there, not who published it. For the files with a `.sig`, the minisign signature is the stronger check. How to verify each: [docs/VERIFY-DOWNLOADS.md](docs/VERIFY-DOWNLOADS.md).

### Supply Chain

- **cargo-deny.** Enforced in CI to audit Rust dependencies for known vulnerabilities, duplicate crates, and license compliance.
- **SBOM.** A CycloneDX Software Bill of Materials is published with every release, accompanied by a cosign attestation.

## Scope

### In Scope

- The 4DA desktop application (all platforms)
- The update and auto-update mechanism
- The bundled MCP server mcp-memory-server (`@4da/mcp-server` has its own policy: https://github.com/4DA-Systems/4da-mcp-server/security/policy)

### Out of Scope

- **Third-party dependencies.** If you find a vulnerability in an upstream dependency, please report it to the upstream maintainer. We will update promptly once a fix is available.
- **Social engineering** of 4DA Systems employees or contributors.
- **Denial of service** against infrastructure (4da.ai, update servers).
- **Attacks requiring physical access** to an unlocked machine where 4DA is running.

## Known Security Properties

These are architectural properties, not claims of invulnerability.

- **BYOK (Bring Your Own Key).** API keys are stored in the OS credential store (see "Credential storage" above for the owner-only `settings.json` fallback used when the store is unavailable) and are only transmitted to the providers the user has explicitly configured. 4DA Systems never receives or stores user API keys.
- **Local-first, direct-to-provider.** 4DA has no server-side database, no user accounts, and no 4DA-operated analytics, tracking, or backend storage. Your indexed content, scores, and decisions stay in a SQLite database on your machine, and your source code and file contents are not sent anywhere (the one opt-in exception, cloud embeddings, is in [`NETWORK.md`](NETWORK.md) §2a). Outbound traffic, summarised (the complete, source-referenced list is [`NETWORK.md`](NETWORK.md)):
  - **Dependency checks:** package names and installed versions from your lockfiles go to OSV.dev; package names go to npm, crates.io, PyPI and the Go module proxy. Go module paths are sent in full, so a private module path names its repository.
  - **Source adapters** fetching public content (Hacker News, GitHub, Reddit, arXiv, RSS and the others you enable).
  - **BYOK AI providers** you have explicitly configured (Anthropic, OpenAI, an OpenAI-compatible endpoint, or local Ollama). A cloud provider receives item titles and snippets plus a short context profile: dependency names (and, for the AI briefing, installed and fixed versions), project labels, git-derived topics, your interests and saved-item titles. Local paths are rewritten to project labels at the LLM client boundary (`src-tauri/src/privacy_egress.rs`).
  - **Licence traffic, paid licences only:** the key goes to Keygen (`api.keygen.sh`) for validation and, for subscriptions near expiry, to 4DA's own server `4da.ai` for renewal. "Recover License by Email" sends the email address you type to `4da.ai`.
  - **Update checks** against the signed updater endpoint on GitHub Releases, and **one-time downloads** (the local search model from Hugging Face, when a feature first needs it).
  - **No crash reporting or telemetry.** 4DA sends no crash reports to any server. If you hit a bug, **Settings → Privacy → Export diagnostics** builds a scrubbed, local report (usernames and secrets redacted) that you review and choose whether to attach to a bug report. Nothing is transmitted automatically.
- **4DA's own server is for licences only.** 4DA Systems runs `4da.ai`, which the app contacts only for the licence renewal and recovery calls above. It receives no projects, scores, searches or reading history.
- **The Windows webview runtime is not 4DA's code.** On Windows the interface is drawn by Microsoft Edge WebView2, which contacts Microsoft on its own (diagnostic data, updates, SmartScreen, crash dumps). 4DA's interface cannot reach the network (its CSP allows no host); details and controls are in [`NETWORK.md`](NETWORK.md) §5.
- **Local HTTP listener (Signal Terminal).** While 4DA runs it serves a read-only web UI and JSON API on `127.0.0.1:4446` (4447 in development builds) (`src-tauri/src/signal_terminal.rs`). It binds to loopback only, rejects any request whose `Host` header is not a loopback name for that port (DNS-rebinding defence), sends no CORS headers, and requires an `X-4DA-Token` header (or `?token=` on the event stream) matching a 32-character random token stored in `signal_terminal_token.txt` in the data directory, compared in constant time with no localhost bypass. The token persists across restarts; delete the file to rotate it. Some responses include local project paths, so any process running as your user that can read the token file can read them. There is no setting to disable the listener yet; it stops when the app quits. Reports about this surface are in scope.
- **On-device activity log.** 4DA records UI activity (tab opens, result clicks, feedback) in a local SQLite table to power relevance learning. This data stays on your machine and is never transmitted. A Privacy → Activity Tracking setting exists and is off by default, but the current code does not yet consult it, so the recording happens regardless of that setting until the gate is wired.
- **Database encryption.** The default public build uses bundled SQLite without at-rest encryption. The SQLCipher scaffolding exists for self-compiled builds. Opt-in at-rest encryption for the default installer is on the v1.1 roadmap.
- **No accounts.** There is no user database, authentication system, or session infrastructure to breach.

For the authoritative list of every outbound connection with source-code references, see [`NETWORK.md`](NETWORK.md) and [`docs/NETWORK-TRANSPARENCY.md`](docs/NETWORK-TRANSPARENCY.md).

## Installer Changes and Uninstalling

### Microsoft Defender exclusions (Windows installer)

If the Windows installer runs with administrator rights, it adds two Microsoft Defender exclusions: the process name `fourda.exe` and the install folder (`src-tauri/windows/hooks.nsh`, `NSIS_HOOK_POSTINSTALL`). Without administrator rights the commands fail silently and nothing is added. The uninstaller removes both, again only when it runs with administrator rights. To check: `Get-MpPreference | Select-Object ExclusionProcess, ExclusionPath` in an elevated PowerShell; to remove by hand: `Remove-MpPreference -ExclusionProcess fourda.exe` and `Remove-MpPreference -ExclusionPath "<install folder>"`.

### What uninstalling leaves behind

Uninstalling removes the program. It does **not** remove your data or stored secrets unless you take the steps below. Paths are from `src-tauri/src/runtime_paths.rs`; `com.4da.app` is the app identifier.

**Turn these off in the app first** (they live outside the install folder): Settings → Monitoring → background refresh (removes the Windows scheduled task `4DA Background Refresh`; by hand: `schtasks /Delete /TN "4DA Background Refresh" /F`), and launch at login if you enabled it.

| | Windows | macOS | Linux |
|---|---|---|---|
| Data (database, `settings.json`, backups, digests, Signal Terminal token) | `%APPDATA%\com.4da.app\` | `~/Library/Application Support/com.4da.app/` | `~/.local/share/4da/` |
| Cache (downloaded search model; on Windows also the WebView2 profile, `EBWebView`) | `%LOCALAPPDATA%\com.4da.app\` | `~/Library/Caches/com.4da.app/` | `~/.cache/4da/` |
| Older builds' digest folder (before 2026-10-10) | `%APPDATA%\4da\` | `~/Library/Application Support/4da/` | inside `~/.local/share/4da/` |
| Stored secrets (API keys, licence key, licence state) | Credential Manager → Windows Credentials → Generic Credentials: entries whose name ends in `.com.4da.app` (or `.com.4da.app.p.<id>` for extra profiles) | Keychain Access: items with service `com.4da.app` | Secret Service (GNOME Keyring / KWallet): items with service `com.4da.app` |

- **Windows:** the uninstaller offers **Delete app data** (unchecked by default). Ticking it removes `%APPDATA%\com.4da.app` and `%LOCALAPPDATA%\com.4da.app`; it does not touch `%APPDATA%\4da` or the Credential Manager entries. The app also registers the `fourda://` link handler and a notification identity under `HKCU\Software\Classes` (`fourda`, `AppUserModelId\com.4da.app`); the uninstaller removes the `fourda` key when it points at the uninstalled program, and the notification key may remain (harmless, removable with `reg delete "HKCU\Software\Classes\AppUserModelId\com.4da.app" /f`).
- **macOS / Linux:** delete the folders above (and the webview's own data folder, if your system created one named `com.4da.app`), then the keychain items (macOS: `security delete-generic-password -s com.4da.app -a <name>` per item, or Keychain Access; Linux: `secret-tool clear service com.4da.app`).
- Deleting the data folder deletes your local database, which is the only copy of your 4DA history. Deleting the stored secrets removes your saved API keys and licence key; keep your licence key if you plan to reinstall.

## Contact

For security matters: **security@4da.ai**

For general inquiries: **https://4da.ai**

## License

4DA is licensed under FSL-1.1-Apache-2.0. See [LICENSE](LICENSE) for details.
