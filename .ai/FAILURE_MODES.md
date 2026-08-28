# 4DA — Known Failure Modes

Living document of fragile areas, previous regressions, and "never again" lessons. If you hit one of these, add your case here before fixing it — so the next person hits the fix and not the bug.

---

## Build & toolchain

### Vite-dep-update-while-running crash
**Symptom.** `fourda.exe` is running. You update a Vite-adjacent dep (`vite`, `@tailwindcss/vite`, `@vitejs/plugin-react`, etc.) via `pnpm install`. Next build or route load crashes with `Cannot find module vite@X.X.X_@emnapi+core...`.

**Root cause.** The running process holds the old `node_modules/.vite/deps` paths in memory. `pnpm` replaces the deps but the runtime's module resolution is stale.

**Guards in place.**
- `pnpm postinstall` clears `node_modules/.vite/deps` on every install.
- `pnpm run validate:vite-smoke` does a cold-start and verifies 13 critical routes.
- `pnpm run validate` includes the smoke test.

**If it happens.** `taskkill /F /IM fourda.exe && pnpm install --frozen-lockfile`.

---

### Windows binary CRLF corruption (the "16-bit application" class)
**Symptom.** A tracked binary (installer, Tauri icon, compiled exe) shipped in a release fails to launch on Windows with *"Unsupported 16-Bit Application"* or just silently refuses. Or a fresh clone on a Windows box shows corrupt PNGs.

**Root cause.** Contributor on Windows with default `core.autocrlf=true`. Without an explicit `.gitattributes` marking binaries as `binary`, git rewrites LF→CRLF on checkout inside the binary, mangling the PE header.

**Guards in place (2026-04-19).**
- `.gitattributes` at repo root with explicit `binary` markers for every known binary extension.
- CI installer smoke test on v1.1+ roadmap.

**If it happens.** Verify `.gitattributes` covers the extension. Re-clone with `git clone -c core.autocrlf=false`. For a shipped release: re-tag from a clean checkout.

---

### `cargo test --lib` suddenly failing to compile
**Symptom.** `cargo test` fails with `E0425: cannot find super::SOMETHING` or similar private-interface errors. Build was fine yesterday.

**Root cause (seen 2026-04-18).** Test references a `const` / `fn` declared inside a function body (function-local scope) rather than module scope. `super::FOO` in a `#[cfg(test)] mod tests` block only reaches module-level items.

**Guards in place.** CI should run `cargo test --lib --no-run` on every PR (added 2026-04-19 Wave 5).

**If it happens.** Lift the referenced item to module scope and mark `pub(crate)` if needed.

---

### Pre-commit hook blocks legitimate content
**Symptom.** Pre-commit fails with `Secrets/sensitive data detected` on a test fixture or legal-page ABN disclosure.

**Root cause.** The secret scanner in `.husky/pre-commit` uses aggressive patterns. It has targeted exclusions for (a) clearly-fake test strings, (b) `site/src/contact.njk` for phone numbers, (c) legal pages for ABN/TFN — but the exclusion list needs to be maintained as surfaces grow.

**If it happens on a legitimate value.**
1. For test fixtures: split the string literal to prevent pattern detection (`"sk-" + "rest-of-fake-key"`).
2. For legal disclosures: add the file path to the ABN exclusion case statement in `.husky/pre-commit` (the current allowlist covers `docs/legal/*`, `LICENSE`, `NOTICE`, `CLA.md`, `TRADEMARKS.md`, `SECURITY.md`, `README.md`, `docs/launch/*`, `docs/philosophy/*`, and `site/src/*`).

**Never.** Do not use `git commit --no-verify` to route around this gate.

---

### `tauri dev` watcher dies on the relink lock and does NOT retry
**Symptom.** You merge/pull Rust changes, the watcher prints `File … changed. Rebuilding application…`, then:
```
Caused by: Access is denied. (os error 5)
 ELIFECYCLE  Command failed with exit code 101.
```
The whole `tauri dev` process exits — taking **vite (4444) and Victauri (7373) down with it** — while the OLD `fourda.exe` keeps running. Nothing retries, so the app silently stays on pre-merge code and any "activation" you believe happened did not.

**Root cause.** The running `fourda.exe` holds `src-tauri/target/debug/fourda.exe`; the linker cannot overwrite it. Sometimes the watcher kills the app first and succeeds — sometimes it does not, and then it dies. Observed both outcomes on 2026-07-26.

**Fix.** Just restart the dev loop: `pnpm run tauri dev`. Its `dev` script runs `scripts/kill-fourda.cjs` first, so it force-kills the stale app itself, restarts vite, and relinks with cached artifacts (~2–3 min). Force-killing skips `Drop`, so sweep any ghost tray icons afterwards with `pnpm run flush-tray-ghosts`.

**Always verify activation by artifact, never by assumption.** Check `ls -la src-tauri/target/debug/fourda.exe` (mtime AND size) against the merge time before claiming a fix is live.

---

## Data & database

### `cargo test` cannot run while dev server is running
**Symptom.** `cargo test` reports a file-lock error or a hanging test process.

**Root cause.** `fourda.exe` (dev mode) holds the SQLite DB lock. `cargo test` spins up its own DB instance but collides on certain resources.

**Workaround.** Use `cargo test --lib` (no integration tests) while the dev server is running. For full test runs, stop the dev server first.

---

### Malformed user DB causes startup panic
**Symptom.** 4DA startup panics at migration time with a cryptic rusqlite error.

**Root cause.** `src-tauri/src/db/migrations.rs` has historically used many `.unwrap()` calls. A corrupt or unexpectedly old DB file hits one of those and the whole app dies.

**Guards partial.** Some migrations now use `ResultExt::context()` (`src-tauri/src/error.rs`). Systematic migration to `?` + context is scheduled.

**If it happens.** Check `%APPDATA%\com.4da.app\data\4da.db`. If the user can afford to lose local state, rename it to `.broken` and let 4DA re-create. Otherwise open with the CLI `sqlite3` and inspect the `schema_version` table.

---

### Plaintext team-crypto despite `_enc` schema naming (resolved 2026-04-19)
**Symptom.** The `team_crypto` table has columns `our_private_key_enc` and `team_symmetric_key_enc`. Pre-Wave-16 builds wrote the bytes as plaintext despite the suffix.

**Status: RESOLVED.** Both keys now live in the OS keychain (key names `team_privkey__<team_id>` and `team_symkey__<team_id>`). The DB columns remain as a fallback for hosts without a reliable keychain and are blanked to a zero-length BLOB on successful keychain round-trip. All five touchpoints (two INSERT paths in `team_sync_commands.rs`; three READ paths in `app_setup.rs` and `team_sync_scheduler.rs`) route through the `team_sync_crypto::persist_*` / `read_team_*` helpers. The helpers use write-then-read-back verification so a keyring that lies about the write (observed on some Windows Credential Manager setups) can never cause silent loss. Old rows lazy-migrate on the next read. See Wave 16 commit.

---

### sqlite-vec `vec0` tables reject `INSERT OR REPLACE` (cost: a 3-month silent outage)
**Symptom.** A write to `source_vec` / `context_vec` / `topic_vec` fails with `UNIQUE constraint failed on <table> primary key` — but only for rowids that ALREADY exist. Inserts of new rowids succeed, so ingest looks healthy while every *update* path silently fails.

**Root cause.** `vec0` virtual tables do not honour `OR REPLACE` conflict resolution. Discovered 2026-07-26: `upgrade_pending_to_complete` used `INSERT OR REPLACE INTO source_vec`, and because `upsert_source_item` gives every item a `source_vec` row, **every** re-embed failed. 624 consecutive retry cycles, 0 upgrades, 887 items stranded `embedding_status='pending'` since April holding vectors of superseded content. Fixed in #377.

**The correct idiom** (already used everywhere else — `upsert_source_item` is the tell: it applies `INSERT OR REPLACE` to the FTS5 table but `UPDATE` to `source_vec` in the SAME transaction):
```rust
let updated = tx.execute("UPDATE source_vec SET embedding = ?1 WHERE rowid = ?2", …)?;
if updated == 0 {
    tx.execute("INSERT INTO source_vec (rowid, embedding) VALUES (?1, ?2)", …)?;
}
```
`INSERT OR REPLACE` is only safe against a vec0 table when the row is *provably absent* (e.g. guarded by `NOT EXISTS`, as in `ace/topic_embeddings.rs`'s backfill).

**Regression guard.** `db::sources::tests::upgrade_pending_to_complete_works_when_vec_row_already_exists` — verified to fail pre-fix with the exact constraint error.

---

### Silent repair loops (why the above survived 90 days)
**Symptom.** A background repair/retry loop makes zero progress indefinitely and logs nothing that distinguishes it from having no work to do.

**Root cause — two concealment patterns.** (1) The write result is discarded: `let _ = conn.execute(…)` or `…​.is_ok()`. This repo has been bitten at least three times (see the comments at `app_setup.rs:378` and `:1146`, plus #377). (2) The outcome log is gated on success: `if upgraded > 0 { info!(…) }`, so total failure prints *nothing* and reads as "idle".

**Rule.** **Any loop whose job is to REPAIR state must report the ZERO case, not just the success case** — with a failure count and the first error. See the re-embed retry in `analysis_status.rs` for the shape (`upgraded` / `failed` / `fallback`).

**Related.** A diagnostic that cannot be wrong is not a diagnostic: the startup "Embedding model" line hardcoded a model-name string literal regardless of the real provider, which is how an embedding-layer discrepancy stayed invisible (#378).

---

### Materialized derived state needs BOTH an epoch stamp and a pass that converges it

**Symptom.** A stored value that was computed by some version of the logic keeps being served long after that logic changed. The corpus reports "100% current" — because the thing being *measured* is current — while a second derived value sitting next to it is months stale and nothing is even looking at it.

**Root cause.** Each derived value goes stale on its OWN clock. `relevance_score` got a `scored_pipeline_version` stamp, a drain, and scoped epochs (#372–#374). `feed_relevant` — the verdict that decides what the user actually SEES — got a timestamp and nothing else. Once the drain finished, every item was score-current and therefore **invisible to the drain**, while 399 of 430 curated items still held a verdict a superseded pipeline had decided (#380, measured live 2026-07-26). 219 of them the current pipeline outright rejected; 157 were the exact look-alike class v18 had banned three weeks earlier (#375).

**Why it could not self-heal.** The write path only re-judges what its selection window returns (`get_items_tiered`), so anything that ages out of that window is frozen permanently. The tell in the data: every curated verdict shares one timestamp floor — the last corpus-wide pass. A stamp alone would not have fixed this; the converging pass is the other half.

**Rules.**
- If you materialize a value derived from versioned logic, stamp it with that version **and** ship the pass that re-derives stale ones. One without the other is a slow leak.
- Put the pass where every path reaches it. #380's plan specified a call site reachable only from an operator-run drain command, which *also* returned early when no stale scores remained — i.e. it would never have run in the exact state that motivated it.
- Prefer **demote-only** for a per-item pass. Removing something the current logic rejects needs no batch context; *promoting* does (dedup / diversity / rerank), so it isn't a per-item decision.
- Do **not** make consumers filter on the stamp. Excluding stale rows empties the surface after every bump (94% of live graph nodes here) until the pass catches up. The pass converging IS the fix.

---

### Provenance must be read from the producer's flag, never inferred from a value signature

**Symptom.** A cleanup/reconciliation pass is about to delete or demote records, and you need to know which ones a *different* code path created so you can spare them. The obvious move — recognise them by a distinctive value they carry — silently misses a whole class.

**Root cause (#380).** Two paths write `feed_relevant = true` without the scorer agreeing. The concept-graph injection builds the item with a fixed `top_score: 0.45`, so a "score == 0.45" probe finds it. But `compute_serendipity_candidates` takes items the scorer **rejected** and flips the flag while keeping their **original** score — invisible to that probe, and sitting in exactly the band the pass was going to demote. The planning doc concluded "0 such items exist, a naive pass is safe today" on the strength of the 0.45 probe. That was a property of the probe, not of the corpus: the pass would have deleted live anti-bubble picks.

**What it should have been (and now is).** `SourceRelevance::serendipity` — a boolean the producers already set at **both** sites — persisted as `feed_verdict_source`. Exact, not inferred.

**Rules.** Before writing a heuristic to recognise records by shape, grep for a flag the producer already sets. When a claim rests on "we searched and found none", state the search's *coverage*, not just its result — enumerate the writers (grep every construction site of the type) rather than probing for one signature. Provenance that was never persisted is unrecoverable: `source_items` had no column recording it, so pre-Phase-101 rows are permanently unclassifiable and the pass accepts a bounded, self-healing one-time cost for them.

---

### A partial index that isn't COVERING still pays one row lookup per row

**Symptom.** You add an index for a probe that runs every cycle, the query plan says it's using an index, and the probe is still slow on a cold process.

**Root cause.** SQLite used the index to *find* the rows, then fetched each row from the table to evaluate the rest of the predicate. On the Phase-101 verdict probe, indexing `feed_verdict_version` alone left the planner on the older `idx_si_feed_relevant` and did 426 random row lookups: **902 ms cold, on every cycle, forever.** Adding the second predicate column (`feed_verdict_source`) made the index cover the whole query — `SCAN … USING COVERING INDEX` — at **3.7 ms**. This is the same cost class Phase 100 was created to eliminate (a 700–800 ms per-cycle probe), re-introduced by an index that looked correct.

**Rules.** For any probe on a hot path, put **every** column the predicate touches into the index, and confirm with `EXPLAIN QUERY PLAN` that the plan says `COVERING INDEX` — "uses an index" is not the bar. Verify on a copy of the real corpus, not a test DB: the cost is in the row fetches, which a small table never shows. Assert the index's columns in a test (`pragma_index_info`) so a later edit can't quietly drop the covering property. Note that wrapping a column in `COALESCE(...)` makes the predicate non-sargable — the index can still be *scanned* (cheap when partial), but it can no longer be *seeked*.

---

## IPC & command surface

### Ghost command silent failure
**Symptom.** A frontend `invoke('xyz')` call hangs or errors cryptically. No obvious backend log.

**Root cause.** The Rust `#[tauri::command]` handler exists but was not added to the `invoke_handler!` registration list in `lib.rs`. OR the handler name on the frontend does not match the Rust fn name (case, underscores).

**Guards in place.**
- `pnpm run validate:commands` (`scripts/validate-commands.cjs`) cross-references every `invoke('...')` call against registered handlers.
- `pnpm run validate:wiring` tightens this further.

**If it happens.** Run `pnpm run validate:commands`. The mismatch will be reported with file:line.

---

### `MutexGuard<SourceRegistry>` not Send across await
**Symptom.** Compile error `future cannot be sent between threads safely` on a command touching `SourceRegistry`.

**Root cause.** `MutexGuard` is not `Send`. Holding it across an `.await` boundary is a type error.

**Fix.** Bracket the lock scope with `{}` to drop the guard before the await. Example pattern is in `src-tauri/src/state.rs`.

---

## Scoring & pipeline

### Generic package-name false positives in security alerts
**Symptom.** Preemption feed surfaces an "alert" for every article mentioning "buffer" because the Node.js `buffer` package is in your deps, even though the article is about buffer overflows (the concept).

**Root cause.** SQL `LIKE '%crypto%'` matches cryptocurrency articles. Package names that collide with common English security terminology produce 40-80 false positives per month.

**Guards in place.** `SUPPRESSED_GENERIC_NAMES` at module scope in `src-tauri/src/preemption.rs` (lifted 2026-04-19 Wave 1). Currently blocks 47 generic names from the SECURITY ALERT path only — they still surface in Blind Spots and Knowledge Gaps (different matching strategy).

**Proper fix (v1.1).** Ecosystem-aware CVE cross-ref (match on `{ecosystem, package_name}` tuple, not just name).

---

## Intelligence & onboarding honesty

### Proxy-derived state claims (the "AI provider configured" lie)
**Symptom.** A first-run user with **no provider** sees the app claim a capability it doesn't have: "AI provider configured", `has_llm:true` / `llm_tier:"cloud"`, fabricated tech/interest counts, and background LLM jobs (briefings, digests, translation, summaries) that fire against a non-provider and fail silently. The inverse also occurs — a user who selected the **built-in** local model is told the system is *not* configured (false-negative), or a builtin-generated result is not labelled "local".

**Root cause.** A boolean/string asserting a capability is computed from a **proxy** that is true even when the real state is false:
- `!api_key.is_empty()` / `has_api_key` **without** confirming a real selected provider — a stale keychain/ENV key with `provider == "none"` flips it true.
- a single-provider OR-shortcut (`provider == "ollama" || !api_key.is_empty()`) that silently **drops `builtin`**.
- `embeddingMode !== 'keyword-only'` used to claim an **LLM** is configured — built-in fastembed embeddings are *always* on, conflating semantic search with an LLM provider.
- a user-facing **count read from the optimistic frontend store** instead of the authoritative backend command.

**The cure — one provider-driven source of truth.** `content_personalization::context::compute_has_llm(provider, api_key)` (`src-tauri/src/content_personalization/context.rs`, `pub(crate)`) is the single gate: `none`/`""` → false, `ollama` → true, cloud → needs a key. Every gate that decides whether to attempt an LLM call routes through it; the frontend mirrors the same provider-driven logic (`src/components/Onboarding.tsx`).

**Update (2026-06-03) — the built-in local LLM was removed.** The bundled llama-server "Built-in" provider (sidecar + model catalog) was retired (UI removal `25f0d945`; backend removal Phase 2): it duplicated Ollama and couldn't ship cleanly. `compute_has_llm` no longer has a `builtin` arm (Ollama is the only keyless local provider), and a launch migration resets any persisted `provider == "builtin"` → `"none"` (`settings/manager_init.rs`) so a pre-removal profile degrades honestly to BYOK/Ollama rather than pointing at a deleted sidecar. Built-in *embeddings* (fastembed) are unaffected — they were always on and are not an LLM provider.

**Guards in place (2026-06-02).**
- `scripts/check-llm-gate-honesty.cjs` (pre-commit) — fails the commit on a new `api_key.is_empty() || provider=="ollama"|"builtin"` / `has_api_key || provider==='ollama'` / `!matches!(provider,…) && api_key.is_empty()` construct. Escape hatch: `llm-gate-ok: <reason>`.
- `scripts/check-vanity-metrics.cjs` (pre-commit) — doctrine rule 3, fails on banned counters rendered as a number/`{{count}}`. Escape hatch: `vanity-ok: <reason>`.
- Both gates are pinned by `scripts/*.test.cjs` (`pnpm run test:scripts`) which also enumerate the gates' **known blind spots** (variable-indirection, alternate key-presence spellings, renamed flags, tag-separated counters, semantic vanity). These syntactic gates are not proofs — capability-claim correctness is still a PR-review responsibility.
- `compute_has_llm` unit tests in `context.rs` (incl. a guard that `builtin` no longer reads as a keyless provider); onboarding persistence/provider-selection tests in `src/components/onboarding/quick-setup-utils.test.ts` + `use-quick-setup.test.ts`.

**Prevention rule (enforce in review).** Never derive a capability claim (`has_llm`, `enabled`, "configured", "ready", "available", "local") from `!api_key.is_empty()` or a single-provider OR-shortcut. Capability is a property of the **selected provider**, not of key presence or embedding mode. Any new construct missing an explicit `"none"`/`""` branch is a regression.

**Full antibody (this machine, gitignored ops memory).** `.claude/wisdom/antibodies/2026-06-02-proxy-derived-state.md` — the per-site lurking-scan table and verified-clean list.

---

## Release & CI

### Desktop updater channel hijacked by non-desktop GitHub releases
**Symptom.** Installed desktop clients stay on an old binary even though newer code exists. The app may open a newer local database with an older schema reader and report `Database schema version N is newer than this version of 4DA supports (max M)`.

**Root cause.** The updater endpoint used GitHub's global `/releases/latest/download/latest.json`. Any non-desktop release, especially `mcp-v*`, can become GitHub's "latest" release. If that release lacks `latest.json`, the frontend updater check fails and older clients remain stale. If app semver is not bumped, even a generated manifest can be invisible to the updater.

**Guards in place (2026-07-29).**
- `tauri.conf.json` points at `releases/download/desktop-latest/latest.json`, a desktop-only updater manifest pointer.
- `.github/workflows/release.yml` requires `latest.json` before publishing and uploads it to the `desktop-latest` release.
- `.github/workflows/build-mcpb-extensions.yml` marks MCP bundle releases as prereleases so future MCP tags cannot become GitHub's global latest.
- `scripts/check-release-channel.cjs` runs in pre-commit, CI, and `pnpm run validate` to enforce endpoint, manifest, prerelease, and app-version consistency.

**If it happens.** Check the installed exe path and mtime, then inspect the configured updater URL. `https://github.com/4DA-Systems/4DA/releases/latest` is not a valid desktop updater authority; only the `desktop-latest` manifest pointer is.

### SSL.com CodeSignTool download can return a landing HTML page instead of a ZIP
**Symptom.** Windows release build fails at signing, OR worse, ships an unsigned exe that SmartScreen flags. `Expand-Archive` succeeds but extracts nothing usable.

**Root cause.** The `Invoke-WebRequest` hits `ssl.com/download/codesigntool-for-windows/` which can redirect to a landing page rather than the versioned ZIP.

**Guards in place (2026-04-19 Wave 5, updated 2026-07-30).** Release workflow computes SHA-256 of the downloaded zip and hard-fails on mismatch. SHA is pinned (`317d429b...`). Post-build step verifies Authenticode signature on every .exe/.msi before upload. EV cert issued 2026-05-12, eSigner active, all GitHub secrets set.

---

## Document hygiene

### Planning doc accidentally tracked at repo root
**Symptom.** A file named `PLAN-XYZ.md` or `AUDIT-foo.md` accidentally shows up in `git status` and `git diff` tries to commit it.

**Guards in place.** `scripts/check-doc-location.cjs` runs in pre-commit. Rejects root-level files matching internal planning patterns. `.gitignore` at root covers the planning-doc glob.

**If it happens.** Don't `--no-verify`. Move the doc to `.claude/plans/` (gitignored) OR add it explicitly to `scripts/doc-allowlist.json` with rationale.

---

## Learned state & scoring

### Fitted artifact outlives the data it was fit on
**Symptom.** A "learned" transform (calibration curve, cached embedding, tuned threshold) behaves absurdly while all its training tables look empty/healthy. 2026-08-11 incident: after the 07-31 DB reset, `data/calibrations/{hash}/judge.json` (fit 06-19 from 50 mislabeled samples, every bucket → 1.0) kept loading — the model's honest 1/5 judgments were remapped to 5/5, the reconciler added +0.15 to 48 items/cycle, and every remapped 1.0 was re-persisted as a "raw" sample for the next fit.

**Root cause class.** Learned state persisted OUTSIDE the DB (files, kv, caches) with identity keyed on model/prompt but NOT on the corpus/data epoch it was fit from. A reset wipes the evidence but not the conclusion. This is the failure mode `INVARIANTS.md` INV-041 exists to prevent — `data/calibrations/` is one of its four recorded out-of-DB exceptions, and the incident is what turned INV-041 from "everything lives in SQLite" (never true) into "anything outside SQLite must be bound to the data it was derived from". Same class one layer down: `embedding_cache` stored the model name but never filtered on it (fixed v19 — model now in the lookup key); the tuned `relevance_threshold` in kv_store was re-installed on every ACE warmup (removed v19).

**Guards in place (v19).** Degenerate curves refused at save AND load (`CalibrationCurve::degeneracy_reason`); rerank uniformity circuit-breaker (all-identical scores → pass discarded); calibration samples persist the RAW pre-transform score; Phase 103 purged the 3,028 poisoned samples; embedding cache lookups require model match. Residual: fitted artifacts are still not stamped with a corpus epoch — AD-029 re-enable criterion (4).

**If it happens.** Quarantine (move, don't delete) the artifact directory; no-curve/no-cache is a documented-safe pass-through everywhere. Then live-verify the next cycle's telemetry (`agreed/skeptical/enthusiastic` in the rerank log line).

### Post-pipeline score writers bypass categorical caps
**Symptom.** Items the pipeline capped (commodity ceiling, UGC caps) rank at the top of the feed anyway; score signatures cluster at writer-specific values (e.g. three items tied at 0.948).

**Root cause class.** `score_item`'s caps are applied INSIDE the pipeline, but cross-encoder rerank (0.4/0.6 blend), dedup cluster boost (+0.09), source-tier percentile normalization, and the LLM reconciler (±0.15, clamped at 1.0 not the 0.92 knee) all overwrite `top_score` AFTER it. v18 made the VERDICT categorical; the SCORE still ranked the feed.

**Guards in place (v19).** `ScoreBreakdown::score_ceiling` set at cap time and re-asserted in `finalize_scores` — the one pass that already runs after every writer (`scoring/analyzer.rs`). Frontend score-sort honors the ceiling too (`use-result-filters.ts`). Any NEW post-pipeline score writer must run before `finalize_scores` or re-assert ceilings itself.

---

## When to add to this file

- You hit a bug that took more than an hour to track down.
- You encounter a "wait why is this like this" moment and the answer is "past incident."
- A regression re-appears in a PR — add the guard AND the failure mode entry.
- An adversarial audit catches a class you missed — document the class, not just the instance.

Keep entries short. Link to code by `file:line`. If the fix requires more than two paragraphs, link to an ADR in `.claude/plans/` or a strategy doc.


---

## FM: A maintenance loop that reports attempts instead of conversions

**Observed:** 2026-08-27. The stale-version drain logged `stale=500` every eleven
minutes for days and looked healthy. It was converting **five** of those 500. The
other 495 were scored at full price and then deleted from the results vector by
the batch layer (cross-source dedup, fuzzy title, topic dedup, temporal
clustering removed 831 of 1,458 per cycle) *before* `persist_cycle_results`
wrote the version stamp — so they kept an older stamp, were re-selected by the
next cycle's deterministic `ORDER BY`, and were re-scored and discarded again.
Net drain 88 items/hour against a 46,997-item backlog: **22 days**, with 1.1% of
the compute doing useful work. The same machine cleared it in **22m07s** the
moment the one-shot path was run by hand.

**Class:** the same one as the 90-day re-embed outage already in this file — a
repair loop that only reports its own activity is indistinguishable from one that
works. `stale=500` was true every time and meant nothing.

**Structural fixes shipped (AD-033):**
- `persist_cycle_results` stamps and scores every item the scorer EVALUATED, not
  every item that survived the batch layer (`analysis_cycle::EvaluatedItem`).
  Rank and verdict stay on survivors — they are batch-relative by doctrine.
- The drain no longer merges into the display batch at all; it runs beside the
  cycle (`drain_stale_scores_budgeted`), where nothing can delete its work
  before it is written.
- Drain logs now carry `converted`, measured as a before/after count, and warn
  explicitly when `rescored > 0 && converted == 0`.
- A backlog past `DRAIN_TO_COMPLETION_THRESHOLD` triggers a run-to-completion
  drain automatically, on every path — the one-shot drain had existed since
  PIPELINE_VERSION 7 and no code anywhere called it.

**Detection:** `get_scoring_coverage` reports the pipeline-version histogram and
the context-cache coverage. `4da::backfill` warns when more than 5% of the corpus
is on a superseded epoch, naming the consequence: every surface except Blind
Spots ranks items judged by two pipeline versions against each other.

**The generalisable rule:** when you add a loop that is supposed to shrink
something, log the SIZE OF THE THING, before and after. Never log the size of the
attempt.
