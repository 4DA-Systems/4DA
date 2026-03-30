# Network Transparency

**4DA Systems Pty Ltd** | **Version:** 1.0.0 | **Last updated:** 2026-03-30

This document lists every outbound network connection the 4DA application makes. Every claim is backed by a source code reference you can audit yourself. If the application makes a connection not listed here, it is a bug -- report it to security@4da.ai.

---

## How to Verify

You do not need to trust this document. Verify it yourself:

### Monitor Network Traffic

| Platform | Tool | Command / Setup |
|----------|------|-----------------|
| Windows | Wireshark | Filter: `ip.addr != 127.0.0.1` |
| Windows | Fiddler | Enable HTTPS decryption |
| macOS | Little Snitch | Per-app connection logging |
| macOS | Wireshark | Filter: `ip.addr != 127.0.0.1` |
| Linux | Wireshark | Filter: `ip.addr != 127.0.0.1` |
| Linux | tcpdump | `tcpdump -i any -n not host 127.0.0.1` |
| Any | mitmproxy | HTTPS interception proxy |

### Firewall Test

Block all outbound traffic from 4DA and use the app. With the free tier and Ollama (local AI), the app works fully offline after initial content fetching. If something breaks, the failing connection is one the app depends on -- and it should be listed below.

### Grep the Source Code

```bash
# Find all HTTP client usage in Rust
grep -rn "reqwest\|HttpClient\|http_client" src-tauri/src/ --include="*.rs"

# Find all outbound URLs
grep -rn "https://" src-tauri/src/ --include="*.rs" | grep -v "//.*https://"

# Confirm zero analytics SDKs
grep -rn "mixpanel\|segment\|amplitude\|google.analytics\|hotjar\|sentry\|bugsnag\|datadog" src-tauri/src/
# Expected result: no matches
```

---

## Complete Connection Inventory

Every outbound connection, in one table:

| # | Destination | Trigger | Data Sent | Auth | Source File |
|---|-------------|---------|-----------|------|-------------|
| 1 | `hacker-news.firebaseio.com` | Auto (5 min) | GET request | None | `src-tauri/src/sources/hackernews.rs` |
| 2 | `api.github.com` | Auto (1 hr) | GET search + README | Optional PAT | `src-tauri/src/sources/github.rs` |
| 3 | `export.arxiv.org` | Auto (1 hr) | GET request | None | `src-tauri/src/sources/arxiv.rs` |
| 4 | `api.twitter.com` (v2) | Auto (15 min) | GET request | Bearer token (BYOK) | `src-tauri/src/sources/twitter.rs` |
| 5 | `www.reddit.com` | Auto | GET request per subreddit | None | `src-tauri/src/sources/reddit.rs` |
| 6 | `dev.to/api` | Auto | GET request | None | `src-tauri/src/sources/devto.rs` |
| 7 | `github.com` (CVE advisories) | Auto | GET advisory API | None | `src-tauri/src/sources/github.rs` |
| 8 | `lobste.rs` | Auto | GET request | None | `src-tauri/src/sources/lobsters.rs` |
| 9 | YouTube feeds | Auto | GET request | None | `src-tauri/src/sources/youtube.rs` |
| 10 | Product Hunt feeds | Auto | GET request | None | `src-tauri/src/sources/producthunt.rs` |
| 11 | RSS/Atom feed URLs | Auto | GET request | None | `src-tauri/src/sources/rss.rs` |
| 12 | `api.openai.com` | User-initiated | Content + system prompt | API key (BYOK) | `src-tauri/src/llm.rs` |
| 13 | `api.anthropic.com` | User-initiated | Content + system prompt | API key (BYOK) | `src-tauri/src/llm.rs` |
| 14 | Custom OpenAI-compatible URL | User-initiated | Content + system prompt | API key (BYOK) | `src-tauri/src/llm.rs` |
| 15 | `api.openai.com/v1/embeddings` | User-initiated | Text for embedding | API key (BYOK) | `src-tauri/src/embeddings.rs` |
| 16 | `localhost:11434` | User-initiated or fallback | Content for local AI | None | `src-tauri/src/llm.rs`, `src-tauri/src/embeddings.rs` |
| 17 | `api.keygen.sh` | License activation + every 6 hrs | License key only | Keygen account ID | `src-tauri/src/settings/license.rs` |
| 18 | `github.com/.../latest.json` | Auto (periodic) | GET version check | None | `src-tauri/tauri.conf.json` (Tauri updater) |
| 19 | Team Relay server | User-enabled | E2E encrypted blobs | Team key | `src-tauri/src/team_sync_commands.rs` |
| 20 | Webhook endpoints | Enterprise events | HMAC-signed payloads | HMAC-SHA256 | `src-tauri/src/webhooks.rs` |
| 21 | OIDC/JWKS endpoints | Enterprise SSO | Public key fetch | None | `src-tauri/src/sso_crypto.rs` |
| 22 | User-specified domains | Toolkit HTTP proxy | User-crafted request | User-specified | `src-tauri/src/toolkit_http.rs` |

**Total distinct connection types: 22**

---

## Detailed Breakdown

### Content Sources (Rows 1-11)

All content sources are user-configurable. Each can be enabled or disabled in the Settings panel. When disabled, zero network requests are made to that service.

**What is sent:** Standard HTTP GET requests. The only identifying information is your device's IP address, which is inherent to any network connection.

**What is NOT sent:** No user ID, no device ID, no API keys (except Twitter which requires BYOK), no personal data, no tracking parameters.

**HTTP client configuration** (`src-tauri/src/http_client.rs`):
- `HTTP_CLIENT`: 30-second timeout, 10-second connect timeout
- Connection pooling via reqwest
- TLS enforced on all external connections

**Fetch intervals** are configurable. Defaults:
- Hacker News: every 5 minutes
- GitHub, arXiv: every 1 hour
- Twitter: every 15 minutes
- Others: automatic based on source configuration

### Cloud LLM Providers (Rows 12-16)

**Critical: No LLM calls happen automatically.** LLM providers are only contacted when the user explicitly triggers analysis, synthesis, or embedding generation.

**What is sent:**
- The content being analysed (article text, search query)
- A system prompt (defines the analysis task)
- Your API key in the request header

**What is NOT sent:** Your local database, your project files, your settings, your browsing history, device identifiers.

**Ollama (Row 16)** runs entirely on your machine at `localhost:11434`. When Ollama is your provider, zero data leaves your computer for AI processing.

**Source code references:**
- LLM routing: `src-tauri/src/llm.rs`
- Embedding calls: `src-tauri/src/embeddings.rs`
- Timeout: 60 seconds (LLM), 120 seconds (Ollama)

### License Validation (Row 17)

**Only applies to Signal tier.** Free tier users make zero calls to Keygen.

- **Endpoint:** `https://api.keygen.sh/v1/accounts/{KEYGEN_ACCOUNT_ID}/licenses/actions/validate-key`
- **Method:** POST
- **Data sent:** License key only
- **Data NOT sent:** Name, email, device ID, usage data, or any personal information
- **Frequency:** At activation, then re-validated every 6 hours
- **Offline support:** Results cached locally for 7 days. Offline Ed25519 verification also available.
- **Source:** `src-tauri/src/settings/license.rs` (around line 587)

### Auto-Updates (Row 18)

The Tauri updater checks for new application versions by fetching a JSON manifest from GitHub Releases.

- **Endpoint:** `https://github.com/runyourempire/4DA/releases/latest/download/latest.json`
- **Data sent:** Standard GET request. No personal data, no device identifiers, no telemetry.
- **Security:** Updates are signed with Minisign. The public key (ID: `19AF42B1B6971703`) is embedded in `src-tauri/tauri.conf.json` (line 48). The updater rejects any payload that fails signature verification.
- **Configuration:** `src-tauri/tauri.conf.json`, lines 44-49

### Team Sync (Row 19) -- Enterprise Only

Only active when the user explicitly enables team sync.

- **Encryption:** XChaCha20Poly1305 with X25519 key exchange and HKDF key derivation
- **Architecture:** Zero-knowledge relay. The server handles only encrypted blobs and cannot read, decrypt, or inspect any content.
- **Key management:** Private keys generated on-device and never transmitted. Zeroized from memory after use.
- **Source:** `src-tauri/src/team_sync_commands.rs` (around lines 435-644)
- **HTTP client:** Dedicated `TEAM_CLIENT` with 15-second timeout

### Webhooks (Row 20) -- Enterprise Only

Webhooks fire only for enterprise team events and only to endpoints the administrator configures.

- **Authentication:** HMAC-SHA256 signature in `X-4DA-Signature-256` header
- **Retry strategy:** Exponential backoff (1m, 5m, 30m, 2h, 12h)
- **Circuit breaker:** Auto-disables after 10 consecutive failures
- **Timeout:** 10 seconds
- **Source:** `src-tauri/src/webhooks.rs` (around line 370)

### SSO/OIDC (Row 21) -- Enterprise Only

Only active when enterprise SSO is configured.

- **Endpoints:** OIDC discovery (`{issuer}/.well-known/openid-configuration`) and JWKS
- **Data sent:** Public key fetch only. No user credentials transmitted through 4DA.
- **Caching:** JWKS cached for 1 hour
- **Source:** `src-tauri/src/sso_crypto.rs` (around line 40)

### Toolkit HTTP Proxy (Row 22)

A developer tool for testing API calls. The user explicitly constructs and sends each request.

- **Domain allowlist:** Only permits requests to: OpenAI, Anthropic, GitHub, Keygen, X, arXiv, YouTube, and localhost. Requests to other domains are blocked.
- **Purpose:** Prevents the toolkit from being used as a data exfiltration vector
- **Source:** `src-tauri/src/toolkit_http.rs` (around line 89)

---

## What Does NOT Exist

The following services, SDKs, and patterns are **not present** in the 4DA codebase:

| Category | Absent Services |
|----------|-----------------|
| Analytics | Mixpanel, Segment, Amplitude, Google Analytics, Hotjar, Heap |
| Crash reporting | Sentry, Bugsnag, Crashlytics, Datadog |
| A/B testing | LaunchDarkly, Optimizely, Split |
| Advertising | No ad SDKs of any kind |
| Device fingerprinting | No fingerprinting libraries |
| Remote logging | No external log shipping |
| Tracking pixels | None |
| User accounts | No auth service, no OAuth to 4DA Systems |

**Verification:**

```bash
grep -rn "mixpanel\|segment\|amplitude\|google.analytics\|hotjar\|sentry\|bugsnag\|crashlytics\|datadog\|launchdarkly\|optimizely\|fingerprint" src-tauri/src/ src/
# Expected: no matches
```

### Local Telemetry

`src-tauri/src/telemetry.rs` does track user events (searches, feature usage, navigation) -- but this data is stored **exclusively** in the local SQLite database. There is no function, endpoint, or mechanism to transmit this data externally.

**Verification:**

```bash
# Confirm telemetry.rs contains no HTTP calls
grep -n "reqwest\|http_client\|fetch\|HttpClient" src-tauri/src/telemetry.rs
# Expected: no matches
```

Users can clear all local telemetry at any time via the `clear_telemetry()` command.

---

## CSP Enforcement

The Content Security Policy is defined in `src-tauri/tauri.conf.json` (line 35) and enforced by the WebView rendering engine:

```
default-src 'self';
script-src 'self';
style-src 'self' 'unsafe-inline';
img-src 'self' data:;
connect-src 'self'
  https://api.anthropic.com
  https://api.openai.com
  http://localhost:11434
  https://hacker-news.firebaseio.com
  https://export.arxiv.org
  https://www.reddit.com
  https://api.github.com;
font-src 'self' data:;
frame-src 'none';
object-src 'none';
base-uri 'self'
```

**What this means:** The browser engine inside 4DA will **block** any frontend JavaScript from making connections to domains not in the `connect-src` list. This is a hard technical restriction, not a policy.

Note: Rust backend HTTP calls (via reqwest) are not restricted by CSP -- CSP only governs the frontend WebView. Backend calls are auditable via the source code references in this document.

---

## How to Run Fully Offline

4DA can operate with zero internet connectivity:

1. **Use the free tier** -- no license validation calls to Keygen
2. **Use Ollama** as your AI provider -- runs at `localhost:11434`, all processing local
3. **Disable all content sources** in Settings -- no fetch calls to HN, GitHub, Reddit, etc.
4. **Disable auto-updates** -- or simply firewall the app

In this configuration, 4DA makes **zero outbound network connections**. All intelligence features work using locally cached content and local AI.

For partial offline operation: fetch content while online, then disconnect. Cached content remains available, and Ollama provides local AI analysis.

---

## HTTP Clients

4DA uses three purpose-built HTTP clients, all defined in `src-tauri/src/http_client.rs`:

| Client | Timeout | Connect Timeout | Purpose |
|--------|---------|-----------------|---------|
| `HTTP_CLIENT` | 30s | 10s | General-purpose: sources, license, webhooks |
| `PROBE_CLIENT` | 15s | 5s | Quick health checks: API validation, Ollama status |
| `TEAM_CLIENT` | 15s | 5s | Team sync relay operations |

All clients:
- Use TLS for external connections
- Share a connection pool
- Have explicit timeouts (no indefinite hangs)
- Use the `reqwest` crate (Rust HTTP client)

---

## Signal Terminal (Inbound Only)

4DA runs a local HTTP server at `127.0.0.1:4444` (production) for its own UI. This is **inbound only** -- it does not make outbound connections.

- **Binding:** localhost only (not accessible from other machines)
- **Authentication:** `X-4DA-Token` header required for API routes
- **No CORS headers:** Prevents cross-origin access from other websites
- **Source:** `src-tauri/src/signal_terminal.rs`

---

## Questions or Concerns

- **General:** support@4da.ai
- **Security reports:** security@4da.ai
- **Privacy:** privacy@4da.ai

If you find a network connection not listed in this document, please report it as a security issue. We take undocumented connections seriously.

---

4DA Systems Pty Ltd (ACN 696 078 841) | FSL-1.1-Apache-2.0 | [Source Code](https://github.com/runyourempire/4DA)
