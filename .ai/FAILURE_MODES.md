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

**The cure — one provider-driven source of truth.** `content_personalization::context::compute_has_llm(provider, api_key)` (`src-tauri/src/content_personalization/context.rs`, `pub(crate)`) is the single gate: `none`/`""` → false, `ollama`/`builtin` → true, cloud → needs a key. Every gate that decides whether to attempt an LLM call routes through it; the frontend mirrors the same provider-driven logic (`src/components/Onboarding.tsx`). Onboarding never persists a provider it can't run — picking Built-in with no downloaded model persists honest `none` (`src/components/onboarding/quick-setup-utils.ts` `saveBuiltinProvider`).

**Guards in place (2026-06-02).**
- `scripts/check-llm-gate-honesty.cjs` (pre-commit) — fails the commit on a new `api_key.is_empty() || provider=="ollama"|"builtin"` / `has_api_key || provider==='ollama'` / `!matches!(provider,…) && api_key.is_empty()` construct. Escape hatch: `llm-gate-ok: <reason>`.
- `scripts/check-vanity-metrics.cjs` (pre-commit) — doctrine rule 3, fails on banned counters rendered as a number/`{{count}}`. Escape hatch: `vanity-ok: <reason>`.
- Both gates are pinned by `scripts/*.test.cjs` (`pnpm run test:scripts`) which also enumerate the gates' **known blind spots** (variable-indirection, alternate key-presence spellings, renamed flags, tag-separated counters, semantic vanity). These syntactic gates are not proofs — capability-claim correctness is still a PR-review responsibility.
- `compute_has_llm` unit test in `context.rs`; builtin-persistence unit tests in `src/components/onboarding/quick-setup-utils.test.ts` + `use-quick-setup.test.ts`.

**Prevention rule (enforce in review).** Never derive a capability claim (`has_llm`, `enabled`, "configured", "ready", "available", "local") from `!api_key.is_empty()` or a single-provider OR-shortcut. Capability is a property of the **selected provider**, not of key presence or embedding mode. Any new construct missing an explicit `"none"`/`""` branch is a regression.

**Full antibody (this machine, gitignored ops memory).** `.claude/wisdom/antibodies/2026-06-02-proxy-derived-state.md` — the per-site lurking-scan table and verified-clean list.

---

## Release & CI

### SSL.com CodeSignTool download can return a landing HTML page instead of a ZIP
**Symptom.** Windows release build fails at signing, OR worse, ships an unsigned exe that SmartScreen flags. `Expand-Archive` succeeds but extracts nothing usable.

**Root cause.** The `Invoke-WebRequest` hits `ssl.com/download/codesigntool-for-windows/` which can redirect to a landing page rather than the versioned ZIP.

**Guards in place (2026-04-19 Wave 5, updated 2026-05-13).** Release workflow computes SHA-256 of the downloaded zip and hard-fails on mismatch. SHA is pinned (`033b55dc...`). Post-build step verifies Authenticode signature on every .exe/.msi before upload. EV cert issued 2026-05-12, eSigner active, all GitHub secrets set.

---

## Document hygiene

### Planning doc accidentally tracked at repo root
**Symptom.** A file named `PLAN-XYZ.md` or `AUDIT-foo.md` accidentally shows up in `git status` and `git diff` tries to commit it.

**Guards in place.** `scripts/check-doc-location.cjs` runs in pre-commit. Rejects root-level files matching internal planning patterns. `.gitignore` at root covers the planning-doc glob.

**If it happens.** Don't `--no-verify`. Move the doc to `.claude/plans/` (gitignored) OR add it explicitly to `scripts/doc-allowlist.json` with rationale.

---

## When to add to this file

- You hit a bug that took more than an hour to track down.
- You encounter a "wait why is this like this" moment and the answer is "past incident."
- A regression re-appears in a PR — add the guard AND the failure mode entry.
- An adversarial audit catches a class you missed — document the class, not just the instance.

Keep entries short. Link to code by `file:line`. If the fix requires more than two paragraphs, link to an ADR in `.claude/plans/` or a strategy doc.
