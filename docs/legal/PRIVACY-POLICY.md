# Privacy Policy

**4DA Systems Pty Ltd** | **Effective Date: 15 March 2026** | **Last Updated: 15 March 2026**

---

## The Short Version

4DA is a desktop application that runs on your computer. Your data stays on your computer. We do not collect it, we do not transmit it, we do not store it on our servers, and we do not sell it. This privacy policy exists because the law requires one, but also because we believe you deserve to know exactly how your software works.

---

## 1. Introduction and Scope

This Privacy Policy describes how **4DA Systems Pty Ltd** (ACN 696 078 841, ABN 51 696 078 841; "we", "us", "our") handles information in connection with:

- **The 4DA Desktop Application** ("the App") -- a local-first desktop application built with Tauri 2.0 (Rust backend, React frontend) that runs entirely on your device
- **The 4DA Website** (4da.ai) -- our informational website
- **The 4DA Store** (shop.4da.ai) -- our merchandise store hosted on Shopify

These three products have fundamentally different privacy characteristics, and this policy addresses each separately. The App is local-first software with no server-side data collection. The Website and Store are standard web properties with typical web privacy considerations.

**Our registered business address is in Queensland, Australia.** We comply with the Australian Privacy Act 1988 (Cth), the EU General Data Protection Regulation (GDPR), and the California Consumer Privacy Act (CCPA), along with other applicable privacy laws.

For GDPR purposes, 4DA Systems Pty Ltd is the data controller for any personal data processed through the Website and Store. For the App, you are the sole controller of your own data -- we never have access to it.

---

## 2. The 4DA Desktop Application

### 2.1 Privacy by Architecture

4DA is designed so that privacy is not a policy choice but a technical fact. The App has no server-side component that collects user data. There is no 4DA cloud service, no user database on our end, and no mechanism for us to receive your data even if we wanted to. Your data lives in a local SQLite database on your machine and nowhere else.

No cloud account is required. No sign-up. No registration. You download the App and use it.

### 2.2 What Data the App Processes Locally

The App processes the following data **entirely on your local machine**:

- **Content from public APIs:** The App fetches publicly available content from sources including Hacker News, GitHub, arXiv, Reddit, and RSS feeds. These requests go directly from your machine to the public APIs. No personal data is included in these requests beyond your device's IP address, which is inherent to any network request.
- **Your project files:** The Autonomous Context Engine (ACE) can scan local project directories you configure to understand your development context and improve content relevance. This data is processed locally and never leaves your machine.
- **Your preferences and settings:** Configuration data stored in a local JSON file on your device. This file contains non-sensitive preferences such as theme, source configuration, and display options. Sensitive credentials are stored separately in your operating system's keychain (see Section 2.4).
- **Local database:** A SQLite database (with sqlite-vec for vector search) stored on your device containing aggregated content, embeddings, and relevance scores. This database is never synced to, uploaded to, or transmitted to any external service.
- **Local telemetry:** Usage analytics are stored locally for your own reference. This telemetry data is never sent to 4DA Systems or any third party.
- **Content scoring and analysis:** The PASIFA scoring algorithm runs entirely on your machine. When configured, local embeddings are generated via Ollama (also on your machine). When you opt to use a cloud LLM provider, only the specific content being analysed is sent (see Section 2.4).

### 2.3 What Data We Collect from the App

**None.**

- No telemetry is sent externally
- No usage analytics leave your device
- No crash reports are transmitted
- No error logs are sent to external services
- No tracking pixels, fingerprinting, or behavioural profiling
- No user accounts, logins, or registrations
- No cookies (the App is a desktop application, not a website)
- No third-party SDKs that phone home
- No device identifiers sent anywhere

We have made a deliberate architectural decision to build **zero data collection infrastructure** for the App. We cannot collect your data because we have built no mechanism to do so.

### 2.4 API Key Storage and the BYOK Model

4DA operates on a **Bring Your Own Key (BYOK)** model. You provide your own API keys for any external services you choose to use. We do not supply, manage, or have access to your API keys.

**How your API keys are stored:**

Your API keys are stored in your **operating system's native credential manager** -- the most secure local storage available on your platform:

- **Windows:** Windows Credential Manager
- **macOS:** macOS Keychain
- **Linux:** Secret Service API (GNOME Keyring or KWallet)

API keys are never stored in plaintext configuration files. If you previously used a version of 4DA that stored keys in the settings file, the App automatically migrates them to the OS keychain and removes them from the plaintext file.

If the OS keychain is unavailable (e.g., headless Linux environments or CI systems), the App falls back to in-memory storage for the duration of the session, and keys are not persisted to disk.

**Your API keys are never transmitted to 4DA Systems or any party other than the specific provider you configured.**

### 2.5 LLM API Calls

The App supports integration with large language model (LLM) providers. Here is exactly how this works:

- **Supported providers:** Anthropic (Claude), OpenAI, OpenAI-compatible endpoints (including providers such as Groq, DeepSeek, and others you configure), and Ollama (local).
- **API calls go directly from your machine to the provider.** The App makes HTTPS requests directly to the provider's API endpoints (e.g., api.anthropic.com, api.openai.com, or any custom base URL you configure for OpenAI-compatible providers). 4DA Systems does not proxy, intercept, log, or route these calls through any 4DA infrastructure. We never see the content of these requests or responses.
- **What is sent:** The content being analysed (e.g., article text, search queries) and a system prompt. Your API key is included in the request headers for authentication with the provider.
- **Ollama runs entirely locally.** If you use Ollama as your LLM provider (the default configuration), all AI processing happens on your machine at `localhost:11434` with zero external API calls. This is a fully offline option. If you point Ollama to a remote instance, data flows to that remote address -- this is your configuration choice.

When you use a third-party LLM provider, you are subject to that provider's privacy policy. We encourage you to review:

- [Anthropic Privacy Policy](https://www.anthropic.com/privacy)
- [OpenAI Privacy Policy](https://openai.com/policies/privacy-policy)
- The privacy policy of any OpenAI-compatible provider you choose to configure

### 2.6 Content Source Fetching

The App fetches publicly available content from sources you configure:

- **Hacker News:** Public API requests to `hacker-news.firebaseio.com`. No authentication required.
- **Reddit:** Public API requests to `www.reddit.com`. No authentication tokens sent.
- **RSS/Atom feeds:** Standard HTTP GET requests to the feed URLs you configure.
- **GitHub:** Public API requests to `api.github.com`. If you configure a GitHub Personal Access Token (PAT) for higher rate limits or private repository access, your PAT is sent to GitHub's API in the request headers. Your PAT is stored in the OS keychain (see Section 2.4) and is never sent to 4DA Systems.
- **arXiv:** Public API requests to `export.arxiv.org`.

All of these requests go directly from your machine to the respective service. Your device's IP address is visible to each service as part of the standard HTTPS connection.

### 2.7 Digest Email (Optional)

The App can optionally send you a periodic email digest of relevant content. This feature is **off by default** and requires you to configure your own SMTP server credentials. When enabled:

- Emails are sent **directly from your machine** to your SMTP provider. 4DA Systems never sees your email address, SMTP credentials, or digest contents.
- Your SMTP credentials are stored locally on your device.
- This feature is entirely opt-in and can be disabled at any time.

### 2.8 License Validation (Signal Tier)

If you purchase a Signal subscription, the App validates your license key using the [Keygen](https://keygen.sh) license management service. This is the **only external call that 4DA Systems controls**. This validation:

- Sends your **license key** to Keygen's validation API
- Does **not** send your name, email address, device identifiers, or any other personal data
- Occurs at activation and periodically thereafter (cached for 24 hours to minimise network calls)
- Results are cached locally to allow offline usage between validations

The App also supports **offline license verification** using Ed25519 digital signatures embedded in the license key itself. This verification requires no network access whatsoever.

Keygen may log the IP address of the request as part of standard server operations. See [Keygen's Privacy Policy](https://keygen.sh/privacy/) for details.

The free tier of 4DA does not perform any license validation and makes no calls to Keygen.

### 2.9 Auto-Updates

The App checks for updates using the standard Tauri updater, which queries GitHub Releases. This check:

- Sends a standard HTTPS request to GitHub's API to check for new versions
- Does **not** include any personal data, device identifiers, or telemetry
- Your IP address is visible to GitHub as part of the standard HTTPS connection

See [GitHub's Privacy Statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement) for how GitHub handles server logs.

### 2.10 Team Relay (Optional, Enterprise Feature)

4DA offers a Team Relay feature for teams to sync relevant content metadata across devices. This feature is designed with the same privacy-first principles as the rest of the App:

- **End-to-end encryption:** All data synced through the relay is encrypted on the client using XChaCha20Poly1305 with X25519 key exchange and HKDF key derivation. Encryption and decryption happen exclusively on your device.
- **Zero-knowledge relay:** The relay server handles only encrypted blobs. It cannot read, decrypt, or access the plaintext content of any synced data. It is architecturally a "dumb pipe" -- it routes encrypted payloads without any ability to inspect them.
- **Key management is local:** Each team member's X25519 private key is generated on their device and never leaves it. Team-wide symmetric keys are distributed encrypted per-member. Private keys are zeroized from memory when no longer needed.
- **No content visibility:** 4DA Systems operates the relay infrastructure but has no ability to view the content being synced between team members.

### 2.11 Source Code Transparency

The 4DA source code is publicly available under the FSL-1.1-Apache-2.0 license. You can audit exactly what the App does, what network requests it makes, and verify that it behaves as described in this policy. We believe source-available code is the strongest privacy guarantee we can offer.

---

## 3. The 4DA Website (4da.ai)

Unlike the App, our website is a standard web property. When you visit 4da.ai:

### 3.1 Data We May Collect

- **Server logs:** Our hosting provider (Vercel) may collect standard server log data including IP addresses, browser type, referring pages, and pages visited. See [Vercel's Privacy Policy](https://vercel.com/legal/privacy-policy).
- **Analytics:** We may use privacy-respecting analytics to understand aggregate traffic patterns. If used, this data is anonymised and does not track individual users across sessions.
- **Cookies:** The website may use essential cookies for basic functionality. We do not use third-party advertising cookies or cross-site tracking cookies.

### 3.2 Data We Do Not Collect via the Website

- We do not require account creation on the website
- We do not collect personal information through the website unless you voluntarily provide it (e.g., by emailing us)

---

## 4. The 4DA Store (shop.4da.ai)

Our merchandise store is hosted on Shopify. When you make a purchase:

### 4.1 Data Shopify Collects

Shopify processes your purchase information including name, email, shipping address, and payment details. This data is handled by Shopify in accordance with their privacy practices. See [Shopify's Privacy Policy](https://www.shopify.com/legal/privacy).

### 4.2 Our Use of Store Data

We use information from store purchases solely to:

- Fulfil and ship your order
- Respond to customer service inquiries
- Comply with legal obligations (e.g., tax reporting)

We do not use purchase data for marketing, profiling, or any purpose unrelated to fulfilling your order unless you explicitly opt in to communications.

---

## 5. What We Do Not Do

To be unambiguous:

- **We do not sell your data.** Not now, not ever, not to anyone.
- **We do not share your data** with third parties for their marketing or advertising purposes.
- **We do not build profiles** about you based on your use of the App.
- **We do not serve advertisements** in the App, on the Website, or through the Store.
- **We do not engage in cross-service tracking** between the App, Website, and Store.
- **We do not use third-party SDKs** that collect data in the App.
- **We do not collect device fingerprints** or hardware identifiers.

---

## 6. Third-Party Services Summary

| Service | Used By | Data Shared | Purpose |
|---------|---------|-------------|---------|
| LLM Providers (Anthropic, OpenAI, OpenAI-compatible) | App (BYOK, user-initiated) | Content being analysed + your API key (direct from your machine to provider) | AI-powered content analysis |
| Ollama | App (default) | Nothing (fully local at localhost:11434) | Local AI processing and embeddings |
| Hacker News API | App | Standard HTTP requests (your IP visible) | Content aggregation |
| GitHub API | App | Standard HTTP requests + optional PAT (your IP visible) | Content aggregation, update checks |
| Reddit | App | Standard HTTP requests (your IP visible) | Content aggregation |
| arXiv | App | Standard HTTP requests (your IP visible) | Content aggregation |
| RSS/Atom feeds | App | Standard HTTP requests (your IP visible) | Content aggregation |
| Keygen | App (Signal tier only) | License key | License validation |
| User's SMTP provider | App (optional, user-configured) | Digest email contents | Email delivery |
| Vercel | Website | Server logs, analytics | Website hosting |
| Shopify | Store | Purchase and shipping information | Merchandise sales |

---

## 7. Data Retention and Deletion

### 7.1 App Data

All App data resides on your device. You have complete control:

- **Delete the database:** Remove the local SQLite database file to erase all aggregated content, embeddings, and relevance scores
- **Delete settings:** Remove the settings file to erase all non-sensitive configuration
- **Delete keychain entries:** Remove the "com.4da.app" entries from your OS credential manager (Windows Credential Manager, macOS Keychain, or Linux Secret Service) to erase stored API keys and license keys
- **Uninstall the App:** Removes the application binary. Deleting the data directory and keychain entries removes all associated data
- **We cannot delete your App data** because we never had access to it

No action from us is required for you to fully erase all traces of the App from your system.

### 7.2 Website Data

Server logs retained by Vercel are subject to Vercel's data retention policies. We do not independently retain website visitor data.

### 7.3 Store Data

Purchase records are retained as required by Australian tax law (generally 5 years). You may request deletion of non-legally-required data by contacting privacy@4da.ai.

---

## 8. Children's Privacy

The 4DA App, Website, and Store are not designed for, marketed to, or intended for use by children under the age of 13 (or under 16 in jurisdictions where GDPR applies). We do not knowingly collect personal information from children.

If you believe a child has provided us with personal information (e.g., via email or the Store), please contact us at privacy@4da.ai and we will promptly delete it.

---

## 9. Your Rights

Because the App collects no personal data, most privacy rights are automatically satisfied for App usage. For data held in connection with the Website or Store, the following rights apply:

### 9.1 Australian Privacy Act

Under the Australian Privacy Principles (APPs), you have the right to:

- Access personal information we hold about you
- Request correction of inaccurate information
- Lodge a complaint with us or the Office of the Australian Information Commissioner (OAIC) if you believe your privacy has been breached

**Small business exemption:** Under the Australian Privacy Act 1988, businesses with annual turnover under AUD 3 million are generally exempt from the APPs. While this exemption may currently apply to 4DA Systems, we voluntarily comply with the APPs because privacy is foundational to our product. We commit to maintaining this standard regardless of whether the Act legally requires it.

To make a request, contact privacy@4da.ai. We will respond within 30 days.

### 9.2 GDPR (European Economic Area, United Kingdom, Switzerland)

If you are in the EEA, UK, or Switzerland, you have the right to:

- **Access** your personal data
- **Rectification** of inaccurate data
- **Erasure** ("right to be forgotten")
- **Restriction** of processing
- **Data portability** -- receive your data in a structured, machine-readable format
- **Object** to processing based on legitimate interests
- **Withdraw consent** at any time where processing is based on consent
- **Lodge a complaint** with your local data protection supervisory authority

**Legal basis for processing:** Where we process personal data (Website analytics, Store transactions), our legal bases are:

- **Contractual necessity** -- to fulfil merchandise orders
- **Legitimate interests** -- to operate and improve the Website, provided these interests are not overridden by your rights
- **Legal obligation** -- to comply with tax and business record-keeping laws

**Note on the App:** For the App, GDPR obligations are inherently satisfied because we process no personal data. All data processing occurs locally on your device under your sole control.

To exercise your rights, contact privacy@4da.ai. We will respond within 30 days (extendable by 60 days for complex requests, with prior notice).

### 9.3 CCPA (California)

If you are a California resident, you have the right to:

- **Know** what personal information we collect, use, and disclose
- **Delete** personal information we hold about you
- **Opt out of sale** of personal information -- **we do not sell personal information**
- **Non-discrimination** -- we will not treat you differently for exercising your rights

**Categories of personal information collected in the past 12 months:**

| Category | Collected? | Source | Purpose |
|----------|-----------|--------|---------|
| Identifiers (name, email, address) | Only via Store purchases | You (directly) | Order fulfilment |
| Internet activity (browsing, search) | Aggregate analytics only (Website) | Automatic | Website improvement |
| Commercial information (purchase history) | Only via Store | You (directly) | Order fulfilment, legal compliance |

We do not collect biometric data, geolocation data, professional or employment information, education information, or inferences drawn about you from any of the above.

To make a CCPA request, contact privacy@4da.ai or write to us at the address in Section 14. We will verify your identity and respond within 45 days.

---

## 10. International Data Transfers

### 10.1 The App

The App does not transfer your data internationally -- or at all -- because your data stays on your device. When you choose to use LLM provider APIs, data flows directly from your device to the provider's servers. The location of those servers depends on the provider you choose:

- **Anthropic:** Servers primarily in the United States
- **OpenAI:** Servers primarily in the United States
- **OpenAI-compatible providers:** Server locations depend on the specific provider you configure
- **Ollama:** Your own device (no transfer)

These transfers are initiated by you, using your own API keys, and are governed by the respective provider's privacy policy. 4DA Systems has no role in these transfers.

### 10.2 Website and Store

Our website is hosted on Vercel, which operates globally. Shopify also operates globally. Data processed by these services may be transferred to and stored in countries outside your jurisdiction, including the United States. These providers maintain appropriate safeguards for international transfers, including Standard Contractual Clauses where required by GDPR.

---

## 11. Security

### 11.1 The App

The App's security model is straightforward: your data is on your device, protected by your operating system's security controls.

**API key protection:** API keys are stored in your operating system's native credential manager (Windows Credential Manager, macOS Keychain, or Linux Secret Service), which provides hardware-backed or OS-level encryption depending on your platform. Keys are never written to plaintext files.

**Team Relay encryption:** The Team Relay feature uses XChaCha20Poly1305 authenticated encryption with X25519 key exchange and HKDF key derivation (SHA-256). Private keys are zeroized from memory when no longer needed. This ensures that data in transit and at rest on the relay server is cryptographically protected against unauthorised access, including by 4DA Systems.

**Recommendations:**

- Keep your operating system and the App updated
- Use full-disk encryption on your device
- Protect your API keys as you would any credential
- Use a strong OS login password (this protects your keychain entries)

### 11.2 Website and Store

We rely on industry-standard security measures provided by Vercel and Shopify, including TLS encryption for all data in transit and secure payment processing (Shopify is PCI-DSS compliant).

---

## 12. Changes to This Policy

We may update this Privacy Policy from time to time. When we make changes:

- We will update the "Last Updated" date at the top of this policy
- For material changes, we will provide notice through the App's release notes or on our website
- The current version of this policy is always available in the App's source repository and at [4da.ai/privacy](https://4da.ai/privacy)

We encourage you to review this policy periodically. Your continued use of the App, Website, or Store after changes are posted constitutes acceptance of the updated policy.

---

## 13. Governing Law

This Privacy Policy is governed by and construed in accordance with the laws of Queensland, Australia, without regard to conflict of law principles. Any disputes arising under this policy shall be subject to the exclusive jurisdiction of the courts of Queensland, Australia.

---

## 14. Contact Us

If you have questions about this Privacy Policy, want to exercise your privacy rights, or have a concern:

**Email (preferred):**
- General inquiries: [support@4da.ai](mailto:support@4da.ai)
- Privacy-specific requests: [privacy@4da.ai](mailto:privacy@4da.ai)

**Entity:**
4DA Systems Pty Ltd
ACN 696 078 841 | ABN 51 696 078 841
Queensland, Australia

**Response times:**
- General inquiries: within 5 business days
- Privacy rights requests: within 30 days (as required by applicable law)

**Complaints:**
If you are unsatisfied with our response, you may lodge a complaint with:
- **Australia:** Office of the Australian Information Commissioner (OAIC) -- [oaic.gov.au](https://www.oaic.gov.au)
- **EU/EEA:** Your local data protection supervisory authority
- **UK:** Information Commissioner's Office (ICO) -- [ico.org.uk](https://ico.org.uk)

---

## 15. Summary Table

| Question | App | Website | Store |
|----------|-----|---------|-------|
| Do we collect personal data? | No | Minimal (server logs) | Yes (for orders) |
| Do we store data on our servers? | No | Vercel handles logs | Shopify handles data |
| Do we sell data? | No | No | No |
| Do we use cookies? | N/A (desktop app) | Essential only | Shopify standard |
| Do we track you? | No | No | No |
| Can you delete your data? | Yes (it is on your device) | Contact us | Contact us |
| Do we require an account? | No | No | For purchases (via Shopify) |
| Do we use advertising? | No | No | No |
| Where are API keys stored? | OS keychain (your device) | N/A | N/A |
| Can we read your content? | No | N/A | N/A |

---

*This Privacy Policy is written in plain language because we believe privacy policies should be understood, not endured. If anything is unclear, contact us -- we are happy to explain.*
