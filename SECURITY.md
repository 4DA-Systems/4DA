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

Send vulnerability reports to **security@4da.ai**. Include:

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
- **Credential storage.** API keys are stored in the platform keychain (Windows Credential Manager, macOS Keychain, Linux Secret Service). Credentials are never written to plaintext files.
- **Path canonicalization.** All file system operations canonicalize paths to prevent symlink and directory traversal attacks.
- **SSRF prevention.** Outbound HTTP requests are validated against an allowlist. Private and internal IP ranges are blocked.

### Frontend (React/TypeScript)

- **Content Security Policy.** Strict CSP enforcement: no inline scripts, no iframes, no `eval()`.
- **HTML sanitization.** All rendered HTML is sanitized with DOMPurify before insertion into the DOM.

### Update Mechanism

- **Signed updates.** Application updates are signed with Ed25519 (minisign). The updater verifies signatures before applying any update.
- **Code signing.** Windows binaries are EV code-signed. macOS binaries are signed and notarized by Apple.

### Supply Chain

- **cargo-deny.** Enforced in CI to audit Rust dependencies for known vulnerabilities, duplicate crates, and license compliance.
- **SBOM.** A CycloneDX Software Bill of Materials is published with every release, accompanied by a cosign attestation.

## Scope

### In Scope

- The 4DA desktop application (all platforms)
- The update and auto-update mechanism
- Bundled MCP servers (mcp-memory-server, mcp-4da-server)

### Out of Scope

- **Third-party dependencies.** If you find a vulnerability in an upstream dependency, please report it to the upstream maintainer. We will update promptly once a fix is available.
- **Social engineering** of 4DA Systems employees or contributors.
- **Denial of service** against infrastructure (4da.ai, update servers).
- **Attacks requiring physical access** to an unlocked machine where 4DA is running.

## Known Security Properties

These are architectural properties, not claims of invulnerability.

- **BYOK (Bring Your Own Key).** API keys are stored in the platform keychain and are only transmitted to the providers the user has explicitly configured. 4DA Systems never receives or stores user API keys.
- **No telemetry.** Zero data leaves the machine without explicit user action. There are no analytics, crash reporters, or phone-home mechanisms.
- **Local-first.** 4DA functions fully offline. There is no required server connection, no user account, and no remote database.
- **No accounts.** There is no user database, authentication system, or session infrastructure to breach.

## Contact

For security matters: **security@4da.ai**

For general inquiries: **https://4da.ai**

## License

4DA is licensed under FSL-1.1-Apache-2.0. See [LICENSE](LICENSE) for details.
