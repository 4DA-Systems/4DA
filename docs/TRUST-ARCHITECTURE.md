# Trust Architecture

**Why 4DA is built so that trust is unnecessary.**

Most software asks you to trust that the company won't misuse your data. 4DA is architectured so that trust is unnecessary -- your data never leaves your machine. This is not a policy promise. It is a technical fact, verifiable in the source code.

---

## 1. The Trust Problem in Software

Every application that handles your data creates an implicit contract: you hand over information, and the company promises to treat it responsibly. This model has three structural weaknesses:

**Policies can change.** A privacy policy is a legal document, not a technical constraint. It can be rewritten at any time by the same people who wrote the original. Companies are acquired, boards change, investors apply pressure, and policies quietly evolve. What was forbidden in version 1.0 becomes permissible in version 2.0.

**Policies require ongoing good faith.** Even an honest policy depends on every employee, contractor, and third-party integration respecting it in perpetuity. A single rogue database query, a misconfigured analytics pipeline, or a careless vendor integration can violate a policy without anyone noticing.

**Policies are words. Architecture is math.** A policy says "we won't send your data to our servers." An architecture that has no servers makes that statement a tautology. You cannot exfiltrate data to infrastructure that does not exist.

4DA takes the architectural approach. The question is not "will 4DA respect my privacy?" The question is "can 4DA violate my privacy?" The answer, by design, is no.

---

## 2. The 7-Layer Trust Stack

Trust is not binary. It exists in layers, each reinforcing the others. Most software companies start at Layer 4 (legal compliance) and work upward. 4DA starts at Layer 1 (philosophy) and builds through every layer.

```
Layer 7: Community       Users vouching for the product
Layer 6: Track Record    Years of consistent, honest operation
Layer 5: Transparency    Source-available code, public audits, honest communication
Layer 4: Legal           Registered company, privacy policy, Australian jurisdiction
Layer 3: Technical       Signed binaries, Minisign updates, CSP enforcement
Layer 2: Architecture    Local-first + direct-to-provider — no 4DA-operated server for your data to land in
Layer 1: Philosophy      Privacy-by-design, not privacy-by-policy
```

**Layers 1 through 5 are in place today.** Layers 6 and 7 are earned through consistent operation over time. No shortcut exists for them, and we will not pretend otherwise.

### Layer 1: Philosophy

4DA was conceived as local-first software from the first line of code. Privacy is not a feature that was added later. It is the architectural foundation that every other feature is built on top of. The application was designed from the ground up so that there is no server to send data to, no account system to correlate users, and no telemetry pipeline to aggregate behavior.

### Layer 2: Architecture

The application is a Tauri 2.0 desktop binary. Rust backend, React frontend, SQLite database. Everything runs on your machine. There is no cloud component. There is no sync service. There is no "optional" data sharing. The architecture makes data exfiltration structurally impossible without the user explicitly configuring an external API and providing their own key.

### Layer 3: Technical Controls

- **Code signing:** Windows binaries are signed with an EV code signing certificate from SSL.com. macOS binaries are signed with an Apple Developer ID certificate and notarized by Apple.
- **Update verification:** The Tauri updater verifies every update against a Minisign public key (key ID `19AF42B1B6971703`) before applying it. A compromised download server cannot push malicious updates.
- **Content Security Policy:** The CSP in `tauri.conf.json` restricts all outbound connections to an explicit allowlist. The browser context inside the application cannot contact arbitrary servers.

### Layer 4: Legal

4DA Systems Pty Ltd is a registered Australian company (ACN 696 078 841), incorporated in Queensland, Australia. We are subject to the Australian Privacy Act 1988 (Cth), the EU General Data Protection Regulation (GDPR), and the California Consumer Privacy Act (CCPA). Our Privacy Policy is published at `docs/legal/PRIVACY-POLICY.md` and on 4da.ai. Australian consumer protection law provides strong statutory guarantees that cannot be disclaimed.

### Layer 5: Transparency

The complete source code is available under the FSL-1.1-Apache-2.0 license. Anyone can read, audit, and build the application from source. The license converts to fully open Apache-2.0 after two years, ensuring the code can never be locked away. We do not operate a "source-available but you can't actually build it" model -- the repository includes pinned dependencies (`Cargo.lock`, `pnpm-lock.yaml`) and build instructions.

### Layer 6: Track Record

This layer is earned, not declared. We will build it through years of shipping updates that never add telemetry, never require accounts, and never change the architectural guarantees described in this document.

### Layer 7: Community

This layer belongs to users, not to us. When people who use 4DA begin vouching for it based on their own experience and their own verification, that will mean more than anything we could write in a document.

---

## 3. Architectural Proof Points

Each claim below includes the mechanism that enforces it and the method you can use to verify it independently.

### a) Your data stays local

**Mechanism:** All user data is stored in a SQLite database at `data/4da.db` on your local filesystem. There is no remote database, no sync endpoint, no backup service, and no cloud storage integration. The application has no server-side component that could receive data even if the client attempted to send it.

**Verify:** Monitor the application's network traffic using Wireshark, Fiddler, or your operating system's built-in network monitor. Every outbound connection will be to one of the endpoints listed in the CSP allowlist (see Section 3c). None of them are 4DA Systems servers, because 4DA Systems does not operate data-receiving servers.

### b) Zero telemetry collection

**Mechanism:** The file `src-tauri/src/telemetry.rs` implements usage analytics that are stored exclusively in the local SQLite database. Line 1 of that file:

```rust
//! Local Telemetry — privacy-first usage analytics that never leave the machine.
```

No analytics SDKs are included in the application. There is no Mixpanel, Segment, Amplitude, Google Analytics, Hotjar, PostHog, or Plausible integration.

**Verify:** Search the codebase yourself:

```bash
grep -r "mixpanel\|segment\|amplitude\|google-analytics\|hotjar\|posthog\|plausible" src-tauri/src/
```

The only results are in exclusion lists and URL validation rules -- code that explicitly rejects these services, not code that uses them.

### c) API keys never leave your machine (except to the provider you configured)

**Mechanism:** API keys are stored in your operating system's native credential manager via the `keyring` crate (`src-tauri/src/settings/keystore.rs`):

- **Windows:** Credential Manager
- **macOS:** Keychain
- **Linux:** Secret Service (GNOME Keyring / KWallet)

Keys are transmitted only to the endpoints you configure -- your chosen LLM provider (Anthropic, OpenAI) or your local Ollama instance. The Content Security Policy in `tauri.conf.json` enforces this at the browser level:

```
connect-src 'self'
  https://api.anthropic.com
  https://api.openai.com
  http://localhost:11434
  https://hacker-news.firebaseio.com
  https://export.arxiv.org
  https://www.reddit.com
  https://api.github.com
```

No 4DA Systems endpoint appears in this list. The application cannot send data to us because the CSP does not permit it.

**Verify:** Inspect the CSP directly in `src-tauri/tauri.conf.json` (line 35). Audit the LLM request code in `src-tauri/src/llm.rs`. Every outbound API call uses the key stored in your local keychain and sends it only to the provider URL you selected.

### d) The app works fully offline

**Mechanism:** When configured with Ollama (a local LLM running at `localhost:11434`), the application makes zero external network requests. Local embeddings are computed via `sqlite-vec`. Content sources can be individually disabled for full air-gap operation.

**Verify:** Disconnect your network, configure Ollama as your LLM provider, and use the free tier. The application continues to function with full intelligence capabilities.

### e) Updates are cryptographically verified

**Mechanism:** The Tauri updater plugin verifies every update against a Minisign public key before applying it. The public key (ID `19AF42B1B6971703`) is embedded in the application binary via `tauri.conf.json`. Update manifests are served from GitHub Releases, not from 4DA-controlled infrastructure.

Platform-specific signing provides an additional layer:
- **Windows:** EV code signing certificate from SSL.com
- **macOS:** Apple Developer ID certificate with Apple notarization

**Verify:** The Minisign public key is visible in `src-tauri/tauri.conf.json` (line 48). Download verification instructions are available in VERIFY-DOWNLOADS.md.

### f) The source code is auditable

**Mechanism:** The application is published under the FSL-1.1-Apache-2.0 license. This is a source-available license that permits reading, building, auditing, modifying, and redistributing the code for any non-competing purpose. After two years, the license automatically converts to Apache-2.0 with no restrictions beyond standard open-source terms.

**Verify:** The LICENSE file is in the repository root. The complete build toolchain is documented. Dependencies are pinned. You can clone the repository and build the application yourself.

---

## 4. What Happens If 4DA Systems Disappears

This is the question that separates local-first software from everything else.

**The app keeps working.** There is no server to shut down. No authentication endpoint to go offline. No license server to stop responding (with one narrow exception; see below). The binary on your machine continues to function exactly as it did the day you installed it.

**Your data remains yours.** The SQLite database is a standard file format readable by any SQLite client. Your data is not encrypted with a key held by 4DA Systems. It is not stored in a proprietary format. It is yours, in a format the entire software industry can read.

**The source code persists.** It is publicly available. Anyone can fork it and continue development. The FSL-1.1-Apache-2.0 license automatically converts to Apache-2.0, removing even the non-compete restriction. There are no legal barriers to continuation.

**The build remains reproducible.** Pinned dependencies (`Cargo.lock`, `pnpm-lock.yaml`) ensure that the exact versions used to build a release can be restored. The build does not depend on 4DA Systems infrastructure.

**The one exception:** Signal tier license validation uses Keygen (api.keygen.sh). If Keygen becomes unreachable, the offline validation cache (7-day window) continues to authorize the application. If both Keygen and the cache expire, Signal tier features degrade to free tier. The free tier has zero external dependencies and is completely unaffected.

---

## 5. The BYOK Contract

BYOK -- Bring Your Own Key -- is not just a feature. It is a trust architecture decision.

**You provide your own API keys** for any cloud service you choose to use (Anthropic, OpenAI). If you choose not to use any cloud service, no keys are needed.

**Your keys are stored in your operating system's credential manager** -- Windows Credential Manager, macOS Keychain, or Linux Secret Service. Not in a config file. Not in our database. In the same secure storage your OS uses for its own credentials.

**Your keys are sent only to the provider you configured.** If you enter an Anthropic API key, it is sent to `api.anthropic.com` and nowhere else. The CSP enforces this at the browser level.

**4DA Systems never sees, stores, proxies, or has access to your keys.** There is no intermediary server. There is no "4DA API" that your requests pass through. The application on your machine talks directly to the provider you chose.

**If you use Ollama, zero keys are needed.** Zero external API calls are made. The application runs entirely on your hardware, using your local LLM, with your local embeddings, against your local database. The network cable could be unplugged and nothing would change.

---

## 6. Comparison: Trust Models

| Aspect | Cloud SaaS | Local + Cloud Backend | 4DA (Local-First) |
|---|---|---|---|
| Where your data lives | Their servers | Split between local and remote | Your machine only |
| Account required | Yes | Yes | No |
| Works offline | No | Partially | Yes (with Ollama) |
| Company shutdown impact | Total loss of service and data | Partial loss (cloud features die) | Zero impact (free tier) |
| Source audit possible | No | Partial (client only) | Full (client and backend are the same binary) |
| Trust required | High | Medium | Minimal |
| API key handling | They store your keys | They proxy your requests | Direct to provider, keys in OS keychain |
| Telemetry | Typically yes | Typically yes | None (local analytics only) |
| Data portability | Export tool (maybe) | Export tool (maybe) | Standard SQLite file, always accessible |

---

## 7. Remaining Trust Requirements

Honesty demands acknowledging what trust is still required, even in a local-first architecture. Pretending otherwise would undermine the credibility of everything above.

### Trust that the binary matches the source code

When you download a compiled binary, you are trusting that it was built from the published source code and not modified afterward.

**Mitigations:**
- Build from source yourself using the published repository and pinned dependencies.
- Verify the code signature (Windows EV cert, macOS Developer ID + notarization) to confirm the binary was produced by 4DA Systems and not tampered with in transit.
- Compare checksums published with each release.

### Trust in the dependency supply chain

4DA depends on hundreds of Rust crates and npm packages. A compromised dependency could theoretically introduce malicious behavior.

**Mitigations:**
- Dependencies are pinned to exact versions via `Cargo.lock` and `pnpm-lock.yaml`.
- Critical paths (encryption, key storage, database access) use well-audited Rust crates.
- Pure Rust is used for security-critical operations (no C bindings for OCR, no native Node modules for crypto).
- The Rust compiler's safety guarantees eliminate entire classes of memory-corruption supply chain attacks.

### Trust that future updates won't introduce surveillance

A trustworthy version today does not guarantee a trustworthy version tomorrow.

**Mitigations:**
- Source diffs between any two versions are publicly inspectable.
- Minisign verification means you can pin to a known-good version and refuse updates.
- The CSP is visible in the source code -- adding a new outbound endpoint would appear in the diff.
- The FSL-1.1-Apache-2.0 license ensures the source remains available for comparison.

### Trust in Keygen for license validation (Signal tier only)

Signal tier license validation contacts `api.keygen.sh`. This is the only 4DA feature that depends on a third-party service for authorization.

**Mitigations:**
- Only Signal tier is affected. The free tier makes zero external calls for license validation.
- Offline validation caches the result for 7 days, providing resilience against outages.
- If validation fails after the cache expires, the application degrades gracefully to free tier -- it does not stop working.

---

## 8. Verification Ecosystem

This document is the architectural overview. The following documents provide specific verification procedures:

| Document | Purpose |
|---|---|
| **NETWORK-TRANSPARENCY.md** | Every outbound connection the app makes, with verification steps |
| **BUILD-FROM-SOURCE.md** | Build the application from source code yourself |
| **VERIFY-DOWNLOADS.md** | Verify binary authenticity using code signatures and checksums |
| **SECURITY-AUDIT-GUIDE.md** | Audit the trust-critical code paths in the source |
| **SECURITY.md** | Report security vulnerabilities responsibly |
| **docs/legal/PRIVACY-POLICY.md** | Legal privacy commitments under AU/EU/US law |
| **LICENSE** | FSL-1.1-Apache-2.0 full terms |

Each of these documents is designed to be independently verifiable. You do not need to take our word for any claim. The tools to check are either built into your operating system or freely available.

---

## 9. Our Commitment

These are not marketing statements. They are engineering constraints that we commit to maintaining in every release:

- **We will never add telemetry, tracking, or data collection to the application.** Usage analytics remain local-only, visible to you, deletable by you.
- **We will never require a cloud account for core functionality.** The application works without an account today and will work without an account in every future version.
- **We will never gate privacy behind a paid tier.** The free tier and the Signal tier have identical privacy characteristics. Paying gives you more features, not more privacy.
- **We will publish all security audit results in full.** No redactions, no "we found and fixed some issues." The complete findings, the complete remediation.
- **We will maintain the source-available license.** The code will remain auditable. The FSL-1.1-Apache-2.0 license ensures this legally.

If we ever determine that we cannot keep one of these commitments, we will explain why publicly before making any change. We will not change the rules quietly and hope no one notices.

---

**4DA Systems Pty Ltd** (ACN 696 078 841) | Queensland, Australia
License: FSL-1.1-Apache-2.0 | Source: Available at github.com/runyourempire/4DA
