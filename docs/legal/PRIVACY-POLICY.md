# Privacy Policy

**4DA Systems** | **Effective Date: 1 March 2026** | **Last Updated: March 2026**

---

## The Short Version

4DA is a desktop application that runs on your computer. Your data stays on your computer. We don't collect it, we don't transmit it, we don't store it on our servers, and we don't sell it. This privacy policy exists because the law requires one, but also because we believe you deserve to know exactly how your software works.

---

## 1. Introduction & Scope

This Privacy Policy describes how **4DA Systems** (ABN pending; "we", "us", "our") handles information in connection with:

- **The 4DA Desktop Application** ("the App") -- a Tauri 2.0 desktop application that runs locally on your device
- **The 4DA Website** (4da.ai) -- our informational website
- **The 4DA Store** -- our merchandise store hosted on Shopify

These three products have fundamentally different privacy characteristics, and this policy addresses each separately. The App is local-first software with no server-side component. The Website and Store are standard web properties with typical web privacy considerations.

**Our registered business address is in Queensland, Australia.** We comply with the Australian Privacy Act 1988 (Cth), the EU General Data Protection Regulation (GDPR), and the California Consumer Privacy Act (CCPA), along with other applicable privacy laws.

For GDPR purposes, 4DA Systems is the data controller for any personal data processed through the Website and Store. For the App, you are the sole controller of your own data -- we never have access to it.

---

## 2. The 4DA Desktop Application

### 2.1 What Data the App Processes

The App processes the following data **entirely on your local machine**:

- **Content from public APIs:** The App fetches publicly available content from sources like Hacker News, GitHub, arXiv, Reddit, and RSS feeds to aggregate developer-relevant information. No personal data is included in these requests beyond your device's IP address (which is inherent to any network request).
- **Your project files:** The Autonomous Context Engine (ACE) scans local project directories you choose to share with the App to understand your development context. This data never leaves your machine.
- **Your preferences and settings:** Configuration data stored in a local `settings.json` file on your device.
- **Local database:** A SQLite database stored on your device containing aggregated content, embeddings, and relevance scores. This database is never synced, uploaded, or transmitted.

### 2.2 What Data We Collect from the App

**None.**

- No telemetry
- No usage analytics
- No crash reporting
- No error logging to external services
- No tracking pixels, fingerprinting, or behavioural profiling
- No user accounts, logins, or registrations

We have made a deliberate architectural decision to have **zero server-side infrastructure** for the App. There is no 4DA server that the App communicates with. We cannot collect your data because we have built no mechanism to do so.

### 2.3 BYOK (Bring Your Own Key) -- LLM API Calls

The App supports integration with large language model (LLM) providers including Anthropic, OpenAI, and Ollama. When you configure an LLM provider:

- **You provide your own API key.** We do not supply, manage, or have access to your API keys.
- **API calls go directly from your machine to the provider.** The App makes HTTPS requests directly to the provider's API endpoints (e.g., api.anthropic.com, api.openai.com). 4DA does not proxy, intercept, log, or route these calls through any 4DA infrastructure.
- **Your API key is stored locally** in your settings file on your device. It is never transmitted to us.
- **Ollama runs entirely locally.** If you use Ollama as your LLM provider, all processing happens on your machine with no network calls whatsoever.

You are subject to the privacy policies of any LLM provider you choose to use. We encourage you to review:
- [Anthropic Privacy Policy](https://www.anthropic.com/privacy)
- [OpenAI Privacy Policy](https://openai.com/policies/privacy-policy)

### 2.4 License Validation (Pro Tier)

If you purchase a Pro license, the App validates your license key against the [Keygen.sh](https://keygen.sh) API. This validation:

- Sends **only your license key** to Keygen.sh
- Does **not** send your name, email, device fingerprint, or any other personal data
- Occurs only when the App checks license status

Keygen.sh may log the IP address of the request as part of standard server operations. See [Keygen's Privacy Policy](https://keygen.sh/privacy/) for details.

### 2.5 Auto-Updates

The App checks for updates against GitHub Releases. This check:

- Sends a standard HTTPS request to GitHub's API
- Does **not** include any personal data or device identifiers
- Your IP address is visible to GitHub as part of the standard HTTPS connection

See [GitHub's Privacy Statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement) for how GitHub handles server logs.

### 2.6 Open Source Transparency

The 4DA source code is publicly available under the FSL-1.1-Apache-2.0 license. You can audit exactly what the App does, what network requests it makes, and verify that it behaves as described in this policy. We believe this level of transparency is the strongest privacy guarantee we can offer.

---

## 3. The 4DA Website (4da.ai)

Unlike the App, our website is a standard web property. When you visit 4da.ai:

### 3.1 Data We May Collect

- **Server logs:** Our hosting provider (Vercel) may collect standard server log data including IP addresses, browser type, referring pages, and pages visited. See [Vercel's Privacy Policy](https://vercel.com/legal/privacy-policy).
- **Analytics:** We may use Vercel Analytics or similar privacy-respecting analytics to understand aggregate traffic patterns. If used, this data is anonymised and does not track individual users across sessions.
- **Cookies:** The website may use essential cookies for functionality. We do not use third-party advertising cookies or cross-site tracking cookies.

### 3.2 Data We Do Not Collect via the Website

- We do not require account creation on the website
- We do not collect personal information through the website unless you voluntarily provide it (e.g., by emailing us)

---

## 4. The 4DA Store (Shopify)

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

## 5. Third-Party Services Summary

| Service | Used By | Data Shared | Purpose |
|---------|---------|-------------|---------|
| LLM Providers (Anthropic, OpenAI) | App | Your prompts + API key (direct from your machine) | AI-powered features |
| Ollama | App | Nothing (fully local) | Local AI processing |
| Hacker News, GitHub, Reddit, arXiv, RSS | App | Standard HTTP requests (your IP) | Content aggregation |
| Keygen.sh | App (Pro) | License key only | License validation |
| GitHub Releases | App | Standard HTTP request (your IP) | Update checks |
| Vercel | Website | Server logs, analytics | Website hosting |
| Shopify | Store | Purchase/shipping info | Merchandise sales |

We do not share, sell, rent, or trade any personal information with third parties for their marketing purposes.

---

## 6. Children's Privacy

The 4DA App, Website, and Store are not designed for, marketed to, or intended for use by children under the age of 13 (or under 16 in jurisdictions where GDPR applies). We do not knowingly collect personal information from children.

If you believe a child has provided us with personal information (e.g., via email or the Store), please contact us at privacy@4da.ai and we will promptly delete it.

---

## 7. Data Retention & Deletion

### 7.1 App Data

All App data resides on your device. You have complete control:

- **Delete the database:** Remove the local SQLite database file to erase all aggregated content and scores
- **Delete settings:** Remove the settings file to erase all configuration, including any stored API keys
- **Uninstall the App:** Removes the application and its local data
- **We cannot delete your App data** because we never had it

### 7.2 Website Data

Server logs retained by Vercel are subject to Vercel's data retention policies. We do not independently retain website visitor data.

### 7.3 Store Data

Purchase records are retained as required by Australian tax law (generally 5 years). You may request deletion of non-legally-required data by contacting privacy@4da.ai.

---

## 8. Your Rights

Depending on where you live, you have specific rights regarding your personal data. Because the App collects no personal data, most of these rights are automatically satisfied. For data held in connection with the Website or Store:

### 8.1 Australian Privacy Act

Under the Australian Privacy Principles (APPs), you have the right to:

- Access personal information we hold about you
- Request correction of inaccurate information
- Lodge a complaint with us or the Office of the Australian Information Commissioner (OAIC) if you believe your privacy has been breached

To make a request, contact privacy@4da.ai. We will respond within 30 days.

### 8.2 GDPR (European Economic Area, UK, Switzerland)

If you are in the EEA, UK, or Switzerland, you have the right to:

- **Access** your personal data
- **Rectification** of inaccurate data
- **Erasure** ("right to be forgotten")
- **Restriction** of processing
- **Data portability** -- receive your data in a structured, machine-readable format
- **Object** to processing based on legitimate interests
- **Withdraw consent** at any time (where processing is based on consent)
- **Lodge a complaint** with your local supervisory authority

**Legal basis for processing:** Where we process personal data (Website analytics, Store transactions), our legal bases are:
- **Contractual necessity** -- to fulfil merchandise orders
- **Legitimate interests** -- to operate and improve the Website, provided these interests are not overridden by your rights
- **Legal obligation** -- to comply with tax and business record-keeping laws

To exercise your rights, contact privacy@4da.ai. We will respond within 30 days (extendable by 60 days for complex requests, with notice).

### 8.3 CCPA (California)

If you are a California resident, you have the right to:

- **Know** what personal information we collect, use, and disclose
- **Delete** personal information we hold about you
- **Opt out of sale** of personal information -- **we do not sell personal information**
- **Non-discrimination** -- we will not treat you differently for exercising your rights

**Categories of personal information collected in the past 12 months:**

| Category | Collected? | Source | Purpose |
|----------|-----------|--------|---------|
| Identifiers (name, email, address) | Only via Store purchases | You | Order fulfilment |
| Internet activity (browsing, search) | Aggregate analytics only (Website) | Automatic | Website improvement |
| Commercial information (purchase history) | Only via Store | You | Order fulfilment, legal compliance |

We do not collect biometric data, geolocation data, professional/employment information, education information, or inferences about you.

To make a CCPA request, contact privacy@4da.ai or write to us at the address in Section 11. We will verify your identity and respond within 45 days.

---

## 9. International Data Transfers

### 9.1 The App

The App does not transfer your data internationally -- or at all -- because your data stays on your device. When you use LLM provider APIs, the data flows directly from your device to the provider's servers. The location of those servers depends on the provider you choose:

- **Anthropic:** Servers primarily in the United States
- **OpenAI:** Servers primarily in the United States
- **Ollama:** Your own device (no transfer)

These transfers are initiated by you, using your own API keys, and are governed by the respective provider's privacy policy.

### 9.2 Website & Store

Our website is hosted on Vercel, which operates globally. Shopify also operates globally. Data processed by these services may be transferred to and stored in countries outside your jurisdiction, including the United States. These providers maintain appropriate safeguards for international transfers, including Standard Contractual Clauses where required by GDPR.

---

## 10. Security

### 10.1 The App

The App's security model is straightforward: your data is on your device, protected by your operating system's security. We recommend:

- Keeping your operating system and the App updated
- Storing API keys securely (the App stores them in a local configuration file)
- Using full-disk encryption on your device

### 10.2 Website & Store

We rely on industry-standard security measures provided by Vercel and Shopify, including TLS encryption for all data in transit and secure payment processing (Shopify is PCI-DSS compliant).

---

## 11. Changes to This Policy

We may update this Privacy Policy from time to time. When we make material changes:

- We will update the "Last Updated" date at the top of this policy
- For significant changes, we will provide notice through the App's release notes or on our website

We encourage you to review this policy periodically. Your continued use of the App, Website, or Store after changes are posted constitutes acceptance of the updated policy.

---

## 12. Contact Us

If you have questions about this Privacy Policy, want to exercise your rights, or have a privacy concern:

**Email (preferred):**
- General inquiries: [support@4da.ai](mailto:support@4da.ai)
- Privacy-specific requests: [privacy@4da.ai](mailto:privacy@4da.ai)

**Entity:**
4DA Systems
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

## 13. Summary Table

| Question | App | Website | Store |
|----------|-----|---------|-------|
| Do we collect personal data? | No | Minimal (analytics) | Yes (for orders) |
| Do we store data on our servers? | No | Vercel logs | Shopify |
| Do we sell data? | No | No | No |
| Do we use cookies? | N/A (desktop app) | Essential only | Shopify standard |
| Do we track you? | No | No | No |
| Can you delete your data? | Yes (it's on your device) | Contact us | Contact us |
| Do we require an account? | No | No | For purchases only (via Shopify) |

---

*This Privacy Policy is written in plain language because we believe privacy policies should be understood, not endured. If anything is unclear, please reach out -- we are happy to explain.*
