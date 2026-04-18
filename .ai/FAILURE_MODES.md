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

### Plaintext team-crypto despite `_enc` schema naming (v1.1 item)
**Symptom.** The `team_crypto` table has columns `our_private_key_enc` and `team_symmetric_key_enc`. In v1.0 builds, the bytes are plaintext despite the suffix.

**Status.** Documented in `SECURITY.md` and annotated at both INSERT sites in `src-tauri/src/team_sync_commands.rs`. v1.1 will wrap with SQLCipher or move to OS keychain.

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

## Release & CI

### SSL.com CodeSignTool download can return a landing HTML page instead of a ZIP
**Symptom.** Windows release build fails at signing, OR worse, ships an unsigned exe that SmartScreen flags. `Expand-Archive` succeeds but extracts nothing usable.

**Root cause.** The `Invoke-WebRequest` hits `ssl.com/download/codesigntool-for-windows/` which can redirect to a landing page rather than the versioned ZIP.

**Guards in place (2026-04-19 Wave 5).** Release workflow now computes SHA-256 of the downloaded zip and hard-fails on mismatch (placeholder SHA is in place — must be filled when actual version is pinned).

---

### AWE sidecar shipping an unpinned HEAD
**Symptom.** Two 4DA releases built 24 hours apart ship different AWE binaries because `git clone --depth 1 ...awe.git` pulled different HEADs. A compromise of the AWE repo silently propagates.

**Guards in place (2026-04-19 Wave 5).** Release workflow now clones AWE at a specific tag (currently `v0.3.0`). Must be bumped deliberately per 4DA release as part of the release checklist.

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
