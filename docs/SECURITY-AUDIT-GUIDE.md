# 4DA Security Audit Guide

**Version:** 1.0.0
**Last updated:** 2026-03-30
**Applies to:** 4DA v1.0.0 (commit history on `main`)

This document maps every trust-critical code path in the 4DA codebase so that security auditors and technically minded users can verify 4DA's privacy and security claims without reading 300+ Rust modules.

---

## 1. Purpose

4DA claims to be privacy-first, local-first, and zero-telemetry. This guide exists so you do not have to take our word for it. It provides:

- A complete inventory of every outbound network connection the application makes
- The exact files and line ranges where API keys are handled
- Verification commands you can run yourself to confirm each claim
- A description of what a vulnerability would look like in each area

**Who this is for:** Security researchers, enterprise IT teams evaluating 4DA for deployment, privacy-conscious users, and anyone performing due diligence on the codebase.

**What this is not:** Marketing material. Every file path and line number in this document is real and verifiable against the source code.

---

## 2. Architecture Overview

4DA is a [Tauri 2.0](https://v2.tauri.app/) desktop application. The mental model:

```
+---------------------------------------------------+
|  React/TypeScript Frontend (Webview)               |
|  - Runs in a system webview (WebView2/WebKit)      |
|  - Restricted by Content Security Policy            |
|  - Communicates ONLY via Tauri IPC invoke()         |
+---------------------------------------------------+
                    |  IPC Bridge
                    |  (only registered #[tauri::command] functions)
+---------------------------------------------------+
|  Rust Backend (Tauri process)                      |
|  - All network requests originate here              |
|  - All data storage managed here                    |
|  - SQLite + sqlite-vec for local database           |
|  - OS keychain for secret storage                   |
+---------------------------------------------------+
                    |
+---------------------------------------------------+
|  Local Filesystem                                  |
|  - data/4da.db (SQLite)                            |
|  - data/settings.json (non-sensitive prefs)        |
|  - OS Keychain (API keys)                          |
+---------------------------------------------------+
```

**Key architectural constraint:** The frontend is treated as untrusted. It cannot make network requests directly (CSP blocks this). All external communication passes through explicitly registered Rust command handlers.

---

## 3. Quick Audit Checklist

Ten things to verify, each with a single command. Run these from the repository root.

| # | Claim | Verification Command |
|---|-------|---------------------|
| 1 | Telemetry module makes zero network calls | `grep -rn "reqwest\|fetch\|HttpClient\|\.send()" src-tauri/src/telemetry.rs` (expect: no matches) |
| 2 | API keys are redacted in Debug output | `grep -n "REDACTED" src-tauri/src/settings/types.rs` (expect: lines 40, 49, 293, 571) |
| 3 | API keys are zeroized on drop | `grep -n "zeroize" src-tauri/src/settings/types.rs` (expect: lines 9, 59, 60) |
| 4 | CSP blocks arbitrary connect-src | `grep "connect-src" src-tauri/tauri.conf.json` (expect: explicit allowlist, no wildcards) |
| 5 | No `frame-src` allowed | `grep "frame-src" src-tauri/tauri.conf.json` (expect: `frame-src 'none'`) |
| 6 | Toolkit HTTP has domain allowlist | `grep -A 25 "ALLOWED_DOMAINS" src-tauri/src/toolkit_http.rs` (expect: ~20 specific domains) |
| 7 | Data export strips sensitive fields | `grep -A 20 "SENSITIVE_KEYS" src-tauri/src/data_export.rs` (expect: 18 key patterns) |
| 8 | Team crypto uses XChaCha20Poly1305 | `grep "XChaCha20Poly1305" src-tauri/src/team_sync_crypto.rs` (expect: import and usage) |
| 9 | Private keys zeroized on drop | `grep -A 5 "impl Drop" src-tauri/src/team_sync_crypto.rs` (expect: `key.zeroize()`) |
| 10 | Update signatures verified | `grep "pubkey" src-tauri/tauri.conf.json` (expect: base64-encoded Minisign public key) |

---

## 4. Trust-Critical Code Map

### 4.1 API Key Storage and Handling

**Files:**
- `src-tauri/src/settings/types.rs` -- Settings struct definitions, Debug redaction, zeroize
- `src-tauri/src/settings/keystore.rs` -- Platform keychain integration (199 lines)
- `src-tauri/src/settings/` -- Full settings management module

**How it works:**

API keys are stored in the OS platform keychain under the service name `com.4da.app`:
- **Windows:** Credential Manager
- **macOS:** Keychain
- **Linux:** Secret Service (GNOME Keyring / KWallet)

Four key names are managed (see `keystore.rs` line 16):
```
llm_api_key, openai_api_key, x_api_key, license_key
```

**Security measures implemented:**

1. **Zeroize on drop** (`types.rs` lines 57-62): `LLMProvider` implements `Drop` which calls `.zeroize()` on `api_key` and `openai_api_key` fields, overwriting memory with zeros when the struct is deallocated.

2. **Debug redaction** (`types.rs` lines 31-55): Custom `Debug` impl for `LLMProvider` prints `[REDACTED]` instead of key values. Same for `LicenseConfig` (lines 286-302) and the top-level `Settings` struct (lines 552-589) which redacts `x_api_key`.

3. **Automatic migration** (`keystore.rs` lines 127-198): `migrate_from_plaintext()` moves keys from `settings.json` to the OS keychain. After migration, plaintext fields are cleared.

4. **API error sanitization** (`llm.rs` lines 29-57): `sanitize_api_error()` redacts strings matching API key patterns (`sk-*`, `pk_*`, long alphanumeric tokens) from error messages before they reach logs or the frontend.

**What a vulnerability would look like:**
- A code path that logs `settings.llm.api_key` directly (bypassing the Debug impl)
- A serialization path that writes keys to disk without redaction
- A frontend-exposed command that returns raw key values

**Verification:**
```bash
# Confirm no raw key logging (should return only REDACTED references)
grep -rn "api_key" src-tauri/src/ --include="*.rs" | grep -i "log\|info!\|warn!\|debug!\|error!\|tracing" | grep -v "REDACTED\|redact\|mask\|strip\|sanitize\|key_name\|test\|//\|doc"

# Confirm zeroize dependency is used
grep "zeroize" src-tauri/Cargo.toml
```

---

### 4.2 Network Boundary (All Outbound Connections)

This is the most critical section. Every byte that leaves the machine originates from one of the paths listed below. There are no others.

**HTTP Client definitions** (`src-tauri/src/http_client.rs`, 63 lines):

| Client | Purpose | Timeout |
|--------|---------|---------|
| `HTTP_CLIENT` | General-purpose (license validation, API probes) | 30s |
| `PROBE_CLIENT` | Health checks, status probes | 15s |
| `TEAM_CLIENT` | Team relay operations | 15s |

Additional purpose-built clients exist in specific modules (documented in `http_client.rs` lines 8-17): `embeddings.rs` (90s), `llm.rs` (120s), `webhooks.rs` (10s), and others.

**Complete inventory of outbound network destinations:**

| Module | File | Destination | Purpose | User-initiated? |
|--------|------|-------------|---------|-----------------|
| Source adapters | `src-tauri/src/sources/hackernews.rs` | `hacker-news.firebaseio.com` | Fetch HN stories | Automatic (monitoring) |
| Source adapters | `src-tauri/src/sources/reddit.rs` | `www.reddit.com` | Fetch Reddit posts | Automatic (monitoring) |
| Source adapters | `src-tauri/src/sources/github.rs` | `api.github.com` | Fetch trending repos, CVEs | Automatic (monitoring) |
| Source adapters | `src-tauri/src/sources/arxiv.rs` | `export.arxiv.org` | Fetch arXiv papers | Automatic (monitoring) |
| Source adapters | `src-tauri/src/sources/rss.rs` | User-configured RSS URLs | Fetch RSS feeds | User-configured |
| Source adapters | `src-tauri/src/sources/twitter.rs` | `api.x.com` | Fetch tweets (BYOK) | User-configured |
| Source adapters | `src-tauri/src/sources/youtube.rs` | `www.youtube.com` | Fetch channel RSS | User-configured |
| Source adapters | `src-tauri/src/sources/lobsters.rs` | `lobste.rs` | Fetch Lobsters stories | Automatic (monitoring) |
| Source adapters | `src-tauri/src/sources/devto.rs` | `dev.to` | Fetch Dev.to articles | Automatic (monitoring) |
| Source adapters | `src-tauri/src/sources/producthunt.rs` | `www.producthunt.com` | Fetch PH posts | Automatic (monitoring) |
| Source adapters | `src-tauri/src/sources/cve.rs` | `api.github.com` (GHSA) | Fetch security advisories | Automatic (monitoring) |
| LLM provider | `src-tauri/src/llm.rs` | `api.anthropic.com` | Anthropic Claude API | User-configured (BYOK) |
| LLM provider | `src-tauri/src/llm.rs` | `api.openai.com` | OpenAI GPT API | User-configured (BYOK) |
| LLM provider | `src-tauri/src/llm.rs` | `localhost:11434` | Ollama (local) | User-configured |
| Embeddings | `src-tauri/src/embeddings.rs` | `api.openai.com` or `localhost:11434` | Text embeddings | Automatic (indexing) |
| License validation | `src-tauri/src/settings/license.rs` | `api.keygen.sh` | Validate license key | On activation + periodic |
| App updates | Tauri updater plugin | `github.com/.../releases` | Check for updates | Automatic (configurable) |
| Team relay | `src-tauri/src/team_sync_commands.rs` | User-configured relay URL | Encrypted metadata sync | User-configured (opt-in) |
| Webhooks | `src-tauri/src/webhooks.rs` | Admin-configured URLs | Enterprise event webhooks | Admin-configured |
| SSO/OIDC | `src-tauri/src/sso_crypto.rs` | IdP discovery endpoints | JWKS key fetch | Enterprise SSO |
| Digest email | Uses `lettre` SMTP | User-configured SMTP server | Send digest emails | User-configured |

**What is NOT in this list (and should never be):**
- Telemetry endpoints
- Analytics services
- Crash reporters
- Advertising networks
- 4DA Systems servers (beyond GitHub releases for updates)

**What a vulnerability would look like:**
- A `reqwest` call in `telemetry.rs` (there are currently zero)
- An HTTP client sending data to a domain not in the CSP allowlist
- User content (article text, search queries, project file contents) sent to any endpoint other than the user-configured LLM provider

**Verification:**
```bash
# Find ALL outbound HTTP calls in the entire Rust codebase
grep -rn "\.send().await\|\.post(\|\.get(" src-tauri/src/ --include="*.rs" | grep -v "test\|#\[cfg(test)\]" | grep -v "//" | sort

# Confirm telemetry.rs has zero network calls
grep -c "reqwest\|HttpClient\|\.send()" src-tauri/src/telemetry.rs
# Expected output: 0

# List all reqwest usage by file
grep -rl "reqwest" src-tauri/src/ --include="*.rs" | sort
```

---

### 4.3 IPC Boundary (Frontend to Backend)

**How Tauri IPC works:**

The frontend can only call Rust functions that are explicitly registered in the `invoke_handler`. This registration happens in `src-tauri/src/lib.rs` starting at line 471.

The complete command list spans lines 471-891 (~420 lines of command registrations). Every `invoke()` call from the frontend must match one of these registered handlers. Unregistered calls are silently rejected by Tauri.

**Capability system** (`src-tauri/capabilities/default.json`):

```json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default",
    "notification:default",
    "deep-link:default",
    "updater:default"
  ]
}
```

This restricts the main window to only the listed Tauri plugin permissions. No filesystem access, no shell access, no clipboard access beyond what these plugins provide.

**What a vulnerability would look like:**
- A `#[tauri::command]` function that accepts arbitrary SQL or shell commands
- A command that returns raw file contents without path validation
- A registered command that was intended to be internal-only

**Verification:**
```bash
# Count total registered IPC commands
grep -c "::" src-tauri/src/lib.rs | head -1
# Or more precisely, count lines in the generate_handler block:
sed -n '/generate_handler/,/\]/p' src-tauri/src/lib.rs | grep "::" | wc -l

# List all registered commands
sed -n '/generate_handler/,/\]/p' src-tauri/src/lib.rs | grep "::" | sed 's/.*:://' | sed 's/,.*//' | sort

# Find all #[tauri::command] functions across the codebase
grep -rn "#\[tauri::command\]" src-tauri/src/ --include="*.rs" | wc -l
# Compare with the generate_handler count to detect orphaned commands
```

---

### 4.4 Content Security Policy

**File:** `src-tauri/tauri.conf.json` line 35

The CSP restricts what the webview can do at the browser engine level:

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

**Key restrictions:**
- `frame-src: 'none'` -- No iframes allowed (blocks clickjacking and embedding attacks)
- `object-src: 'none'` -- No Flash/Java/plugin embedding
- `script-src: 'self'` -- No inline scripts, no external script sources
- `connect-src` -- Explicit allowlist of external domains (only LLM providers and source APIs)
- `base-uri: 'self'` -- Prevents base tag injection

**Note on `style-src 'unsafe-inline'`:** This is common in Tauri/React apps where CSS-in-JS is used. It does not enable script injection because `script-src` is restricted to `'self'`.

**What a vulnerability would look like:**
- `connect-src *` or `connect-src https:` (wildcard allowing connections to any HTTPS domain)
- `script-src 'unsafe-eval'` or `script-src 'unsafe-inline'` (enabling script injection)
- Removal of `frame-src 'none'`

**Verification:**
```bash
# Extract and inspect the full CSP
grep "csp" src-tauri/tauri.conf.json

# Confirm no wildcards in connect-src
grep "connect-src" src-tauri/tauri.conf.json | grep -c "\*"
# Expected: 0
```

---

### 4.5 Data Storage

All persistent data is stored locally. There is no cloud database, no remote sync (except opt-in Team Relay which is E2E encrypted), and no user accounts.

| Storage | Location | Contents | Sensitivity |
|---------|----------|----------|-------------|
| SQLite database | `data/4da.db` | Content items, scores, telemetry events, user preferences, team data | Medium (user's content selections) |
| Settings file | `data/settings.json` | Configuration, thresholds, feed URLs, non-sensitive prefs | Low (no secrets after keychain migration) |
| OS Keychain | Platform-specific | API keys, license keys | High |
| Exported data | User-chosen path | GDPR data exports (redacted) | Medium |

**Database is never synced remotely.** The only module that touches remote storage is Team Relay (`team_sync_commands.rs`), which encrypts all data client-side before transmission.

**Verification:**
```bash
# Confirm settings.json is gitignored
grep "settings.json" .gitignore

# Confirm database files are gitignored
grep "4da.db" .gitignore

# List all SQLite operations that write to disk
grep -rn "open_db_connection\|rusqlite::Connection::open" src-tauri/src/ --include="*.rs" | head -20
```

---

### 4.6 Telemetry (Local Only)

**File:** `src-tauri/src/telemetry.rs` (541 lines)

The telemetry module records usage events (search queries, view navigation, feature adoption) into the local `user_events` SQLite table. It also tracks application errors in `error_telemetry`.

**Critical claim: This module makes zero outbound network calls.**

The file imports:
- `rusqlite` (SQLite)
- `serde` (serialization)
- `tracing` (logging)
- `crate::error` (error types)

It does NOT import `reqwest`, `hyper`, `tokio::net`, or any HTTP/network crate.

**User control:**
- `clear_telemetry` command (line 304): Deletes all telemetry data
- `clear_error_telemetry` command (line 537): Prunes error records
- All data visible via `get_usage_analytics` and `get_error_telemetry` commands

**What a vulnerability would look like:**
- Any `use reqwest` or `use hyper` added to this file
- A function that serializes telemetry data and passes it to another module that transmits it
- A background task that reads from `user_events` and sends data externally

**Verification:**
```bash
# Confirm zero network imports
grep -c "reqwest\|hyper\|HttpClient\|TcpStream\|fetch" src-tauri/src/telemetry.rs
# Expected: 0

# Confirm zero network calls
grep -c "\.send()\|\.post(\|\.get(" src-tauri/src/telemetry.rs
# Expected: 0

# Inspect all imports in the file
head -10 src-tauri/src/telemetry.rs
```

---

### 4.7 Team Relay Encryption

**Files:**
- `src-tauri/src/team_sync_crypto.rs` -- Cryptographic primitives
- `src-tauri/src/team_sync_commands.rs` -- Tauri command handlers
- `src-tauri/src/team_sync.rs` -- Sync logic

**Encryption stack:**
- **Symmetric encryption:** XChaCha20Poly1305 (authenticated encryption with 24-byte nonce)
- **Key exchange:** X25519 Diffie-Hellman
- **Key derivation:** HKDF-SHA256
- **Nonce size:** 24 bytes (see `team_sync_crypto.rs` line 25)

**Key hierarchy:**
1. Each team member generates an X25519 keypair locally (`TeamCrypto::generate()`, line 71)
2. The team admin generates a symmetric team key
3. The team key is encrypted per-member using X25519 for distribution
4. All sync entries are encrypted with the team symmetric key

**Security measures:**
- Private keys never leave the device (`our_private_key` field is not `pub`)
- Custom `Debug` impl prints `[REDACTED]` for private key (line 49-59)
- `Drop` impl calls `zeroize()` on `team_key` (lines 41-47)
- The relay server receives only encrypted blobs and cannot decrypt them

**What a vulnerability would look like:**
- `team_key` or `our_private_key` appearing in a log statement
- The relay server receiving unencrypted metadata
- Nonce reuse (using a fixed or predictable nonce instead of random generation)

**Verification:**
```bash
# Confirm XChaCha20Poly1305 usage
grep "XChaCha20Poly1305" src-tauri/src/team_sync_crypto.rs

# Confirm zeroize on drop
grep -A 6 "impl Drop for TeamCrypto" src-tauri/src/team_sync_crypto.rs

# Confirm private key is not pub
grep "our_private_key" src-tauri/src/team_sync_crypto.rs
# Expected: field declaration without `pub`, and [REDACTED] in Debug

# Confirm nonce generation uses OsRng (cryptographic randomness)
grep "OsRng" src-tauri/src/team_sync_crypto.rs
```

---

### 4.8 Data Export and Redaction

**File:** `src-tauri/src/data_export.rs` (~935 lines)

GDPR-style data export available on all tiers (not feature-gated). Before writing any export, sensitive fields are recursively stripped from all JSON data.

**Sensitive field list** (lines 68-88):
```
api_key, apiKey, api_keys, token, secret, password,
private_key, privateKey, access_token, accessToken,
refresh_token, refreshToken, secret_key, secretKey,
x_api_key, openai_key, anthropic_key, groq_key, openrouter_key
```

**How stripping works** (`strip_sensitive_fields`, lines 91-118):
- Recursively traverses all JSON objects and arrays
- Case-insensitive matching against the sensitive key list
- Replaces matched values with the string `"[REDACTED]"`

**What a vulnerability would look like:**
- A new key field name (e.g., `bearer_token`) that is not in the `SENSITIVE_KEYS` list
- A code path that exports data without calling `strip_sensitive_fields`
- A non-JSON export format that bypasses the JSON-based stripping

**Verification:**
```bash
# List all sensitive key patterns
grep -A 20 "SENSITIVE_KEYS" src-tauri/src/data_export.rs

# Confirm strip_sensitive_fields is called before every write
grep -n "strip_sensitive_fields" src-tauri/src/data_export.rs

# Run the existing test suite for data export
cd src-tauri && cargo test data_export --lib
```

---

### 4.9 Update Mechanism

**Configuration:** `src-tauri/tauri.conf.json` lines 44-49

```json
"updater": {
  "endpoints": [
    "https://github.com/runyourempire/4DA/releases/latest/download/latest.json"
  ],
  "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIH..."
}
```

Updates are fetched from GitHub Releases and signed with Minisign. The public key is embedded in the application binary at compile time. The Tauri updater plugin verifies the cryptographic signature before applying any update.

**What a vulnerability would look like:**
- The `pubkey` field being empty or containing a weak key
- An additional endpoint pointing to a non-GitHub domain
- A code path that bypasses signature verification

**Verification:**
```bash
# Decode and inspect the public key
echo "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDE5QUY0MkIxQjY5NzE3MDMKUldRREY1ZTJzVUt2R1lDUHhrYS9LYXpPWTZzLzh3ODV0SzdDOHJENklSQWIxdWNPaFZmZVBSWkYK" | base64 -d
# Expected: Minisign public key with ID 19AF42B1B6971703

# Confirm only GitHub endpoints
grep -A 5 "endpoints" src-tauri/tauri.conf.json
```

---

### 4.10 Toolkit HTTP Proxy

**File:** `src-tauri/src/toolkit_http.rs`

The developer toolkit includes an HTTP probe feature that lets users test API endpoints. To prevent abuse as an open proxy, it enforces a strict domain allowlist.

**Allowed domains** (lines 50-72):
```
api.openai.com, api.anthropic.com, generativelanguage.googleapis.com,
localhost, 127.0.0.1, 0.0.0.0,
api.keygen.sh,
hacker-news.firebaseio.com, www.reddit.com, oauth.reddit.com,
api.github.com, api.x.com, export.arxiv.org,
www.youtube.com, lobste.rs, dev.to, www.producthunt.com
```

The `is_domain_allowed()` function (line 75) parses the URL and checks the hostname against this list before permitting any request.

**What a vulnerability would look like:**
- A bypass in `is_domain_allowed()` (e.g., using URL encoding, IP address forms, or DNS rebinding)
- The allowlist containing a wildcard or overly broad domain

**Verification:**
```bash
# Inspect the domain validation function
grep -A 20 "fn is_domain_allowed" src-tauri/src/toolkit_http.rs

# Confirm no wildcard entries
grep "ALLOWED_DOMAINS" -A 25 src-tauri/src/toolkit_http.rs | grep "\*"
# Expected: no output
```

---

### 4.11 Enterprise Webhooks

**File:** `src-tauri/src/webhooks.rs`

Enterprise webhook delivery uses HMAC-SHA256 signed payloads. Each webhook has a unique secret stored in the local database.

**Signing** (line 69 imports `hmac` + `sha2`): Payloads are signed with `X-4DA-Signature-256: sha256={signature}` header. The signature is computed using the webhook's secret key, preventing payload tampering if the delivery URL is intercepted.

**Circuit breaker:** After 10 consecutive failures (`CIRCUIT_BREAKER_THRESHOLD`, line 45), a webhook is automatically disabled to prevent infinite retry loops.

**Retry backoff** (line 43): 1min, 5min, 30min, 2hr, 12hr. Maximum 5 retries before marking delivery as exhausted.

---

### 4.12 License Validation

**File:** `src-tauri/src/settings/license.rs`

License validation calls the Keygen API (`api.keygen.sh`) to verify license keys. This is the only connection to a service related to 4DA's business operations.

**What is sent:** Only the license key itself (line 577-581).
**What is NOT sent:** No machine fingerprint, no usage data, no user identity, no telemetry.

**Rate limiting:** Maximum 5 activation attempts per minute (line 29).
**Caching:** Successful validations are cached for 168 hours (7 days) to support offline operation (line 26).
**Re-validation:** Every 6 hours to prevent `settings.json` manipulation (line 76).

**Verification:**
```bash
# Inspect exactly what is sent to Keygen
grep -A 10 "serde_json::json" src-tauri/src/settings/license.rs | head -15
# Expected: only "meta.key" field

# Confirm the only external URL
grep "keygen.sh" src-tauri/src/settings/license.rs
```

---

### 4.13 Community Intelligence

**File:** `src-tauri/src/community_intelligence.rs`

Opt-in feature (disabled by default, see `CommunityIntelligenceConfig::default()` at line 24: `enabled: false`). When enabled, shares anonymous scoring patterns -- never content, URLs, identity, preferences, or tech stack (as stated in the module docstring, lines 1-4).

**Verification:**
```bash
# Confirm opt-in default
grep -A 5 "impl Default for CommunityIntelligenceConfig" src-tauri/src/community_intelligence.rs
```

---

### 4.14 Waitlist Signups

**File:** `src-tauri/src/waitlist.rs`

Stores Team/Enterprise tier interest signups in the local SQLite database. Makes zero network calls (confirmed by grep).

**Verification:**
```bash
grep -c "reqwest\|fetch\|\.send()" src-tauri/src/waitlist.rs
# Expected: 0
```

---

## 5. What to Look For

Common vulnerability patterns specific to this architecture:

### 5.1 Tauri IPC Injection
- **Pattern:** A `#[tauri::command]` function that accepts user input and passes it to `conn.execute()` without parameterized queries.
- **How to check:** `grep -rn "execute(" src-tauri/src/ --include="*.rs" | grep -v "params!\|params\[" | grep -v "execute_batch\|test\|//"` -- any result that interpolates user input into SQL is a vulnerability.

### 5.2 Secret Leakage via Logging
- **Pattern:** A `tracing::info!` or `tracing::debug!` that includes a struct containing API keys without using the custom Debug impl.
- **How to check:** `grep -rn "tracing::\|info!\|debug!\|warn!" src-tauri/src/ --include="*.rs" | grep -i "api_key\|secret\|token\|password" | grep -v "REDACTED\|redact\|mask\|strip\|key_name\|target\|sanitize"` -- review any results manually.

### 5.3 Unguarded File Access
- **Pattern:** A Tauri command that reads or writes arbitrary file paths based on frontend input without validation.
- **How to check:** `grep -rn "std::fs::\|tokio::fs::" src-tauri/src/ --include="*.rs" | grep -v "test\|example"` -- review for path traversal.

### 5.4 CSP Bypass via Dynamic Content
- **Pattern:** Frontend code that inserts user-controlled content into the DOM without sanitization (XSS).
- **How to check:** `grep -rn "dangerouslySetInnerHTML\|innerHTML\|v-html" src/ --include="*.tsx" --include="*.ts"` -- each result needs manual review.

### 5.5 Dependency Confusion / Supply Chain
- **Pattern:** A dependency that is not pinned to a specific version, or a dependency pulled from a non-standard registry.
- **How to check:** Verify `Cargo.lock` and `pnpm-lock.yaml` exist and are committed. Check that no `[patch]` or `[replace]` sections in `Cargo.toml` point to unexpected URLs.

### 5.6 Nonce/IV Reuse in Cryptography
- **Pattern:** Reusing a nonce in XChaCha20Poly1305 encryption.
- **How to check:** `grep -n "XNonce\|nonce\|Nonce" src-tauri/src/team_sync_crypto.rs` -- verify nonces are generated from `OsRng` (cryptographic randomness), not from counters or timestamps.

---

## 6. Automated Verification

### Rust Backend

```bash
cd src-tauri

# Clippy lints (includes security-relevant warnings)
cargo clippy --all-targets -- -D warnings

# Run the full test suite (2215+ tests)
cargo test --lib

# Check for known vulnerable dependencies
cargo audit
# (requires: cargo install cargo-audit)

# Check for unsafe code blocks
grep -rn "unsafe " src/ --include="*.rs" | grep -v "// unsafe\|test\|example"
```

### Frontend

```bash
# ESLint (includes security rules)
pnpm run lint

# Run frontend tests (1173+ tests)
pnpm run test

# Audit npm dependencies for known vulnerabilities
pnpm audit
```

### Full Validation Suite

```bash
# Combined validation (file sizes, linting, tests)
pnpm run validate:all
```

### CSP Verification

```bash
# Extract and format the CSP for review
node -e "const c=require('./src-tauri/tauri.conf.json'); console.log(c.app.security.csp.split(';').map(s=>s.trim()).join('\n'))"
```

---

## 7. Third-Party Dependencies

### Rust Dependencies

All Rust dependencies are declared in `src-tauri/Cargo.toml` with exact versions pinned in `src-tauri/Cargo.lock`. Key security-relevant dependencies:

| Crate | Version | Purpose | Why it matters |
|-------|---------|---------|----------------|
| `reqwest` | 0.12 | HTTP client | All outbound network traffic |
| `rusqlite` | 0.32 (bundled) | SQLite | All local data storage |
| `chacha20poly1305` | 0.10 | Authenticated encryption | Team relay encryption |
| `x25519-dalek` | 2.x | Key exchange | Team relay key agreement |
| `keyring` | 3.x | OS keychain | API key storage |
| `zeroize` | 1.x | Memory zeroing | Secret cleanup on drop |
| `jsonwebtoken` | 9.x (optional) | JWT verification | Enterprise SSO |
| `hmac` + `sha2` | 0.12 / 0.10 | HMAC-SHA256 | Webhook signing |

**No pre-built native modules.** Pure Rust alternatives are used for:
- OCR: `ocrs` crate (optional feature flag)
- PDF: `pdf-extract` + `lopdf`
- Office documents: `docx-rs` + `calamine`

### Frontend Dependencies

All frontend dependencies are declared in `package.json` with exact versions pinned in `pnpm-lock.yaml`.

### Auditing the Dependency Tree

```bash
# Rust: check for known CVEs
cd src-tauri && cargo audit

# Rust: view the full dependency tree
cargo tree

# Rust: check a specific crate's transitive deps
cargo tree -p reqwest

# Frontend: check for known vulnerabilities
pnpm audit

# Frontend: view the dependency tree
pnpm list --depth 10
```

---

## 8. Reporting Vulnerabilities

**Do NOT open a public issue for security vulnerabilities.**

- **Email:** security@4da.ai
- **Full policy:** See `SECURITY.md` in the repository root
- **Response timeline:** 48-hour acknowledgment, 7-day assessment, 90-day resolution target
- **Scope:** API key exposure, data exfiltration, update mechanism compromise, IPC bypass, local privilege escalation, CSP bypass, exploitable dependency CVEs
- **Recognition:** Responsible disclosures are credited in release changelogs

---

## Appendix A: File Index

Quick reference for all security-relevant files:

| File | Lines | Description |
|------|-------|-------------|
| `src-tauri/tauri.conf.json` | 99 | CSP, updater config, app permissions |
| `src-tauri/capabilities/default.json` | 13 | Tauri capability permissions |
| `src-tauri/src/settings/types.rs` | ~670 | Settings struct, Debug redaction, zeroize |
| `src-tauri/src/settings/keystore.rs` | 199 | OS keychain integration |
| `src-tauri/src/settings/license.rs` | ~630 | Keygen license validation |
| `src-tauri/src/http_client.rs` | 63 | Shared HTTP client pool |
| `src-tauri/src/llm.rs` | ~400 | LLM provider routing, API error sanitization |
| `src-tauri/src/embeddings.rs` | ~300 | Embedding API calls |
| `src-tauri/src/telemetry.rs` | 541 | Local-only telemetry (zero network calls) |
| `src-tauri/src/data_export.rs` | ~935 | GDPR data export with sensitive field stripping |
| `src-tauri/src/team_sync_crypto.rs` | ~200 | XChaCha20Poly1305 + X25519 encryption |
| `src-tauri/src/team_sync_commands.rs` | ~400 | Team relay command handlers |
| `src-tauri/src/webhooks.rs` | ~640 | Enterprise webhook dispatch (HMAC-SHA256) |
| `src-tauri/src/sso_crypto.rs` | ~200 | OIDC JWKS / SAML signature verification |
| `src-tauri/src/toolkit_http.rs` | ~200 | HTTP probe with domain allowlist |
| `src-tauri/src/community_intelligence.rs` | ~200 | Opt-in pattern sharing (disabled by default) |
| `src-tauri/src/waitlist.rs` | ~60 | Local-only waitlist storage |
| `src-tauri/src/audit.rs` | ~200 | Enterprise audit log |
| `src-tauri/src/lib.rs` | ~897 | IPC handler registration (lines 471-891) |
| `src-tauri/src/sources/` | 17 files | Source adapters (all outbound content fetching) |
| `src-tauri/Cargo.toml` | ~160 | Rust dependency declarations |
| `SECURITY.md` | 106 | Vulnerability reporting policy |

---

## Appendix B: Network Audit One-Liner

To get a complete picture of every file that could make network requests:

```bash
grep -rl "reqwest\|hyper\|TcpStream\|HttpClient\|\.send().await" src-tauri/src/ --include="*.rs" | sort
```

Then for each file, verify that its network calls match the documented purposes in Section 4.2.

---

4DA Systems Pty Ltd (ACN 696 078 841) | FSL-1.1-Apache-2.0
