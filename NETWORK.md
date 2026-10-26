# 4DA Network Manifest

**This document lists every outbound network connection 4DA's own code makes, and the one local
port it listens on (§1f).** Run Wireshark while 4DA is running and every connection made by the
`fourda` process is accounted for below. On Windows you will also see connections from
`msedgewebview2.exe`: that is Microsoft's WebView2 browser runtime, which draws 4DA's window and
talks to Microsoft on its own (§5). Your
raw project files, source code, git history, file contents, and personal information **never leave
your machine** by default. Every flow that transmits anything *derived* from your local content —
chiefly the names and versions of your dependencies, sent to OSV.dev and the package registries
(never your code) — is
listed exhaustively in §4 ("What IS derived from local content and does leave"). The one opt-in
setting that sends text from your local files is cloud embeddings (`allow_cloud_embeddings`, off by
default, §2a).

4DA is local-first and privacy-first. There is **no telemetry, no analytics, and no crash
reporting** (see "Never" at the bottom). The only 4DA-operated server the app ever contacts is
`4da.ai`, for exactly two licence operations: a user-initiated recovery lookup (§2e) and, for paid
**subscription** keys only, a key-only renewal near expiry (§2k). Everything else is either a public third-party
content API, a cloud service you explicitly configured (BYOK LLM / translation / your own SMTP
server), or a one-time setup download.

---

## HTTP client identity

All shared outbound requests use a pooled `reqwest` client
(`src-tauri/src/http_client.rs`, and `src-tauri/src/sources/mod.rs::SHARED_CLIENT`) with:

- **User-Agent:** `Mozilla/5.0 (compatible; desktop-app)` — the default for source fetching,
  connectivity checks, license validation, and article scraping.
- **Exceptions (purpose-built User-Agents):**
  - crates.io adapter → `4DA-Developer-OS/1.0 (https://4da.ai)` (`sources/crates_io.rs`); the
    release-changelog lane sends the same to `static.crates.io` and `4DA-Developer-OS/1.0` to
    `registry.npmjs.org` (`release_changelog/fetch.rs`); the opt-in GitHub release-notes fallback
    sends `4DA-Developer-OS/1.0 (https://4da.ai)` to `crates.io`, `registry.npmjs.org` and
    `api.github.com` (`release_changelog/github.rs`, §2m)
  - HuggingFace / PapersWithCode / GitHub Advisory adapters → `4DA-Developer-OS/1.0`
    (`sources/huggingface.rs`, `sources/papers_with_code.rs`, `sources/cve.rs`)
  - OSV dependency sync → `4DA/1.0 (local-osv-mirror)` (`osv/sync.rs`)
  - Mastodon / Lemmy adapters → `4DA/1.0 (+https://4da.ai)` (`sources/mastodon.rs`,
    `sources/lemmy.rs`)
  - Feed checks you trigger in Settings / Toolkit → `Mozilla/5.0 (compatible; 4DA/1.0)`
    (`source_config_feeds.rs`) and `4DA/1.0 Feed Tester` (`toolkit_intelligence.rs`)
  - Team relay (only if you join/create a team) → `4DA-TeamSync/1.0` (`http_client.rs`)
- No cookies, no login, no tracking headers are sent on any request.

---

## 1. Always-on

These run automatically while the app is open. Every source can be individually enabled/disabled in
**Settings → Sources**.

### 1a. Connectivity pre-check (before every fetch cycle)

Before fetching sources, 4DA races a `HEAD` request against five targets and treats the network
as online as soon as any one succeeds (`src-tauri/src/source_fetching/fetcher.rs`):

| Host | Request |
|---|---|
| `api.github.com` | `HEAD https://api.github.com` |
| `dns.google` (Google DNS) | `HEAD https://dns.google/resolve?name=example.com` |
| `hacker-news.firebaseio.com` | `HEAD https://hacker-news.firebaseio.com/v0/topstories.json` |
| `1.1.1.1` (Cloudflare) | `HEAD https://1.1.1.1/cdn-cgi/trace` |
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
| npm | `registry.npmjs.org` | `/{package}/latest` | auto | None |
| PyPI | `pypi.org` | `/pypi/{package}/json` | auto | None |
| HuggingFace | `huggingface.co` | `/api/models` | auto | None |
| PapersWithCode | `huggingface.co` | `/api/daily_papers` (PwC API now redirects here) | auto | None |
| Stack Overflow | `api.stackexchange.com` | `/2.3/questions?...&site=stackoverflow&tagged={tag}&filter=withbody` | auto | None |
| Go modules | `proxy.golang.org` | `/{module}/@latest`, one request per module your `go.mod` files declare (200 ms apart). The global `index.golang.org` feed is never read | auto, only if Go modules were discovered | Your **full Go module paths** (see the derived-content table in §4) |
| GitHub Advisories | `api.github.com` | `/advisories?per_page=…&sort=published&direction=desc&published=>={date}[&ecosystem={eco}]` (paged via `Link`) | auto | Unauthenticated. Ecosystem names + a date filter. UA `4DA-Developer-OS/1.0` (`sources/cve.rs`) |
| OSV advisories | `api.osv.dev` | `POST /v1/query`, `POST /v1/querybatch`, `GET /v1/vulns/{id}` | auto, only if dependencies were discovered | Your package names + ecosystems, **plus the installed version of each one whenever the lockfile gives it** (`sources/osv.rs` `fetch_ecosystem_vulns` / `fetch_batch_vuln_refs`, `sources/osv_live.rs`) — see §2c |
| Mastodon | `hachyderm.io` | `GET /api/v1/timelines/tag/{tag}?limit={n}` (n ≤ 40); fallback `GET /tags/{tag}.rss` | auto | None. Public hashtag timelines for fixed dev tags (`rust`, `programming`, `javascript`, `python`, `golang`, `webdev`, `linux`, `devops`, `opensource`, `security`). UA `4DA/1.0 (+https://4da.ai)` (`sources/mastodon.rs`) |
| Lemmy | `programming.dev` | `GET /api/v3/post/list?type_=All&sort=Hot&limit={n}` (n ≤ 50); fallback `GET /feeds/all.xml?sort=Hot` | auto | None. UA `4DA/1.0 (+https://4da.ai)` (`sources/lemmy.rs`) |

**RSS default hosts** (`sources/rss.rs`, user-customizable): `feeds.arstechnica.com`,
`www.theverge.com`, `techcrunch.com`, `blog.rust-lang.org`, `engineering.fb.com`, `medium.com`,
`github.blog`, `blog.cloudflare.com`, `martinfowler.com`, `simonwillison.net`, `jvns.ca`,
`danluu.com`.

- **Disable any source:** Settings → Sources. A disabled source makes **zero** network calls.

### 1c. Article scraping

Many items arrive as a bare title or a short teaser. After each background fetch
(`content_enrichment.rs`, run at the end of every cache fill), 4DA fetches readable **text only**
(no images or media) for recent items that are still thin, so they are scored on what they say:

- **What is fetched:** the story link for HN, Lobsters, RSS and Lemmy items; the `[link]` target of
  a Reddit link post; the first outbound link in a Mastodon or Bluesky post (not mentions or
  hashtags); a dev.to post's full text from `dev.to/api/articles/{user}/{slug}`. Stack Overflow
  question bodies come with the list call above (`filter=withbody`), with no extra request.
- **Never fetched:** items that already carry their text (arXiv, CVE/OSV, registries, GitHub), and
  X/Twitter, Facebook, Instagram, LinkedIn, YouTube, Reddit and HN discussion pages.
- **Hosts:** any domain linked by the sources above, plus `dev.to`.
- **Data sent:** a plain `GET` with the default User-Agent. No cookies, no login. Internal and
  private network addresses are refused (SSRF guard).
- **Budget:** at most 60 fetches per cycle, 4 at a time, only for items from the last 48 hours; the
  same host is spaced 0.5 s apart (dev.to 1.1 s); 10 s timeout per page; a failed page is retried
  once, 6 hours later.
- **Scope:** only items from enabled sources are ever fetched, so disabling a source stops this too.

### 1d. Model-pricing refresh

On startup, 4DA refreshes its cost/context-window table for known LLM models
(`src-tauri/src/model_registry.rs::refresh_registry`, called from `app_setup.rs`):

- **Host:** `raw.githubusercontent.com`
- **Endpoint:** `GET .../BerriAI/litellm/main/model_prices_and_context_window.json`
- **Trigger:** once at startup, fire-and-forget, **at most once per 24 h** (cached otherwise).
- **Data sent:** plain `GET`, no user data, no parameters. Falls back to bundled/cached pricing on
  failure.
- **Disable:** offline operation skips it silently; the bundled table is used.

### 1e. Release changelogs (registry package archives)

When a release of one of your dependencies is graded as news for a project that pins it (a
breaking upgrade, a new minor, or a yanked pin), 4DA reads the changelog that ships **inside the
package itself** to say what changed between your version and the new one
(`src-tauri/src/release_changelog/`):

| Host | Request | Data sent |
|---|---|---|
| `static.crates.io` | `GET /crates/{crate}/{crate}-{version}.crate` | The crate name and the **new, public** version. User-Agent `4DA-Developer-OS/1.0 (https://4da.ai)` |
| `registry.npmjs.org` | `GET /{package}/{version}` (version manifest, for `dist.tarball`), then that tarball | The package name and the **new, public** version |

- **Same party, nothing new disclosed:** `static.crates.io` is crates.io's own download host,
  and both registries already receive these package names from the release watch (§1b). The
  version sent is the one the registry itself announced. **Your installed version is never sent**;
  the range is cut on your machine.
- **Never contacted for this by default:** GitHub, docs sites, or any host a registry document
  points to. The archive URL must be https on one of the two hosts above, and a redirect off them
  is not followed. If the package ships no changelog, the card says so; nothing else is fetched,
  **unless you turn on "Release notes from GitHub" (Settings → Privacy, off by default)**, which
  reads the project's GitHub releases for that one release instead (§2m).
- **Read, never run:** archive contents are read in memory as text. Nothing is written to disk
  or executed; only a root-level changelog file is kept. Archives over 5 MB compressed or 64 MB
  uncompressed are refused; a single file over 2 MB is skipped.
- **Budget:** each archive is fetched **once** and cached in the local database. In the
  background (after a fetch cycle) at most 6 archives per cycle, 1.1 s apart; opening a release
  card reads its archive on demand if the background pass has not. 30 s timeout per archive.
- **Disable:** disabling the crates.io / npm sources (Settings → Sources) stops the release rows,
  and with them this lane.

### 1f. Local listener: Signal Terminal on `127.0.0.1:4446` (inbound, this machine only)

Not an outbound connection; listed so that nothing 4DA listens on is undocumented. Every time the
app starts it serves a small web page and read-only JSON API, the **Signal Terminal**, for a browser
tab or your own scripts on this machine: signals, the briefing, radar, decisions, knowledge gaps,
search over already-scored items, and a live event stream (`src-tauri/src/signal_terminal.rs`
`start_signal_terminal`, called from `app_setup.rs` during startup).

- **Address:** bound to `127.0.0.1` only, port **4446** (4447 in development builds). Other machines
  on your network cannot reach it.
- **Host check:** every request, including the page itself, must carry a `Host` header of
  `127.0.0.1:<port>`, `localhost:<port>` or `[::1]:<port>`; anything else gets `403`. This stops a
  web page from reaching it through DNS rebinding. No cross-origin (CORS) headers are sent.
- **Token:** every `/api/*` route requires an `X-4DA-Token` header equal to a 32-character random
  token that 4DA generates on first start and keeps in `signal_terminal_token.txt` in its data
  directory (Windows `%APPDATA%\com.4da.app\data`). The token survives restarts; delete the file and
  a new one is generated at the next start. A missing or wrong token is `401` (compared in constant
  time), with no exception for local callers. `/api/stream` also accepts `?token=` because a
  browser `EventSource` cannot set headers. The routes without a token serve fixed UI pages only.
- **What it exposes:** the same intelligence the app shows you. `/api/decisions` and `/api/gaps`
  include local project paths. Any program running under your user account can read the token
  file and therefore the API — the same account can already read 4DA's database directly.
- **Nothing outbound:** the routes read 4DA's in-memory state and local database; none of them
  makes a network request.
- **Disable:** there is no setting to turn it off yet. It stops when 4DA quits. If the port is
  already taken (for example by a second 4DA instance) it simply does not start.

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
  titles with **no** snippet body; default `full` sends the 2000-char-capped snippet. The setting
  covers every prompt that carries item text: the ingest and rerank judges, the verdict re-judge
  drain, search synthesis, briefings and channels (`src-tauri/src/llm_egress.rs`; a pre-commit gate
  fails any new model call that bypasses it). A model on this machine (Ollama, or a `base_url` on
  localhost) is exempt, because nothing leaves the machine.
- **Auth:** your key, sent as `x-api-key` (Anthropic) or `Authorization: Bearer` (OpenAI-compatible).
  Keys are stored in your OS credential store (Windows Credential Manager / macOS Keychain / Linux
  Secret Service) and removed from `settings.json` once the credential store has verified them. If
  the credential store is unavailable they stay in `settings.json`, restricted to your user account
  (owner-only permissions), and Settings shows which storage is in use. Either way they are never
  sent anywhere but the provider you chose.
- **Key check / model list:** when you test a key or open the model picker, 4DA calls that same
  provider: `POST api.anthropic.com/v1/messages` (a 1-token "hi"), `GET api.openai.com/v1/models`,
  `POST {base_url}/chat/completions` (a 1-token "hi") or `GET {base_url}/models`
  (`src-tauri/src/settings/validation.rs`, `settings_commands_llm/mod.rs`). Local-server detection
  probes only this machine: `localhost:11434/api/version`, `localhost:1234/v1/models`,
  `localhost:8080/v1/models`, `localhost:1337/v1/models`.
- **Embeddings:** `POST api.openai.com/v1/embeddings` is used **only** if you turn on
  `allow_cloud_embeddings` (off by default; `src-tauri/src/embeddings.rs`). Setting an LLM key
  alone never routes embeddings to the cloud. With it on, the text being embedded goes to OpenAI —
  that includes **chunks of text from your local files** (README and other indexed context chunks,
  `ace/readme_indexing.rs`, `reembed_space.rs`), not just item titles. See §4.
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
scanning). It sends the **names of your packages and their ecosystems** (e.g. `tokio` /
`crates.io`), **never your code or file contents**. (`src-tauri/src/osv/`) This mirror sync sends
no versions; the advisory source in §1b does send installed versions to `api.osv.dev`. It is one of
several flows that send names derived from your projects — the package registries and the Go
module proxy (§1b, §1e) receive them too; §4 has the complete list.

| Host | Endpoint | Data sent |
|---|---|---|
| `api.osv.dev` | `POST /v1/querybatch` | Your discovered package names + ecosystems, batched (≤1000/batch) |
| `osv-vulnerabilities.storage.googleapis.com` | `GET /{ecosystem}/all.zip` | Plain `GET` — downloads the public advisory mirror (no user data) |
| `registry.npmjs.org` | `GET /{package}` (abbreviated packument, `Accept: application/vnd.npm.install-v1+json`) | The package name only |
| `crates.io` | `GET /api/v1/crates/{crate}` | The crate name only. User-Agent `4DA-Developer-OS/1.0 (https://4da.ai)` |
| `pypi.org` | `GET /pypi/{package}/json` | The package name only |

- **Registry release check** (the last three rows, `src-tauri/src/osv/release_facts.rs`): after each
  sync, for packages that have a **version-confirmed advisory** only, 4DA reads which releases exist
  and which the registry withdrew (crates.io and PyPI *yanked*, npm *deprecated*), so a fix target
  never names one. At most 300 packages per sync (packages with no answer yet first, then the
  oldest), at most 4 npm/PyPI requests at a time, none started after 150 s; each re-read at most
  once a day; the answer is
  kept in the local database. **Your installed version is never sent.** Each registry is read only
  while its source (npm / crates.io / PyPI, Settings → Sources) is enabled; a disabled source makes
  no call here either. Requests to crates.io are spaced 1.1 s apart. Go modules are not checked
  (their `retract` directives are not read). Offline, targets come from the advisory data alone,
  as before.
- **Parent releases (npm, `src-tauri/src/osv/parent_release_load.rs`):** when a vulnerable npm
  copy is transitive and the installed package that pulls it in pins it below its fix, the same
  abbreviated document (`GET /{package}`, name only) is also read for that **parent** — and for the
  declared dependency the parent arrives through — so the plan can name the parent release whose
  own requirement admits a fixed copy. These are packages already installed in your projects; only
  their names are sent, never a version. Same 60-per-sync cap, daily re-read, local cache and
  npm-source toggle as above; the per-release requirements kept are only those on the vulnerable
  package.
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

### 2e. License recovery (user-initiated)

(`src-tauri/src/settings_commands_license.rs`)

- **Host:** `4da.ai` — the only 4DA-operated server the app ever contacts (see also §2k).
- **Endpoint:** `GET https://4da.ai/api/license/activate?email={email}`
  (builds before 2026-08-20 call the same endpoint at its old `/api/streets/activate` path)
- **Trigger:** only when you click "Recover License by Email" in Settings.
- **Data sent:** your purchase email (query parameter).
- **Data returned:** **not the key.** The server emails the key to the address on file and
  replies with the same acknowledgement whether or not that address holds a licence. The app
  cannot prove you own the address you typed, so the key goes to the mailbox instead — that
  also means the endpoint can't be used to look up whether someone else is a subscriber.

### 2f. App updates (Tauri updater)

(`src-tauri/tauri.conf.json` → `plugins.updater`)

- **Host:** `github.com` (GitHub Releases)
- **Endpoint:** `GET /4DA-Systems/4DA/releases/download/desktop-latest/latest.json`
  (GitHub serves release downloads by redirecting to its asset CDN on a `*.githubusercontent.com`
  host; an accepted update's installer is fetched the same way)
- **Trigger:** shortly after startup, once per session; silent on failure.
- **Data sent:** plain `GET` — no version reporting, no device info. Updates are
  **minisign-verified** against the embedded public key before install; the user must click to apply.

### 2g. Team relay (only if you create/join a team)

(`src-tauri/src/team_sync_scheduler.rs`) — there is **no hardcoded relay host**. The relay is a
**user-configured `relay_url`**; encrypted metadata (XChaCha20-Poly1305) is synced to it only if you
set up a team. With no team configured, zero calls are made. Disable by not joining a team.

**Not in release builds:** team sync is compiled only with the `team-sync` Cargo feature, and
outbound webhooks (`webhooks/`, `POST` to a URL you register, signed `X-4DA-Signature-256`) and SSO
(`sso_crypto.rs`, `GET {issuer}/.well-known/openid-configuration` + its `jwks_uri` on the identity
provider you configure) only with the `enterprise` feature. Neither feature is in the default build
(`src-tauri/Cargo.toml`: `default = ["fastembed-local"]`), so shipped builds contain no code for them.

### 2h. Developer Toolkit HTTP probe (manual only)

A user-triggered tool to test API endpoints. **Never automatic.** Restricted to an allowlist
(`src-tauri/src/toolkit_http.rs`): `api.openai.com`, `api.anthropic.com`,
`generativelanguage.googleapis.com`, `api.keygen.sh`,
`hacker-news.firebaseio.com`, `www.reddit.com`, `oauth.reddit.com`, `api.github.com`, `api.x.com`,
`export.arxiv.org`, `www.youtube.com`, `lobste.rs`, `dev.to`, `www.producthunt.com`, plus
`localhost`/`127.0.0.1`/`0.0.0.0` **on the Ollama port 11434 only** (no other local service is
reachable). The host is taken from a real URL parser and must match exactly; URLs carrying a
username/password are rejected, and every redirect hop is re-checked against the same allowlist.
Anything off the allowlist is rejected.

**Feed checks (manual only).** When you add or test a feed, 4DA fetches the URL you typed — a plain
`GET`, internal/private addresses refused — to confirm it is a valid RSS/Atom feed or to discover the
feed link on that page (`source_config_feeds.rs::validate_rss_feed`, `toolkit_intelligence.rs::toolkit_test_feed`).
Adding a YouTube channel fetches `www.youtube.com/feeds/videos.xml?channel_id={id}` once to check it.

### 2i. Twitter / X (BYOK)

(`src-tauri/src/sources/twitter.rs`) — completely silent unless you provide an X API Bearer Token.

- **Host:** `api.x.com`
- **Endpoints:** `/2/users/by/username/{handle}`, `/2/users/{id}/tweets`,
  `/2/tweets/search/recent` (deep fetch).
- **Auth:** your Bearer Token, sent only to `api.x.com`. No key = zero calls.

### 2j. MCP server `dependency_check` (only when an agent calls the tool)

The `@4da/mcp-server` tool `dependency_check` (`src/live/npm-packument.ts` in [4DA-Systems/4da-mcp-server](https://github.com/4DA-Systems/4da-mcp-server),
`crates-versions.ts`) judges a version an agent is about to install. It runs only when an agent
calls it, and sends nothing while `FOURDA_OFFLINE=true`.

| Host | Endpoint | Data sent |
|---|---|---|
| `registry.npmjs.org` | `GET /{package}` (full packument, `Accept: application/json`) | **Package name only.** Cached on disk with its ETag; repeat reads send `If-None-Match` (the registry's own ETag) |
| `crates.io` | `GET /api/v1/crates/{crate}/versions?per_page=100&sort=semver` (paged), `GET /api/v1/crates/{crate}` | **Crate name only.** User-Agent `4DA-MCP-Server/1.0 (https://4da.ai)`; at most one request per second |
| `index.crates.io` | `GET /{prefix}/{crate}` (sparse index) | **Crate name only** |
| `api.osv.dev` | `POST /v1/querybatch`, `GET /v1/vulns/{id}` | Package names **and versions** (the installed and the proposed one), as `vulnerability_scan` already sends |

### 2k. License renewal (Signal subscriptions only)

(`src-tauri/src/settings/license/renewal.rs`, server `site/functions/api/license/renew.js`)

A subscription key carries its own expiry (monthly ~35 days, annual ~1 year). So that a paying
subscriber is never dropped to Free between billing periods, the app renews the key itself.

- **Host:** `4da.ai`
- **Endpoint:** `POST https://4da.ai/api/license/renew`
- **Data sent:** `{"key": <your current licence key>}` — **the key only** (it already embeds the
  purchase email). No device fingerprint, machine identifiers, usage data or telemetry.
- **When:** only if you hold a monthly/annual Signal key **and** it expires within 10 days (or
  expired less than 60 days ago). First check ~2 minutes after launch, then at most every 12 hours —
  in practice about once per billing period. Also once when you paste an **expired** subscription key
  into Settings → License, to exchange it for the current one. **Free users, lifetime keys and Keygen
  keys never call.**
- **Data returned:** a renewed key valid to the end of the period you have paid for (checked live
  against Stripe; at most 7 days ahead while a failed card is being retried), or "current" /
  "not entitled". Nothing is stored on our side beyond the key already kept against your Stripe
  customer record, plus a one-time marker that your app renews by itself — which stops the monthly
  email carrying a replacement key.
- **Offline / refused:** the current key is kept and simply runs to its own expiry — the call can
  never downgrade you.
- **Disable:** remove the licence key (Settings → License). A cancelled subscription stops renewing
  on its own.

The installed version and the proposed version are never sent to a registry: both are picked out
of the full release list on your machine. Only OSV receives versions.

### 2l. Digest email over SMTP (opt-in)

(`src-tauri/src/digest_email.rs`, `monitoring_jobs.rs`) — silent unless you configure **both** a
digest email address and an SMTP server in the digest settings. Mail goes straight from your
machine to the SMTP server you named; 4DA operates no mail server and never sees it.

- **Host / port:** whatever you entered (`digest.smtp.host`, `digest.smtp.port`). No default host.
- **Data sent:** your SMTP username and password (SMTP AUTH), the from/to addresses, and the
  digest itself — period, item counts, source and topic breakdown, and the top items' titles,
  links, relevance scores, the topics of yours they matched, and any summary / signal note. "Send test email" sends a fixed one-line message instead of the digest.
- **Encryption:** with `use_tls = true` the connection is upgraded with STARTTLS before
  authenticating. **With `use_tls = false`, your SMTP credentials and the digest are sent
  unencrypted** over the network — only use that for a mail relay on your own machine or LAN.
- **Trigger:** on the digest schedule (daily/weekly) while the app runs, plus the manual test.
- **Disable:** clear the digest email address or the SMTP settings.

### 2m. Release notes from GitHub (opt-in, off by default)

(`src-tauri/src/release_changelog/github.rs`) — silent unless you turn on **Settings → Privacy →
Release notes from GitHub** (`privacy.github_release_notes`, default `false`). With it on, when the
release lane (§1e) finds nothing for a graded release (the package ships no changelog, the
changelog cannot be read, or it has no entry for the release), 4DA reads the project's own GitHub
release notes for that release instead:

| Host | Request | When | Data sent |
|---|---|---|---|
| `crates.io` / `registry.npmjs.org` | `GET /api/v1/crates/{crate}` / `GET /{package}/{version}` | only if the setting is on and the package archive yields nothing for the release | The package name and the new public version — the same registries and names as §1b/§1e; read for the `repository` URL in the registry's own metadata |
| `api.github.com` | `GET /repos/{owner}/{repo}/releases?per_page=100&page={n}` (at most 3 pages) | only if the setting is on, the archive yields nothing, and the registry entry links a `github.com` repository | **The GitHub repository of that dependency**, so GitHub learns which repositories you depend on. Unauthenticated (no token, no account), User-Agent `4DA-Developer-OS/1.0 (https://4da.ai)` |

- **New party:** this is the only flow that tells GitHub which of your dependencies' repositories
  you read. That is why it is off by default. Your installed version is not sent; pages are read
  newest first and stop at the first release at or below it, so the number of pages requested
  depends on how far behind you are.
- **Only github.com, only the registry's link:** the repository comes from the package's registry
  metadata, never guessed from its name; a repository on any other host is skipped, and a
  redirect off the three hosts above is not followed. A release counts only when its tag names an
  exact version of the package (`1.2.3`, `v1.2.3`, monorepo `pkg@1.2.3` / `pkg-v1.2.3`).
- **Rate limits:** GitHub allows 60 unauthenticated requests an hour. A rate-limited answer is
  shown on the card ("try again after HH:MM UTC"), is never cached, and 4DA sends nothing more to
  GitHub until the limit resets.
- **Read, never run:** release notes are third-party text, parsed as data by the same parser as a
  package changelog (control and bidirectional characters stripped, entries capped) and shown on
  the card with "From the project's GitHub releases". Answers are cached locally (found notes 120
  days, "no release found" 24 h) under a parser version, so each release is read once.
- **Disable:** turn the setting off. Nothing is sent to GitHub for release notes while it is off.

---

## 3. Setup-time (one-time downloads)

Embeddings run **in-process via fastembed (ONNX Runtime) with zero network** once the model is on
disk. The ONNX runtime ships in the installer. The embedding model does **not** (AD-054: the
dependency engine needs no embeddings; only opt-in interests and natural-language search use them),
so it is fetched **once, on demand** (`src-tauri/src/embeddings_providers/model_store.rs`):

| What | Host | Endpoint |
|---|---|---|
| Embedding model (`nomic-embed-text-v1.5`, fp16 ONNX + tokenizer, 5 files, ~262 MB) | `huggingface.co`, which redirects to its CDN (`cdn.hf.co` / Xet storage) | `GET /nomic-ai/nomic-embed-text-v1.5/resolve/e9b6763023c676ca8431644204f50c2b100d9aab/{onnx/model_fp16.onnx, tokenizer.json, config.json, special_tokens_map.json, tokenizer_config.json}` (a `Range` header resumes a partial file) |
| ONNX Runtime library (only if missing from the install, e.g. a hand-built binary) | `github.com` | `/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-{platform}.{zip,tgz}` |

- **What is sent:** plain `GET`s for those fixed public files. No user data, no identifier, no
  file, project or query content: the request is the same for every install.
- **Pinned:** a fixed revision, and every file's SHA-256 and size are compiled into the app. A file
  only gets its final name once it matches; a mismatch is deleted and reported.
- **Trigger:** only when a feature needs embeddings and no copy is on disk: you turn on an interest
  (Settings → Sources), run a natural-language search, or press **Download** in Settings →
  Intelligence. Not when Ollama already serves embeddings or cloud embeddings are on. A copy already
  on disk (an older installer's bundled model, or a previous download) is used and never fetched
  again. Background work never downloads it.
- **Where:** the app cache (`<cache>/models/nomic-embed-text-v1.5-fp16`).
- **Without it:** embeddings come from Ollama if it runs; otherwise scoring and search use keyword
  matching (FTS). The dependency engine (Preemption, Brief facts, Signal's stack changes, MCP) does
  not use embeddings. No network either way.

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
- **No phoning home.** The only 4DA-operated endpoints are the user-initiated `4da.ai` license
  recovery (§2e) and, for subscription keys near expiry only, the key-only renewal (§2k). There is
  no background 4DA backend receiving data, and free or lifetime installs never contact `4da.ai`.
- **No raw-content transmission (by default).** Project files, source code, file contents, and git
  history never leave your machine. Not the text of your files, not your commit messages, not your
  diffs. **One opt-in exception:** if you turn on `allow_cloud_embeddings` (off by default), the
  chunks of local file text 4DA indexes as context (READMEs and other indexed files) are sent to
  `api.openai.com/v1/embeddings` to be embedded — see the table below.
  Enforced by `scripts/check-privacy-egress.cjs`, which fails the build if a raw-content column is
  read outside the modules that mine and store it — because until 2026-08-28 this bullet was wrong:
  the LLM reranker was including your five most recent commit messages in the context summary it
  sends to your configured cloud provider, and it is on by default. That is removed, and the gate
  exists so the claim is tested rather than asserted.
- **No local paths.** Prompts name your projects by short labels (`4da/relay`, `documents/navcal`),
  never by absolute path. Every prompt passes `src-tauri/src/privacy_egress.rs` at the LLM client
  boundary, which rewrites your home directory, configured context directories and detected project
  roots to those labels — whichever module built the prompt, including ones written later (a test
  fails if an LLM entry point skips it). Until 2026-09-10 the AI briefing named affected projects by
  absolute path, so a project under your home directory would have sent your OS username.
- **What IS derived from local content and does leave, when the relevant feature runs.** This list
  is exhaustive; if you find something not on it, that is a bug, not an omission.

  | Destination | What it receives | Why |
  |---|---|---|
  | `api.osv.dev` | package names + versions | vulnerability matching (§2c) |
  | `registry.npmjs.org` | your npm package names | release / version intelligence |
  | `pypi.org` | your Python package names | as above |
  | `crates.io` | your crate names | as above |
  | `static.crates.io` / `registry.npmjs.org` (§1e) | the crate / package name and the new public version of a graded release — never your installed version | the release card's "what changed" |
  | `api.github.com/repos/{owner}/{repo}/releases` (§2m) — **only with `github_release_notes = true`** (off by default) | the GitHub repository of a dependency whose package shipped no usable changelog for a graded release | the release card's "what changed" |
  | `registry.npmjs.org` / `crates.io` / `pypi.org` registry release check (§2c) | the name of each package with a version-confirmed advisory, and (npm) of each installed package that pins such a copy — name only | no fix target names a yanked or deprecated release; the parent release that fixes a nested advisory is named |
  | `registry.npmjs.org` full packument (MCP `dependency_check`, §2j) | the npm package name being checked — name only | release vetting |
  | `crates.io/api/v1/crates/{crate}/versions` (MCP `dependency_check`, §2j) | the crate name being checked — name only | release vetting |
  | `proxy.golang.org` | your **full Go module paths** | as above |
  | `api.github.com/advisories` | your ecosystem names | advisory matching |
  | `api.github.com/search` | your languages | repo discovery |
  | Reddit | the subreddits you follow | feed fetch (Lemmy and Mastodon use fixed instances and tags — nothing of yours is sent) |
  | Stack Overflow | your tags | question fetch |
  | your configured LLM provider | dependency **names** (and, for the AI brief, the **installed and fix versions** of vulnerable or outdated ones), short project labels, git-derived work **topics**, declared interests, and the titles of items you saved — nouns and version numbers, never your prose | rerank + briefing (BYOK, off unless you supply a key) |
  | `api.openai.com` `POST /v1/embeddings` (§2a) — **only with `allow_cloud_embeddings = true`** | the text being embedded, **including chunks of text from your local files** (the `context_chunks` 4DA indexes: READMEs and other indexed context), plus item text | cloud embeddings (off by default; never enabled by setting a key alone) |
  | your SMTP server (§2l) — only if you configure digest email | SMTP credentials + the digest (item titles, links, scores, the topics of yours they matched); **unencrypted if `use_tls = false`** | digest email |

  Go module paths are the sharpest edge here: a private module path such as
  `github.com/yourcompany/internal-service` discloses the organisation and repository name to
  Google's module proxy. If that matters to you, disable the Go source in Settings → Sources.
- **No device fingerprinting.** License validation sends the key only (§2d).
- **No cookies, no tracking pixels, no third-party scripts.** The frontend loads zero external
  resources.
- **No accounts required** to run the app. No social/share integrations.

---

## 5. Not 4DA's code: the Windows webview runtime

On Windows, 4DA's window is drawn by **Microsoft Edge WebView2**, the browser runtime built into
Windows and shared by other apps. It runs as `msedgewebview2.exe` processes and makes its own
connections to Microsoft, independently of anything 4DA's code does. Microsoft documents that
WebView2 collects required and optional diagnostic data, keeps itself updated, and sends crash dumps
to Microsoft ([Data and privacy in WebView2](https://learn.microsoft.com/microsoft-edge/webview2/concepts/data-privacy)).

- **Observed (2026-10-13, Windows 10, 4DA running):** `msedgewebview2.exe` held HTTPS connections to
  `2620:1ec:33:2::11` (the Microsoft Edge front-end range behind `*.msedge.net`) and
  `2603:1016:401:302a::2` (Azure). The system DNS cache at the time also held Edge update and
  component hosts (`msedge.api.cdp.microsoft.com`, `msedge.b.tlu.dl.delivery.mp.microsoft.com`) and
  `c.bing.com`; the DNS cache is machine-wide, so those may equally come from Microsoft Edge itself.
- **What 4DA sends through it:** nothing. The webview's CSP allows no network host (below), so 4DA's
  interface code cannot open a connection; every request 4DA makes comes from the Rust backend and
  is listed in §1–§3.
- **SmartScreen:** WebView2 turns Microsoft Defender SmartScreen on by default, and 4DA does not
  turn it off (it does not set WebView2's `IsReputationCheckingRequired`). Microsoft says
  SmartScreen sends information about the content a webview loads to Microsoft. 4DA's webview loads the
  app's own bundled pages, and the app opens article links through your default browser
  (`tauri-plugin-opener`).
- **Crash dumps:** if a WebView2 process crashes, Windows sends its minidump to Microsoft unless the
  app enables WebView2's custom crash reporting. 4DA does not enable it today.
- **How to limit it:** Microsoft's documented control for WebView2's *optional* diagnostic data is
  Windows **Settings → Privacy & security → Diagnostics & feedback → Diagnostic data**; required
  data is collected regardless of that setting. A firewall rule blocking `msedgewebview2.exe` would
  not affect 4DA's features (its interface needs no network), but it applies to every app that uses
  WebView2 and also blocks the runtime's own security updates.
- **macOS and Linux** use the system WebKit (WKWebView / WebKitGTK). Their background traffic has
  not been measured for this document.

---

## Content Security Policy (CSP)

The Tauri webview enforces a strict CSP (`src-tauri/tauri.conf.json` → `app.security.csp`). The
`connect-src` for the frontend is exactly:

```
connect-src 'self' ipc: http://ipc.localhost
```

That is the app's own bundled assets and Tauri's IPC bridge to the Rust backend — **no network
host at all**. The webview cannot reach the internet: any JavaScript `fetch`, XHR, WebSocket or
EventSource to any outside host (including `localhost` services such as Ollama) is blocked by the
webview engine. `img-src` is `'self' data:` and `script-src` is `'self'`, so no remote images or
scripts load either. **Every outbound connection in this document is made by the Rust backend**,
and the tables above are the complete inventory of it.

In development only (`pnpm run tauri dev`), `app.security.devCsp` applies instead: the same policy
with `ws://localhost:4444 http://localhost:4444` added to `connect-src` so Vite's hot-reload
can reach the local dev server. Release builds use `csp`, never `devCsp`.

---

## Deep link protocol

- **Scheme:** `fourda://` — inbound only, lets external apps open 4DA to a specific view. No outbound
  network calls.

---

Everything above is verifiable: read the cited source files, or run Wireshark while 4DA is open and
match every connection made by the `fourda` process against this manifest (on Windows, the
`msedgewebview2.exe` processes are Microsoft's runtime, §5).
