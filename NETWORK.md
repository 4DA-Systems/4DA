# 4DA Network Manifest

4DA is local-first and privacy-first. This document lists **every** outbound network call the application makes. If you run Wireshark, every connection you see will be accounted for here.

No raw user data, project contents, or personal information ever leaves your machine.

---

## Source Fetching

These calls retrieve public developer content from the internet. Each source can be individually enabled or disabled in settings. All fetching uses a shared `reqwest::Client` with User-Agent `4DA/1.0` and 30-second timeout.

### Hacker News
- **Domain:** `hacker-news.firebaseio.com`
- **URLs:**
  - `GET https://hacker-news.firebaseio.com/v0/topstories.json`
  - `GET https://hacker-news.firebaseio.com/v0/newstories.json` (deep fetch)
  - `GET https://hacker-news.firebaseio.com/v0/beststories.json` (deep fetch)
  - `GET https://hacker-news.firebaseio.com/v0/askstories.json` (deep fetch)
  - `GET https://hacker-news.firebaseio.com/v0/showstories.json` (deep fetch)
  - `GET https://hacker-news.firebaseio.com/v0/item/{id}.json` (per-story details)
- **When:** Automatic, every 5 minutes (configurable). Deep fetch on first run.
- **Auth:** None. Public API, no key required.

### Reddit
- **Domain:** `www.reddit.com`
- **URLs:**
  - `GET https://www.reddit.com/r/{subreddit}/hot.json?limit={n}`
- **Subreddits (default):** programming, technology, machinelearning, rust, typescript, webdev, datascience
- **Subreddits (deep fetch):** 42 subreddits covering programming languages, ML/AI, DevOps, security, startups, and more
- **When:** Automatic, every 10 minutes. Deep fetch on first run.
- **Auth:** None. Public JSON API, no key required.

### arXiv
- **Domain:** `export.arxiv.org`
- **URLs:**
  - `GET https://export.arxiv.org/api/query?search_query={categories}&start=0&max_results={n}&sortBy=submittedDate&sortOrder=descending`
- **Categories (default):** cs.SE, cs.PL, cs.DB, cs.CR
- **Categories (deep fetch):** adds cs.DC, cs.HC, cs.IR
- **When:** Automatic, every 1 hour. Deep fetch on first run.
- **Auth:** None. Public API, no key required.

### GitHub
- **Domain:** `api.github.com`
- **URLs:**
  - `GET https://api.github.com/search/repositories?q={query}&sort=stars&order=desc&per_page={n}`
  - `GET https://api.github.com/repos/{owner}/{repo}/readme` (content scraping)
- **When:** Automatic, every 1 hour.
- **Auth:** None. Uses unauthenticated API (rate-limited to 10 requests/minute).
- **What it sends:** Search query containing language names and a date filter. No user data.

### RSS Feeds
- **Domains (defaults):**
  - `feeds.arstechnica.com`
  - `www.theverge.com`
  - `techcrunch.com`
  - `blog.rust-lang.org`
  - `engineering.fb.com`
  - `netflixtechblog.com`
  - `github.blog`
  - `blog.cloudflare.com`
  - `martinfowler.com`
  - `simonwillison.net`
  - `jvns.ca`
  - `danluu.com`
- **URLs:** `GET {feed_url}` for each configured RSS/Atom feed URL
- **When:** Automatic, every 30 minutes. User can add/remove feeds.
- **Auth:** None.

### YouTube
- **Domain:** `www.youtube.com`
- **URLs:**
  - `GET https://www.youtube.com/feeds/videos.xml?channel_id={channel_id}`
- **Channels (default):** 16 tech channels (Fireship, ThePrimeagen, Theo, Traversy Media, etc.)
- **When:** Automatic, every 30 minutes. User can customize channels.
- **Auth:** None. Uses public Atom RSS feeds, no API key required.

### Twitter/X
- **Domain:** `api.x.com`
- **URLs:**
  - `GET https://api.x.com/2/users/by/username/{handle}` (user ID lookup)
  - `GET https://api.x.com/2/users/{id}/tweets?max_results=10&...` (timeline)
  - `GET https://api.x.com/2/tweets/search/recent?query={query}&...` (deep fetch search)
- **When:** Only when user provides an X API Bearer Token (BYOK). Every 15 minutes if configured.
- **Auth:** User-provided Bearer Token sent as `Authorization: Bearer {token}`. **Token never leaves the machine** except in direct API calls to api.x.com.
- **Note:** If no API key is configured, this source is completely silent (zero network calls).

### Lobste.rs
- **Domain:** `lobste.rs`
- **URLs:**
  - `GET https://lobste.rs/hottest.json`
  - `GET https://lobste.rs/newest.json` (deep fetch)
- **When:** Automatic, every 10 minutes.
- **Auth:** None. Public API, no key required.

### Dev.to
- **Domain:** `dev.to`
- **URLs:**
  - `GET https://dev.to/api/articles?per_page=30&top=7`
  - `GET https://dev.to/api/articles?per_page=30&tag=programming` (deep fetch)
  - `GET https://dev.to/api/articles?per_page=30&tag=webdev` (deep fetch)
- **When:** Automatic, every 15 minutes.
- **Auth:** None. Public API, no key required.

### Product Hunt
- **Domain:** `www.producthunt.com`
- **URLs:**
  - `GET https://www.producthunt.com/feed`
- **When:** Automatic, every 1 hour.
- **Auth:** None. Public RSS feed.

### Article Scraping (all sources)
- **Domains:** Any domain linked by the sources above
- **When:** After fetching items, 4DA scrapes the linked article URL to extract content (text only, no images/media). This applies to HN, Reddit (link posts), Lobste.rs, and RSS items.
- **Timeout:** 2-10 seconds per article. Rate-limited with 100ms delays between requests.
- **What it sends:** A standard HTTP GET with User-Agent `4DA/1.0`. No cookies, no login, no tracking headers.

---

## Network Connectivity Check

Before fetching sources, 4DA checks if the network is available using a parallel race against three targets:

- `HEAD https://1.1.1.1/cdn-cgi/trace` (Cloudflare)
- `HEAD https://dns.google/resolve?name=example.com` (Google DNS)
- `HEAD https://httpbin.org/get` (httpbin)

**When:** Once per analysis cycle, before source fetching begins.
**Timeout:** 4 seconds. First response wins.
**What it sends:** HEAD request only (no body). No user data.
**If offline:** 4DA falls back to cached content and continues working.

---

## LLM Integration

LLM calls are used for relevance judging (scoring articles against your developer context). All LLM usage is subject to a configurable daily token limit.

### Ollama (Local, Default)
- **Domain:** `localhost:11434` (configurable)
- **URLs:**
  - `POST http://localhost:11434/api/chat` (inference)
  - `POST http://localhost:11434/api/embed` (batch embeddings)
  - `POST http://localhost:11434/api/embeddings` (single-item fallback)
  - `GET http://localhost:11434/api/version` (status check)
  - `GET http://localhost:11434/api/tags` (list models)
  - `POST http://localhost:11434/api/pull` (auto-pull missing models at startup)
- **When:** During analysis, embedding generation, model warming at startup, and Ollama status checks.
- **Auth:** None. Localhost only, no data leaves your machine.
- **Models used:** `nomic-embed-text` (embeddings), user-configured LLM model (default: `llama3`).

### Anthropic Claude (User-Configured)
- **Domain:** `api.anthropic.com`
- **URLs:**
  - `POST https://api.anthropic.com/v1/messages`
- **When:** Only when user configures Anthropic as their LLM provider. Used for relevance judging.
- **Auth:** User-provided API key sent as `x-api-key` header. **Key never stored remotely.**
- **What it sends:** System prompt (scoring rubric) + article titles/snippets + user's developer context summary. No raw project code.
- **Fallback:** If Anthropic API fails (network error, not rate-limit), automatically falls back to local Ollama.

### OpenAI (User-Configured)
- **Domain:** `api.openai.com`
- **URLs:**
  - `POST https://api.openai.com/v1/chat/completions` (LLM inference)
  - `POST https://api.openai.com/v1/embeddings` (text embeddings, model: `text-embedding-3-small`)
- **When:** Only when user configures OpenAI as their LLM provider. Also used for embeddings if OpenAI key is provided.
- **Auth:** User-provided API key sent as `Authorization: Bearer {key}`. **Key never stored remotely.**
- **What it sends:** Same as Anthropic (scoring context + articles). For embeddings: article title + content text.
- **Fallback:** If OpenAI API fails, automatically falls back to local Ollama.

---

## Embedding Generation

Embeddings convert text into vectors for semantic search. Provider selection follows this priority:

1. **OpenAI** (`text-embedding-3-small`) -- if OpenAI is the configured provider, or if a dedicated OpenAI embedding key is set
2. **Ollama** (`nomic-embed-text`) -- if Ollama is configured, or as fallback for Anthropic users
3. **Zero vectors** -- if no embedding provider is available, scoring degrades to keyword-only (no network calls)

The domains and URLs are the same as listed under LLM Integration above.

---

## License Validation

### Keygen (Pro/Team License)
- **Domain:** `api.keygen.sh`
- **URLs:**
  - `POST https://api.keygen.sh/v1/accounts/runyourempirehq/licenses/actions/validate-key`
- **When:** When user enters a license key, and periodically (cached for 24 hours). **Not called if no license key is entered.**
- **What it sends:** The license key only. No device fingerprint, no telemetry, no usage data.
- **Offline behavior:** If the API is unreachable, the current tier is preserved (no downgrade). 4DA never blocks functionality due to network failure.
- **Caching:** Successful validations are cached locally in `data/license_cache.json` for 24 hours.

### Ed25519 License Verification (Offline)
- **No network calls.** Signed license keys (`4DA-{payload}.{signature}`) are verified locally using an embedded ed25519 public key. This is purely cryptographic, no server contact required.

---

## Update Checks

### Tauri Updater Plugin
- **Domain:** `github.com` (GitHub Releases)
- **URLs:**
  - `GET https://github.com/runyourempire/4DA/releases/latest/download/latest.json`
- **When:** 5 seconds after app startup, once per session. Silent on failure.
- **What it sends:** Standard HTTP GET. No user data, no version reporting, no device info.
- **User action:** If an update is available, the user is shown a notification and must explicitly click to download and install.

---

## Toolkit HTTP Probe (User-Initiated Only)

The Developer Toolkit includes an HTTP probe tool that lets users manually test API endpoints. This is **never automatic** -- only triggered by explicit user action.

- **Domain allowlist:** Requests are restricted to these domains only:
  - `api.openai.com`, `api.anthropic.com`, `generativelanguage.googleapis.com`
  - `localhost`, `127.0.0.1`, `0.0.0.0`
  - `api.keygen.sh`
  - `hacker-news.firebaseio.com`, `www.reddit.com`, `oauth.reddit.com`
  - `api.github.com`, `api.x.com`, `export.arxiv.org`
  - `www.youtube.com`, `lobste.rs`, `dev.to`, `www.producthunt.com`
- **Blocked:** Any domain not on the allowlist is rejected. This prevents data exfiltration.

---

## Deep Link Protocol

- **Scheme:** `4da://`
- **Purpose:** Allows external applications to open 4DA to specific views.
- **Network:** No outbound calls. This is an inbound-only protocol handler.

---

## Content Security Policy (CSP)

The Tauri webview enforces a strict CSP that limits which domains the frontend can connect to:

```
connect-src 'self'
  https://api.anthropic.com
  https://api.openai.com
  http://localhost:11434
  https://hacker-news.firebaseio.com
  https://export.arxiv.org
  https://www.reddit.com
  https://api.github.com
  https://www.googleapis.com
```

Any JavaScript attempting to contact domains outside this list will be blocked by the browser engine.

---

## What 4DA Does NOT Do

- **No telemetry.** Zero usage tracking, zero analytics, zero crash reporting.
- **No phoning home.** 4DA has no server. There is no `4da.ai` backend receiving data.
- **No user data transmission.** Your project files, code, git history, and developer context never leave your machine. The only thing sent externally is article text to your chosen LLM provider for scoring -- and only if you configure one.
- **No tracking pixels or third-party scripts.** The frontend loads zero external resources.
- **No cookies.** 4DA does not set or send cookies to any external service.
- **No device fingerprinting.** License validation sends only the license key, not machine identifiers.
- **No background uploads.** All data flows are inbound (fetching content) or to user-configured APIs.
- **No social/share features.** No integration with social platforms that could leak usage patterns.

---

## Summary Table

| Category | Domain(s) | Automatic? | Requires User Config? |
|---|---|---|---|
| Hacker News | hacker-news.firebaseio.com | Yes (5 min) | No |
| Reddit | www.reddit.com | Yes (10 min) | No |
| arXiv | export.arxiv.org | Yes (1 hr) | No |
| GitHub | api.github.com | Yes (1 hr) | No |
| RSS Feeds | Various (12 defaults) | Yes (30 min) | No (customizable) |
| YouTube | www.youtube.com | Yes (30 min) | No (customizable) |
| Twitter/X | api.x.com | No | Yes (API key) |
| Lobste.rs | lobste.rs | Yes (10 min) | No |
| Dev.to | dev.to | Yes (15 min) | No |
| Product Hunt | www.producthunt.com | Yes (1 hr) | No |
| Article scraping | Any linked URL | Yes | No |
| Connectivity check | 1.1.1.1, dns.google, httpbin.org | Yes (per cycle) | No |
| Ollama (local) | localhost:11434 | Yes | No |
| Anthropic | api.anthropic.com | No | Yes (API key) |
| OpenAI | api.openai.com | No | Yes (API key) |
| License validation | api.keygen.sh | No | Yes (license key) |
| Update check | github.com | Yes (once/session) | No |
| Toolkit probe | Allowlisted domains only | No (manual) | No |
