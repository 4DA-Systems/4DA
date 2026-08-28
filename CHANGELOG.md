# Changelog

All notable changes to 4DA will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.0.2] - 2026-08-29

Supersedes 1.0.1, which was tagged and built but never published: a syntax error in the
release workflow’s Windows verification step failed the job after the installers had already
been produced. No 1.0.1 artifacts were ever released. The contents below are that same body
of work, plus a gate (`scripts/check-workflow-shell-syntax.cjs`) that parses every inline
workflow script so a shell typo cannot reach a release tag again.

Curated highlights from a large body of work — this is a selection, not an exhaustive
commit log.

**The Windows installer in this release is not code-signed.** SSL.com eSigner rejects
the signing credentials (`the provided authorization grant is invalid, expired, revoked`),
and shipping a current build was judged better than continuing to serve an April build
with known privacy and security defects. macOS is signed and notarized as normal; Linux
is unaffected. Verify the SHA-256 against `SHASUMS256.txt` before running the Windows
installer.

### Changed

<!-- retired-ok: release history — names the demoted mechanism, does not make the retired claim -->
- **Behavioral learning demoted from scoring authority.** Degenerate calibration curves are now refused at both save and load, raw pre-curve scores are persisted alongside adjusted ones, and a uniform-pass circuit breaker halts a curve that stops discriminating. The engagement multiplier is reduced to the item-side community term, the learned gate axis can no longer confirm an item on its own, and both threshold auto-tuners are frozen. A migration purges poisoned calibration samples. Scoring pipeline version 18 → 19.
- Main navigation collapsed to four views — Brief, Preemption, Blind Spots, Signal.
- Blind Spots are ranked by consequence rather than unread volume.
- Briefing rebuilt around a News vs. Standing Conditions split, with a unified cold-boot path; security items no longer duplicate into the feed when already shown in Preemption.
- Embedding model swapped to bge-small-en-v1.5 (INT8) — smaller, faster, and better calibrated than the previous model.
- Briefing synthesis is BYOK-only; the default models are set so the brief narrates out of the box.
- Raw score percentages replaced with qualitative relevance labels.
- Crash reporting replaced with a local-first diagnostics export.
- Cloud embedding is opt-in; local-only is the default.
- Signal trial shortened from 45 days to 14.
- Website moved from Vercel to Cloudflare Pages.

### Added

- **Identity ledger** (schema 112) — every topic and technology ACE holds now carries a stored, inspectable record of *why* it is there, so "why does it think I care about this?" has an answer instead of a recomputed guess.
- **Materialised context matching** (schema 113) — the per-item context match is cached and maintained incrementally against a trigger-tracked corpus generation, with the grounding filter moved into the vector index partition key so the cached value is a true top-K rather than a truncation artifact. Measured on the live corpus: whole-corpus convergence went from 22 days to 3m59s, and a context change that moved 352 of 400 top-3 lists propagated across 53,752 items in 10 seconds instead of 560.
- **Version intelligence** now resolves the installed version at read time — it had never once run.
- A raw-content egress gate (`scripts/check-privacy-egress.cjs`) that fails the build if a raw-content column is read outside the modules that mine and store it.

- **Graph view for Signal** — an interactive map of how content relates, with story-first clustering, semantic satellites, colorblind-safe categories, an in-app detail panel, and remembered layout.
- **Upgrade Plan** — ranked, per-package dependency upgrade planning, surfaced in Preemption, persisted across restarts, and readable from the CLI via `4da plan [--json]`.
- **Ranked evidence chains** explaining why an item scored the way it did, with score reason chips on the feed.
- **Blind Spots "Assess with AI"** — batched LLM triage, auto-assessment when your dependency set changes, a stack coverage map, package watching, and persistent dismissals.
- **Platform-aware dependency relevance**, so advisories for build targets you do not ship are separated out.
- **Transitive and dev-dependency vulnerability surfacing** via a local OSV audit, including an offline ecosystem cache for air-gapped scanning.
- Curated scoring profiles and day-0 content for Java, C#, Ruby, PHP, and native mobile stacks.
- Security floor is never paywalled — OSV preemption stays available on the free tier.
- **Command palette** with deep-link picks, frecency ranking, and a result cache.
- **In-process ONNX embeddings**, bundled for a zero-download cold start.
- **Hybrid BM25 + vector search** with reciprocal rank fusion and context weighting, plus cross-encoder reranking.
- **Mastodon and Lemmy** source adapters.
- **Curated feed registry** of vetted sources with an in-Settings browser and per-feed health, plus access-strategy failover and adaptive throttling.
- **Headless engine** — keeps the database fresh without the GUI, runnable one-shot or as a daemon, with OS-scheduler background refresh and a Settings toggle.
- **Claude Desktop extension and Claude Code plugin manifest**; MCP server upgraded to SDK v2.
- **Light theme**, plus a grouped side-rail in Settings.
- **Instant 14-day Signal trial**, with graceful handling when no AI provider is configured.
- Onboarding: consent-gated local scanning, editable detected interests, a persistent language switcher, and one-click project scan.
- API keys are probed before saving, with an alert if a working key starts failing.
- **Signal Lifetime plan.**
- Opt-in email digest sent through your own SMTP server.
- Hindi and Italian, bringing runtime localization to 13 languages.

### Fixed

- **ACE was scoring against a contaminated picture of the stack.** Six defects, each independently inflating relevance: the dependency scope filter compared POSIX paths and so matched nothing on Windows and failed open; bare subterms could originate a dependency match and then compound; the scoring pipeline's own benchmark fixture was being admitted as context and teaching ACE what the user likes; four support paths outvoted seven primary manifests in the technology weight; unescaped underscores in topic names acted as SQL `LIKE` wildcards when counting corroboration; and the 7-day topic cliff measured file-edit recency rather than stack membership, so touching a file made a topic current and not touching one expired a core dependency. Phantom dependency matches: 1,320 to 0.
- **Git recency measured branch switches, not commits.** `compute_git_recency` read the mtime of `.git/HEAD`, which `git commit` never rewrites — only a branch switch does. Because that value scales dependency-match confidence, anyone working on a long-lived branch decayed toward the floor while committing daily, quietly suppressing the strongest grounding signal in the product. It now reads the reflog, and resolves the real git directory inside a linked worktree instead of scoring it as unreadable.
- **Snooze was broken on every card**, and reported the failure to the user in English regardless of locale. The action was dropped from the IPC payload before it reached the backend, so both feedback commands rejected it and the learning signal was lost entirely.
- The crash screen's primary recovery button was invisible in light theme (white on a light border, roughly 1.3:1).

- **Feed stability overhaul** (2026-08-23 adversarial audit, scoring pipeline v22): background runs now score only new and changed items instead of re-scoring the whole window every 30 minutes; sub-noise score wobble no longer touches durable state; batch-relative ranking factors (cross-encoder, diversity, source percentile) persist separately from the evidence score, so an item's score has a fixed point; and feed evictions without a categorical reason require two consecutive agreeing runs. The 6-hour cliff that crashed fresh community items to exactly 0.50 on schedule is gone, and interest synthesis is deterministic instead of a per-process lottery.
- **Recall repairs from the same audit**: releases of the developer's own dev-tools (vite, vitest, typescript) and family sub-crates (serde_derive-class) now ground properly; scoped npm packages and Go module paths survive advisory matching; Go standard-library and toolchain advisories are reachable; OSV content queries cover all nine ecosystems instead of two; years-old content is discounted by its published date; and migration stories about the developer's own stack are no longer suppressed as competitor noise.
- **Honest self-measurement**: the high-stakes recall monitor uses the corroborated dependency matcher (it previously reported a permanent 87.5% pseudo-miss-rate from phantom matches); the real-embedding simulation now runs in CI; benchmark quality floors raised to the newly measured levels (score-range 96.5%, security and cold-start 100%).
- Community engagement (favourites, boosts, scores, likes) is now actually ingested for Mastodon, Lemmy, and Bluesky — the "earn it back with engagement" scoring path had no data reaching it.
- Uncorroborated word-chain alerts can no longer carry critical urgency; job-seeker posts join the hiring cap.
- Numerous signal-feed precision failures, including registry release noise that dominated the feed and look-alike package matches.
- Grounding now requires name corroboration before a text match counts, removing phantom critical alerts; several gate count-inflation paths closed.
- CVSS extraction was reading a version-label digit as the severity score.
- A confirmed direct-dependency CVE is now treated as full evidence rather than partial.
- Curation verdicts had no epoch guard, so stale verdicts from a superseded scoring brain persisted.
- Scheduled and headless analysis now persist the feed verdict.
- Cold start no longer shows false assurance, vanity zero-counts, or fabricated accuracy figures on a profile with no feedback; taste-test interests are embedded so the first feed is not empty.
- Unchanged advisories collapse instead of re-alerting daily; ungrounded security alerts cap at advisory level; a misleading "Update" action was replaced with an honest "View advisory".
- OSV advisories were being dropped at intake; C#, PHP, and Dart stacks surfaced no vulnerabilities at all due to an ecosystem mapping gap.
- **The pre-migration backup was deleting itself, leaving migrations with no rollback point.**
- **Keyword search had drifted out of sync with the corpus.** The full-text index was maintained by hand, and the hand-maintenance was incomplete: items whose embedding failed were never indexed at all, edited items kept matching words they no longer contained, and nothing removed an item's index entries when it was deleted — so the first retention run would have made it worse. The index is now maintained by the database itself and is rebuilt once on upgrade.
- Backup pruning only ever collected one naming scheme, so manual pre-migration snapshots and corruption quarantine copies accumulated without limit — roughly 10 GB of them beside a 203 MB database.
- The headless engine did all of the writing and none of the database upkeep, so a machine running without the window open went a full day with no checkpoint, leaving a write-ahead log six times its intended ceiling.
- The re-embed repair pipeline was entirely non-functional.
- Corpus durability: atomic rebuild and collapse detection.
- Out-of-memory crashes in the cross-encoder reranker that killed background analysis.
- UTF-8 character-boundary panics across many modules, including an RSS parsing panic.
- Graph view crash under prototype freezing, an error boundary that locked navigation, and a canvas with no height that never rendered.
- Bounded webview recovery loop; fixed a development cold-boot crash loop.
- Signal tier silently dropped to Free when the settings schema drifted.
- API key loss, addressed with a layered keystore, lazy hydration with backoff, and platform-native credential backends; license recovery gained a multi-layer chain and a recovery banner.
- **Ghost tray icons** now removed on clean exit, with a sweep tool for strays.
- Windows console-window flashes from spawned child processes silenced; notifications now attribute to 4DA rather than the parent process.
- Modals scroll instead of clipping on short windows.

### Removed

- Momentum tab and the vanity metric panels that fed it.
- Evidence tab.
- Playbook tab — STREETS is published on the web instead.
- Tech Radar, superseded by stack intelligence.
- The Artificial Wisdom Engine and all its surfaces.
- The built-in local LLM sidecar (added and removed within this cycle; never shipped in a release). Local AI is Ollama or BYOK.
- "Hours saved" and similar vanity metrics.
- Crash reporting.
- Command Deck, the toolkit micro-tools, the Watches tab, and a large volume of dead components.

### Security

- **Git commit messages were being sent to a configured cloud LLM.** The reranker included the five most recent commit messages in the context summary it uploads, this is on by default once an API key is saved, and the `titles_only` privacy setting did not cover it. `NETWORK.md` stated that git history never leaves the machine; that is now true, and enforced by a gate rather than asserted.
- **The network-disclosure documentation was incomplete.** It claimed OSV package names were the only locally-derived data sent anywhere. npm, PyPI, crates.io, the Go module proxy and the GitHub advisory API all receive package names, and GitHub search, Reddit and Stack Overflow receive languages, subreddits and tags. `NETWORK.md` now carries an exhaustive table, and names the sharpest edge explicitly: a private Go module path discloses the organisation and repository name to Google's module proxy.
- **A licence fast path accepted any non-empty string.** A hand-edited settings file could grant the paid tier permanently, with no signature, no network call and no expiry, because the check short-circuited before the validation cache it claimed to rely on. A non-self-signed key is now usable only when the cache vouches for that exact key by hash, tier and freshness; the same hole in the backup-file recovery path is closed the same way.
- **The SSRF guard could be switched off by a value the frontend controls.** Three call sites skipped internal-address validation whenever the provider was named `ollama`, which disabled the check for any base URL, cloud metadata endpoints included. Validation is now keyed on the host, which also fixes the opposite error: a local LM Studio, llama.cpp or Jan server is exactly as safe as Ollama and was being blocked for not being called Ollama.
- **The repository secret scanner missed every key format the app accepts.** Its pattern stopped at the first hyphen, so current Anthropic and OpenAI project keys were invisible while the audit reported no findings across 1,958 files. Patterns are broadened and moved to a shared, regression-tested module, with a placeholder filter keyed on the matched text rather than the file path — a path exemption would excuse a real key dropped into a test file.
- Atomic-save staging files are now ignored by git. The final settings file was; the temporary file it is written through was not, and it holds the API key and licence key in plaintext for the width of a rename.

- Cleared advisories across the Rust and npm dependency trees, including RUSTSEC-2026-0037, RUSTSEC-2026-0141, RUSTSEC-2026-0187, RUSTSEC-2026-0193/0194/0195, and vulnerable `ws`, `undici`, `esbuild`, `hono`, `lopdf`, `quinn-proto`, and `lettre` versions. Nightly auditing extended to sub-lockfiles.
- **SQL injection in natural language search**, and unparameterized standing queries.
- **Cross-site scripting via an unvalidated URL scheme.**
- Shell-injection and SQL formatting hardening on Linux; macOS hardened runtime.
- SSRF and keychain posture gaps closed.
- Prompt-injection hardening for search synthesis, with an adversarial content filter.
- Prototype freezing for Array, Map, and Set; no-referrer policy.
- Credential-safe project indexing.
- Input validation on IPC command boundaries; webview CSP and plugin isolation.
- Secret scanning across many patterns with layered defense; a private-asset leak gate whose pre-push hook had been scanning the wrong commit range.
- Rate limiting on LLM API usage, enforced by both token count and cost.
- Privacy claims rescoped to match actual behavior, with explicit consent for cloud LLM use and zero-retention defaults where the provider supports it.

## [1.0.0] - 2026-03-08

### Highlights

4DA (4 Dimensional Autonomy) is a privacy-first desktop app that surfaces developer-relevant content from the internet — scored against your actual codebase, running on your machine. No telemetry, no 4DA cloud, no user accounts. First useful results in under 3 minutes after a quick onboarding (pick your stack, add an API key or use Ollama).

### Features

**Intelligence & Scoring**
- 5-axis relevance scoring (PASIFA) evaluates content against your actual codebase and tech stack
- 20 built-in sources: Hacker News, Reddit, arXiv, GitHub, Product Hunt, RSS, YouTube, Twitter/X, DevTo, Lobsters, Stack Overflow, Bluesky, Hugging Face, Papers with Code, CVE, OSV, npm Registry, PyPI, crates.io, and Go Modules
- Taste Test calibration: 15-card interactive session tunes scoring to your preferences
- Developer DNA: auto-detected tech stack profiles your development identity across graduated tiers
- Temporal clustering groups related articles across sources and time
- Novelty detection filters seen-before content and boosts new releases
- Content quality analysis evaluates title quality, content depth, and source authority

**AI Briefings (Free)**
- AI-generated daily intelligence briefings summarize your top signals
- AI-generated weekly digest of the most important developments across all sources

**Intelligence Layer (Signal)**
- Score Autopsy explains exactly why each article received its score
- Natural language queries against your content stream
- Signal cards surface critical and high-priority items above the briefing
- Persistent briefing state with auto-refresh and freshness indicators

**STREETS Playbook (Free)**
- All 7 STREETS modules free on the open web at 4da.ai/streets
- Interactive lessons, templates, and sovereign developer profile

**Developer Tools**
- Essential Toolkit with 7 micro-tools for daily development
- Command Deck with git operations and project management
- Decision Journal for tracking and reviewing technical decisions
- Knowledge gap detection identifies blind spots in your project dependencies
- MCP integration: 14 tools for Claude Code, Cursor, and Copilot

**Privacy & Security**
- Local-first — zero telemetry, zero data collection, no 4DA server
- BYOK (Bring Your Own Key) — API keys never leave your machine
- Local AI via Ollama; optional BYOK cloud models send only what you analyze, to the provider you choose
- Restrictive CSP blocks unauthorized network requests
- Keyword-only mode available without any AI provider

**Localization**
- Full i18n support with built-in translation editor
- Auto-detected locale with manual override
- Translation override system for custom terminology

### Technical
- Built with Tauri 2.0 (Rust) + React 19 + TypeScript + SQLite
- sqlite-vec for local vector search and semantic matching
- 2,435 tests (1,618 Rust + 817 frontend)
- 11MB installer (Windows NSIS)
- Auto-updater with Minisign signature verification
- Standalone CLI binary (`4da`) for terminal workflows
- FSL-1.1-Apache-2.0 license (converts to Apache 2.0 after 3 years)

### Known Limitations
- Ollama required for local AI features (optional — keyword-only mode available without it)
- First analysis may take 60-90 seconds depending on number of configured sources
- Twitter/X source requires API key
