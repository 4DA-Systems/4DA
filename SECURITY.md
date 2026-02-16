# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting a Vulnerability

4DA handles API keys and scans local codebases. We take security seriously.

**Do NOT open a public issue for security vulnerabilities.**

Instead, email: **security@4da.dev**

Include:
- Description of the vulnerability
- Steps to reproduce
- Impact assessment
- Suggested fix (if any)

You will receive an acknowledgment within 48 hours and a detailed response within 7 days.

## Scope

The following are in scope:
- API key exposure or leakage
- Local file access beyond configured directories
- SQLite injection in the database layer
- CSP bypasses in the WebView
- MCP server unauthorized access patterns
- Dependencies with known CVEs

## Security Design

- All data stays on the user's machine. No telemetry, no cloud sync.
- API keys are stored in `data/settings.json` (local, gitignored).
- The MCP server is designed for local-only use and has no authentication.
- CSP is enforced on the WebView to prevent script injection.
- The app runs with minimal OS permissions via Tauri's capability system.
