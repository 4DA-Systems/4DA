# 4DA Network Manifest

**This document lists every outbound network connection 4DA makes — nothing else leaves your
machine.** Run Wireshark while 4DA is running and every host you see is accounted for below. Your
raw project files, source code, git history, file contents, and personal information **never leave
your machine**. The single flow that transmits anything *derived* from your local content is the
optional dependency-vulnerability sync, which sends only **package names** (not code) to OSV — it is
disclosed explicitly in the "Conditional / opt-in" section and can be turned off.

4DA is local-first and privacy-first. There is **no telemetry, no analytics, and no crash
reporting** (see "Never" at the bottom). The only 4DA-operated endpoint the app ever contacts is a
user-initiated license-recovery lookup at `4da.ai`. Everything else is either a public third-party
content API, a cloud service you explicitly configured (BYOK LLM / translation), or a one-time
setup download.

---

## HTTP client identity

All shared outbound requests use a pooled `reqwest` client
(`src-tauri/src/http_client.rs`, and `src-tauri/src/sources/mod.rs::SHARED_CLIENT`) with:

- **User-Agent:** `Mozilla/5.0 (compatible; desktop-app)` — the default for source fetching,
  connectivity checks, license validation, and article scraping.
- **Exceptions (purpose-built User-Agents):**
  - crates.io adapter → `4DA-Developer-OS/1.0 (https://4da.ai)` (`sources/crates_io.rs`)
  - HuggingFace / PapersWithCode / GitHub Advisory adapters → `4DA-Developer-OS/1.0`
    (`sources/huggingface.rs`, `sources/papers_with_code.rs`, `sources/cve.rs`)
  - OSV dependency sync → `4DA/1.0 (local-osv-mirror)` (`osv/sync.rs`)
  - Team relay (only if you join/create a team) → `4DA-TeamSync/1.0` (`http_client.rs`)
- No cookies, no login, no tracking headers are sent on any request.

---

## 1. Always-on

These run automatically while the app is open. Every source can be individually enabled/disabled in
**Settings → Sources**.

### 1a. Connectivity pre-check (before every fetch cycle)

Before fetching sources, 4DA races a `HEAD` request against three targets and uses whichever
responds first (`src-tauri/src/source_fetching/fetcher.rs`):

| Host | Request |
|---|---|
| `1.1.1.1` (Cloudflare) | `HEAD https://1.1.1.1/cdn-cgi/trace` |
| `dns.google` (Google DNS) | `HEAD https://dns.google/resolve?name=example.com` |
| `httpbin.org` | `HEAD https://httpbin.org/get` |

- **Trigger:** once per analysis cycle, before source fetching begins.
- **Data sent:** `HEAD` only — no body, no user data.
- **If offline:** 4DA falls back to cached content and keeps working.
- **Disable:** disabling all sources stops fetch cycles, which stops this check.

### 1b. Content source fetches

All retrieve **public** developer content. Trigger cadence is the default fetch interval per source;
"deep fetch" is a larger one-time pull on first run.

| Source | Host | Endpoint(s) | Trigger | Auth / data sent |
|---|---|---|---|---|
| Hacker News | `hacker-news.firebaseio.com` | `/v0/topstories.json`, `/v0/{new,best,ask,show}stories.json` (deep), `/v0/item/{id}.json` | auto | None |
| HN fallback | `hn.algolia.com`, `hnrss.org` | `hn.algolia.com/api/v1/search?tags=front_page`, `hnrss.org/frontpage` | only if HN primary fails | None |
| Reddit | `www.reddit.com` | `/r/{subreddit}/hot.json?limit={n}` | auto | None (public JSON) |
| Reddit fallback | `www.reddit.com` | `/r/programming/.rss` | only if Reddit primary fails | None |
| arXiv | `export.arxiv.org` | `/api/query?search_query={categories}&...` | auto | None |
| arXiv fallback | `arxiv.org` | `/rss/cs.AI` | only if arXiv primary fails | None |
| GitHub | `api.github.com` | `/search/repositories?q={query}&sort=stars`, `/repos/{owner}/{repo}/readme` | auto | Unauthenticated. Query holds language names + a date filter — no user data |
| GitHub fallback | `github.com` | `/trending?since=daily` | only if GitHub primary fails | None |
| RSS feeds | 12 default hosts (below) | `GET {feed_url}` | auto | None |
| YouTube | `www.youtube.com` | `/feeds/videos.xml?channel_id={id}` (public Atom) | auto | None |
| Dev.to | `dev.to` | `/api/articles?per_page=30&top=7` (+ tag pulls on deep fetch) | auto | None |
| Lobsters | `lobste.rs` | `/hottest.json`, `/newest.json` (deep) | auto | None |
| Product Hunt | `www.producthunt.com` | `/feed` | auto | None |
| Bluesky | `public.api.bsky.app` | `/xrpc/app.bsky.feed.getFeed?feed=...whats-hot` | auto | None (public "What's Hot", no auth) |
| crates.io | `crates.io` | `/api/v1/...` | auto | None (UA `4DA-Developer-OS/1.0`) |
| npm | `registry.npmjs.org` | `/{package}` | auto | None |
| PyPI | `pypi.org` | `/pypi/{package}/json` | auto | None |
| HuggingFace | `huggingface.co` | `/api/models` | auto | None |
| PapersWithCode | `huggingface.co` | `/api/daily_papers` (PwC API now redirects here) | auto | None |
| Stack Overflow | `api.stackexchange.com` | `/2.3/questions?...&site=stackoverflow&tagged={tag}` | auto | None |
| Go modules | `index.golang.org` | `/index?limit={n}` | auto | None |

**RSS default hosts** (`sources/rss.rs`, user-customizable): `feeds.arstechnica.com`,
`www.theverge.com`, `techcrunch.com`, `blog.rust-lang.org`, `engineering.fb.com`, `medium.com`,
`github.blog`, `blog.cloudflare.com`, `martinfowler.com`, `simonwillison.net`, `jvns.ca`,
`danluu.com`.

- **Disable any source:** Settings → Sources. A disabled source makes **zero** network calls.

### 1c. Article scraping

After fetching items, 4DA scrapes the linked article URL to extract **text only** (no images or
media) for HN, Reddit link posts, Lobsters, and RSS items.

- **Hosts:** any domain linked by the sources above.
- **Data sent:** a plain `GET` with the default User-Agent. No cookies, no login.
- **Rate limit:** ~100 ms between requests; 2–10 s per-article timeout.

### 1d. Model-pricing refresh

On startup, 4DA refreshes its cost/context-window table for known LLM models
(`src-tauri/src/model_registry.rs::refresh_registry`, called from `app_setup.rs`):

- **Host:** `raw.githubusercontent.com`
- **Endpoint:** `GET .../BerriAI/litellm/main/model_prices_and_context_window.json`
- **Trigger:** once at startup, fire-and-forget, **at most once per 24 h** (cached otherwise).
- **Data sent:** plain `GET`, no user data, no parameters. Falls back to bundled/cached pricing on
  failure.
- **Disable:** offline operation skips it silently; the bundled table is used.

---

## 2. Conditional / opt-in

None of these run unless you take an action (configure a key, join a team, or have local
dependencies discovered).

### 2a. BYOK cloud LLM (relevance judging)

Only contacted when you configure an LLM API key. Default is **local** (Ollama / in-process
embeddings) — with no key configured, zero LLM network calls leave the machine.
(`src-tauri/src/llm.rs`, `src-tauri/src/llm_judge.rs`)

| Provider | Host | Endpoint |
|---|---|---|
| Anthropic | `api.anthropic.com` | `POST /v1/messages` |
| OpenAI | `api.openai.com` | `POST /v1/chat/completions`, `POST /v1/embeddings` |
| OpenAI-compatible (your `base_url`) | e.g. `api.groq.com`, `api.mistral.ai`, plus front-end-offered OpenRouter / Together / DeepSeek | `POST {base_url}/chat/completions` |
| Ollama (local) | `localhost:11434` (configurable) | `POST /api/chat`, `/api/embed`, `/api/embeddings`, `GET /api/version`, `/api/tags`, `POST /api/pull` |

- **What is sent:** the system prompt (scoring rubric) plus item **titles and content snippets**.
  Content per item is **capped at 2000 characters** (`llm_judge.rs`). **No raw project code, file
  contents, or git history is ever sent.**
- **Privacy control:** Settings → Privacy → `llm_content_level`. Set to `titles_only` to send
  titles with **no** snippet body; default `full` sends the 2000-char-capped snippet.
- **Auth:** your key, sent as `x-api-key` (Anthropic) or `Authorization: Bearer` (OpenAI-compatible).
  Keys are stored only on your machine (keychain) and never sent anywhere but the provider you chose.
- **Retention (zero-retention defaults):** first-party **OpenAI** requests send `store: false`,
  opting out of OpenAI storing the completion for their dashboard/retrieval. **Anthropic** has no
  per-request retention control — zero-data-retention is an account-level agreement you make with
  Anthropic. **OpenAI-compatible** providers (Groq, Mistral, OpenRouter, …) are governed by each
  provider's own data policy; `store` is OpenAI-specific and is deliberately *not* sent to them
  (they may reject unknown fields, and it would have no effect). In every case your data goes only to
  the provider you chose — never to 4DA.
- **Fallback:** on a cloud failure (network, not rate-limit) 4DA falls back to local Ollama.
- **Disable:** remove the API key / select Ollama as the provider.

### 2b. Cloud translation (consent-gated)

Off by default. Only used if you enable a cloud translation provider and supply its key
(`src-tauri/src/translation_providers.rs`).

| Provider | Host | Endpoint |
|---|---|---|
| DeepL | `api-free.deepl.com` / `api.deepl.com` | `POST /v2/translate` |
| Microsoft Translator | `api.cognitive.microsofttranslator.com` | `POST /translate` |
| Google Translate | `translation.googleapis.com` | `POST /language/translate/v2` |

- **What is sent:** the text you asked to translate, plus your key.
- **Disable:** leave cloud translation off (the default); local translation paths send nothing.

### 2c. Dependency-vulnerability sync (OSV) — discloses local package names

Runs only **if 4DA has discovered local dependencies** (from your projects via ACE / lockfile
scanning). This is the **one flow that transmits data derived from your local content** — it sends
the **names of your packages and their ecosystems** (e.g. `tokio` / `crates.io`), **never your code
or file contents**. (`src-tauri/src/osv/`)

| Host | Endpoint | Data sent |
|---|---|---|
| `api.osv.dev` | `POST /v1/querybatch` | Your discovered package names + ecosystems, batched (≤1000/batch) |
| `osv-vulnerabilities.storage.googleapis.com` | `GET /{ecosystem}/all.zip` | Plain `GET` — downloads the public advisory mirror (no user data) |

- **Trigger:** background, at startup, **only if dependencies exist** and the local mirror was last
  synced **> 6 hours ago** (`app_setup.rs`).
- **Disable:** if 4DA has discovered no dependencies, nothing is sent. Removing/disabling ACE project
  scanning prevents dependency discovery and therefore this sync.

### 2d. License validation (Keygen)

Only contacted when you enter a license key (`src-tauri/src/settings/license/keygen.rs`).

- **Host:** `api.keygen.sh`
- **Endpoint:** `POST /v1/accounts/runyourempirehq/licenses/actions/validate-key`
- **Data sent:** `{"meta":{"key": <license key>}}` — the **license key only**. No device fingerprint,
  no machine identifiers, no usage data, no telemetry.
- **Offline:** if unreachable, the current tier is preserved (never downgraded). Successful
  validations cache locally for 24 h.
- **Offline signature check:** signed keys are *also* verified locally against an embedded
  ed25519/minisign key — purely cryptographic, no server contact.
- **Disable:** don't enter a license key (free tier makes no call).

### 2e. License recovery (user-initiated — the only 4DA-operated endpoint)

(`src-tauri/src/settings_commands_license.rs`)

- **Host:** `4da.ai` — **the only 4DA-operated server the app ever contacts.**
- **Endpoint:** `GET https://4da.ai/api/streets/activate?email={email}`
- **Trigger:** only when you click "Recover License by Email" in Settings.
- **Data sent:** your purchase email (query parameter). Returns the license key + tier.

### 2f. App updates (Tauri updater)

(`src-tauri/tauri.conf.json` → `plugins.updater`)

- **Host:** `github.com` (GitHub Releases)
- **Endpoint:** `GET /runyourempire/4DA/releases/latest/download/latest.json`
- **Trigger:** shortly after startup, once per session; silent on failure.
- **Data sent:** plain `GET` — no version reporting, no device info. Updates are
  **minisign-verified** against the embedded public key before install; the user must click to apply.

### 2g. Team relay (only if you create/join a team)

(`src-tauri/src/team_sync_scheduler.rs`) — there is **no hardcoded relay host**. The relay is a
**user-configured `relay_url`**; encrypted metadata (XChaCha20-Poly1305) is synced to it only if you
set up a team. With no team configured, zero calls are made. Disable by not joining a team.

### 2h. Developer Toolkit HTTP probe (manual only)

A user-triggered tool to test API endpoints. **Never automatic.** Restricted to an allowlist
(`src-tauri/src/toolkit_http.rs`): `api.openai.com`, `api.anthropic.com`,
`generativelanguage.googleapis.com`, `localhost`/`127.0.0.1`/`0.0.0.0`, `api.keygen.sh`,
`hacker-news.firebaseio.com`, `www.reddit.com`, `oauth.reddit.com`, `api.github.com`, `api.x.com`,
`export.arxiv.org`, `www.youtube.com`, `lobste.rs`, `dev.to`, `www.producthunt.com`. Anything off
the allowlist is rejected.

### 2i. Twitter / X (BYOK)

(`src-tauri/src/sources/twitter.rs`) — completely silent unless you provide an X API Bearer Token.

- **Host:** `api.x.com`
- **Endpoints:** `/2/users/by/username/{handle}`, `/2/users/{id}/tweets`,
  `/2/tweets/search/recent` (deep fetch).
- **Auth:** your Bearer Token, sent only to `api.x.com`. No key = zero calls.

---

## 3. Setup-time (one-time downloads)

Embeddings run **in-process via fastembed (ONNX Runtime) with zero network by default**. If the
ONNX runtime and model weights are not already bundled/cached, they are fetched **once** on first
use (`src-tauri/src/embeddings_providers/fastembed.rs`):

| What | Host | Endpoint |
|---|---|---|
| ONNX Runtime library | `github.com` | `/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-{platform}.{zip,tgz}` |
| Embedding model weights (`snowflake-arctic-embed-m`, ~220 MB) | HuggingFace Hub CDN (`huggingface.co` + its LFS/Xet CDN) | downloaded by the fastembed/hf-hub client on first init if not bundled |

- **Trigger:** first embedding init only, and only if not already bundled in the install or cached.
- **After setup:** embeddings are 100% local — zero network. If no provider is available at all,
  scoring degrades to keyword-only (still no network).

---

## 4. Never

4DA does **not** do any of the following — verifiable in source and via Wireshark:

- **No crash reporting.** The previous third-party crash reporter (Sentry) was **removed entirely**.
  In its place, you can export a **local, scrubbed diagnostic bundle on demand** (Settings → Privacy
  → Export diagnostics). It is assembled from the on-device log tail, scrubbed of usernames and
  secret-shaped tokens, **written to disk**, and **never transmitted** — you choose whether to attach
  it to a bug report (`src-tauri/src/diagnostics.rs`).
- **No telemetry / analytics.** Zero usage tracking. All telemetry/metrics tables are **local SQLite
  only** and never leave the machine.
- **No phoning home.** The only 4DA-operated endpoint is the user-initiated `4da.ai` license
  recovery (§2e). There is no background 4DA backend receiving data.
- **No raw-content transmission.** Project files, source code, file contents, and git history never
  leave your machine. The only data *derived* from local content that is sent anywhere is OSV
  **package names** (§2c) — not code.
- **No device fingerprinting.** License validation sends the key only (§2d).
- **No cookies, no tracking pixels, no third-party scripts.** The frontend loads zero external
  resources.
- **No accounts required** to run the app. No social/share integrations.

---

## Content Security Policy (CSP)

The Tauri webview enforces a strict CSP (`src-tauri/tauri.conf.json` → `app.security.csp`). The
`connect-src` allowlist for the frontend is exactly:

```
connect-src 'self'
  https://api.anthropic.com
  https://api.openai.com
  http://localhost:11434
  https://hacker-news.firebaseio.com
  https://export.arxiv.org
  https://www.reddit.com
  https://api.github.com
  https://api.keygen.sh
```

Any JavaScript attempting to contact a host outside this list is blocked by the webview engine.
(Most outbound calls are made by the **Rust backend**, not the frontend — the CSP governs the
webview layer; the full backend inventory is the tables above.)

---

## Deep link protocol

- **Scheme:** `4da://` — inbound only, lets external apps open 4DA to a specific view. No outbound
  network calls.

---

Everything above is verifiable: read the cited source files, or run Wireshark while 4DA is open and
match every connection against this manifest.
