# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| Latest  | Yes                |
| Older   | No                 |

Only the latest release of 4DA receives security updates. Users should keep their installation up to date.

## Reporting a Vulnerability

**Do NOT open a public issue for security vulnerabilities.**

Report vulnerabilities by email to **security@4da.ai**. Include as much detail as possible:

- Description of the vulnerability
- Steps to reproduce
- Affected component (frontend, Rust backend, IPC layer, update mechanism, etc.)
- Potential impact assessment
- Suggested mitigation or fix (if any)

### Response Timeline

- **48 hours** -- Acknowledgment of your report
- **7 days** -- Initial assessment and severity classification
- **90 days** -- Target resolution window before coordinated disclosure

We will keep you informed of progress throughout the process.

## Scope

### In Scope

The following are considered valid security vulnerabilities:

- **API key exposure** -- Any path by which locally stored API keys (`data/settings.json`) could be exfiltrated, leaked, or accessed by unauthorized processes
- **Data exfiltration** -- Any mechanism that causes local data (SQLite database, user settings, ACE project scans) to leave the user's machine without explicit consent
- **Update mechanism compromise** -- Bypass or tampering with Minisign-signed application updates
- **IPC boundary bypass** -- Unauthorized access across the Tauri IPC boundary between the frontend webview and the Rust backend
- **Local privilege escalation** -- Exploits that allow the app to gain system privileges beyond its intended scope
- **CSP bypass** -- Content Security Policy violations in the webview that enable script injection
- **Dependency vulnerabilities** -- Critical CVEs in third-party dependencies that are exploitable in 4DA's context

### Out of Scope

The following are **not** considered vulnerabilities:

- Social engineering attacks against users or maintainers
- Attacks requiring physical access to the user's machine
- Denial of service against the local application
- Issues requiring pre-existing malware or a compromised operating system
- Vulnerabilities in third-party API services accessed via user-provided keys
- Self-inflicted misconfiguration of local settings files

## Security Model

4DA is a privacy-first desktop application. Its security model is built on the following principles:

### BYOK (Bring Your Own Key)

Users provide their own API keys. Keys are stored exclusively in `data/settings.json` on the local filesystem and are never transmitted to 4DA Systems or any third party. Keys are sent only to the API endpoints the user has explicitly configured.

### Local-First Architecture

All data processing occurs on the user's machine. The SQLite database and all derived intelligence remain local. There is no remote backend, no cloud sync, and no server-side processing.

### Zero Telemetry

4DA collects no telemetry, no analytics, and no usage data. There are no tracking pixels, no crash reporters, and no remote data collection of any kind.

### Signed Updates

Application updates are signed using Minisign. The update mechanism verifies cryptographic signatures before applying any update, preventing tampering in transit.

### Tauri Capability System

The Rust backend and React frontend communicate through Tauri's IPC layer with enforced capabilities. Only explicitly exposed commands are callable from the frontend. The backend enforces all access control and input validation. The app runs with minimal OS permissions.

## Disclosure Policy

4DA Systems follows a coordinated disclosure process:

1. Reporter submits the vulnerability to **security@4da.ai**.
2. We acknowledge receipt within 48 hours and begin assessment.
3. We work with the reporter to understand and verify the issue.
4. We develop and test a fix.
5. We release the fix and publish a security advisory.
6. The reporter may publish their findings after the fix is released, or after 90 days from the initial report, whichever comes first.

We ask that reporters refrain from public disclosure until a fix is available or the 90-day window has elapsed.

## Recognition

Security researchers who responsibly disclose valid vulnerabilities will be credited in the release changelog, unless they prefer to remain anonymous. We value the work that goes into finding and reporting security issues.

## Contact

- **Security reports:** security@4da.ai
- **General inquiries:** support@4da.ai

---

4DA Systems Pty Ltd (ACN 696 078 841) | FSL-1.1-Apache-2.0
