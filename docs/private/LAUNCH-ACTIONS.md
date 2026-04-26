# Launch Actions — Human-Only Checklist

These items cannot be executed from the repository. They require GitHub-web UI access, npm publisher access, external registrar or bank access, or a decision that should not be automated. Everything auto-fixable has already been shipped on `main` (see commits `49ed7022` through `af4353d1`, Waves 1–10 of the 2026-04-19 war-room pass).

Work this list top-down. Everything above a divider is a prerequisite for items below it.

---

## Tier 0 — Before next public release

### 0.1 Re-tag the current release
**Problem.** The "latest" release is tagged `v0.0.5-test`. First impression: dead alpha.
**Action.** On GitHub → Releases: delete the four `v0.0.x-test` drafts. Re-tag the published release as `v1.0.0-rc.1` (or whatever is truthful). Rewrite the release body with real install instructions + checksum block.
**Verify.** `gh release list --limit 3` returns only real tags.

### 0.2 Upload a social preview image
**Problem.** Every Twitter/Slack/Discord share gets the auto-generated white-box-with-language-pie card.
**Action.** Settings → General → Social preview → upload a 1280×640 PNG. Use `assets/4da-hero.png` as the source or create a cleaner social variant.
**Verify.** `curl -sI https://github.com/runyourempire/4DA | grep -i og:image` shows a user-supplied image, not `opengraph.githubassets.com/*`.

### 0.3 Enable GitHub security features (free, all currently OFF)
**Action.** Settings → Code security and analysis:
- Secret scanning: **Enable**
- Secret scanning push protection: **Enable**
- Dependabot alerts: **Enable**
- Dependabot security updates: **Enable**
- Code scanning (CodeQL): **Enable** (choose "Default setup" for zero config)
**Verify.** `gh api repos/runyourempire/4DA` shows `secret_scanning.status: "enabled"` and `dependabot_security_updates.status: "enabled"`.

### 0.4 Pin the SSL.com CodeSignTool SHA-256
**Problem.** `.github/workflows/release.yml` has a `PLACEHOLDER_SHA256_FILL_IN` sentinel that will hard-fail the release until replaced with the actual hash.
**Action.** Download the current CodeSignTool zip manually, compute SHA-256 (`certutil -hashfile CodeSignTool.zip SHA256` on Windows or `shasum -a 256` on macOS/Linux), paste the value in place of the placeholder. Commit.
**Verify.** A test release run on a branch completes past the "Install SSL.com CodeSignTool" step.

### 0.5 Pin the AWE sidecar tag
**Problem.** `release.yml` clones `runyourempire/awe` at `--branch v0.3.0`. Confirm that tag exists and is the AWE version to ship.
**Action.** In the AWE repo, verify the tag exists; if not, create it at the desired commit. Bump here whenever AWE is intentionally updated.
**Verify.** `git ls-remote --tags https://github.com/runyourempire/awe.git` includes `refs/tags/v0.3.0`.

---

## Tier 1 — Credibility repairs

### 1.1 Re-publish `@4da/mcp-server` from a non-personal npm maintainer
**Problem.** `npm view @4da/mcp-server maintainers` shows `the personal Gmail address` — the exact PII that `CLAUDE.md` forbids from tracked content. It isn't in the repo, but anyone following the README's MCP badge to the npm page sees it.
**Action.**
1. Create npm org `@4da` if not already done.
2. Add a maintainer using a role-based email (e.g., `npm@4da.ai`).
3. Rotate the package ownership: `npm owner add 4da-bot @4da/mcp-server && npm owner rm <current-personal-handle> @4da/mcp-server` (substitute the personal handle shown by `npm view @4da/mcp-server maintainers`).
4. Publish a patch version (`4.0.1`) to refresh the metadata.
**Verify.** `npm view @4da/mcp-server _npmUser.email` does not contain a personal address.

### 1.2 File 3–5 self-authored roadmap issues + 1 Discussion
**Problem.** Public repo shows 0 issues, 0 discussions, 1 contributor. Reads as dead.
**Action.** Open issues with labels `good-first-issue`, `help-wanted`, `enhancement`, `v1.1`. Suggested topics:
- "Add source adapter: `<your-favourite-source>`" (good-first-issue)
- "SQLCipher support for opt-in at-rest encryption" (v1.1)
- "Migrate `db/migrations.rs` unwraps to `?` + `ResultExt::context()`" (help-wanted)
- "Reduce ESLint warning count from 7K to <500" (help-wanted)
- "Investigate cold-start intelligence layer for new users" (enhancement)
File one Discussion: "What would you want 4DA to surface first when it scans your codebase?"
**Verify.** Landing page shows non-zero Issues and Discussions counts.

### 1.3 Reconcile the tool count (13 vs 30 vs 33)
**Problem.** README says 33 tools. npm v4.0.0 description says 30. npm v3.2.0 still shows 13. MCP section lists ~32 explicit names.
**Action.** Count once (read `mcp-4da-server/src/tools/` directory). Update: (a) README, (b) CLAUDE.md, (c) `package.json` description for the npm package, (d) re-publish as patch.
**Verify.** All four surfaces report the same number.

### 1.4 Migrate Keygen account name
**Problem.** The Keygen account is named after the operator's personal GitHub handle (visible as the account slug in every `api.keygen.sh/v1/accounts/<slug>/...` URL the app emits). Not a secret — publicly observable from any license-validation call — but inconsistent with the "4DA Systems" corporate identity.
**Action.** In Keygen dashboard, rename the account (e.g., `4da` or `4da-systems`). Update every tracked reference:
- `paddle-webhook/.env.example`
- `paddle-webhook/README.md:44`
- `paddle-webhook/api/paddle.ts:16`
- `NETWORK.md:190`
- Any runtime config that bakes the URL into the license-validation call
Roll the product and policy tokens. Confirm existing active licenses still validate.
**Verify.** `grep -rn "<old-account-slug>" .` returns nothing (modulo historical docs intentionally preserving context).

### 1.5 Move `paddle-webhook/` to a private repo
**Problem.** Paddle webhook handler sitting in a public repo signals a surface that isn't part of the client. Future webhook-signing-secrets plaintext issue (tracked as v1.1) is less of a concern if the repo is private.
**Action.** Create `runyourempire/4da-paddle-webhook` (private). Move the directory with git history intact. Delete from `runyourempire/4DA`.
**Verify.** Public repo no longer contains `paddle-webhook/`.

---

## Tier 2 — Release engineering

### 2.1 Publish `SHASUMS256.txt` with real releases
**Problem.** README promises per-release SHA-256 aggregates and `.sha256` sidecars. The workflow now produces them on release, but the `v0.0.5-test` release (which is what `releases/latest` points at until Tier 0.1 is done) has none.
**Action.** Once Tier 0.1 is done and a real v1.0.0-rc.1 has been tagged, verify the generated assets include `SHASUMS256.txt` and at least one `.sha256` sidecar per installer.
**Verify.** Download a single installer + its `.sha256`; `sha256sum -c <installer>.sha256` passes.

### 2.2 Add an installer smoke test to the release workflow
**Problem.** CI builds + signs installers but never runs them. A broken NSIS package that compiles cleanly can ship.
**Action.** Add a post-build job that, on the matching OS runner, performs a silent install + `--version` invocation + uninstall. Gate the release on it.

### 2.3 Code-sign runbook
**Problem.** SSL.com EV cert + Apple Developer + Tauri updater minisign pubkey all plumbed, but the secrets handoff is undocumented. New maintainers won't know the rotation protocol.
**Action.** Write `docs/RELEASE-RUNBOOK.md` covering: secret provenance, rotation cadence, what to do when CodeSignTool updates, how to regenerate the Tauri updater pubkey (existing `docs/strategy/UPDATER-KEY-ROTATION.md` has the technical side — add the operational wrapper).

---

## Tier 3 — v1.1 hardening (tracked, not blocking v1.0)

### 3.1 Wrap team-crypto plaintext
**Files.** `src-tauri/src/team_sync_commands.rs:484, 706`.
**Status.** Annotated inline with SECURITY NOTE + disclosed in `SECURITY.md`. Columns `our_private_key_enc` and `team_symmetric_key_enc` store plaintext despite the schema name.
**Action.** Pick one: (a) SQLCipher-wrap the whole DB in a dedicated migration, (b) move both values to the OS keychain with graceful fallback. Option (b) is lower-risk and aligns with how API keys are stored today.

### 3.2 Audit `db/migrations.rs` unwraps
**Status.** 39 `.unwrap()` calls in the migration path. A malformed user DB at startup hard-panics the app.
**Action.** Convert each to `?` with `ResultExt::context()` per the established pattern in `error.rs`.

### 3.3 Audit `settings/mod.rs`, `ace/mod.rs`, `ace/scanner.rs` unwraps
**Status.** 35, 28, 22 respectively.
**Action.** Same migration pattern. Schedule as a hardening sprint.

### 3.4 Oversized-file split (top 10 Rust files)
**Status.** 48 Rust files exceed the 800-line hard limit; currently warn-only. Top offenders:
`scoring/simulation/corpus.rs` (1466), `ace/scanner.rs` (1446), `signals.rs` (1444), `db/sources.rs` (1369), `scoring/benchmark.rs` (1334), `monitoring_briefing.rs` (1308), `context_commands.rs` (1298), `llm.rs` (1298), `source_fetching/fetcher.rs` (1218), `ace/mod.rs` (1215).
**Action.** Either enforce the 800-line hard error (and split these ten) or formally retire the limit. Current state (declared, unenforced) is the worst of both.

### 3.5 Reduce ESLint warnings (7,073 → <500 target)
**Status.** Lint run passes with thousands of warnings, so the signal is unusable.
**Action.** Classify the top rules by count. Either (a) fix the top three (likely `no-floating-promises`, `no-misused-promises`, `strict-boolean-expressions`) or (b) reduce the rule severity if the codebase's patterns are intentional and the rule is aspirational.

### 3.6 Webhook signing secrets → keychain
**Files.** `src-tauri/src/webhooks.rs:146–157`.
**Status.** Self-labelled `TODO(DEBT)`. Plaintext in SQLite. Only lights up once enterprise webhooks are in active use.
**Action.** Same pattern as 3.1.

### 3.7 Opt-in SQLCipher by default
**Status.** Scaffolding present. Default build is unencrypted. Disclosed in SECURITY.md.
**Action.** Make SQLCipher the default for new installs (Windows release channel first), with a migration path for existing databases.

### 3.8 SPDX headers on every source file
**Status.** ~10% coverage today. License metadata should be machine-readable.
**Action.** Script: prepend `// SPDX-License-Identifier: FSL-1.1-Apache-2.0` to every `.rs`, `.ts`, `.tsx` lacking one. Add a pre-commit check that rejects new files without the header.

---

## Tier 4 — Strategic posture (quarterly review)

- **Re-audit:** run both the self-audit and an adversarial audit quarterly. File each as `docs/ADVERSARIAL-AUDIT-YYYY-MM-DD.md` for historical comparison.
- **Bus-factor hedge:** decide whether the trademark + npm + company ownership should have a deputy documented somewhere sealed.
- **Changelog discipline:** every release tag gets a CHANGELOG entry with the audit-relevant changes linked. Stops future-me from losing the thread.

---

## How to update this file

When you complete an item, strike it through (`~~...~~`) with a short note on the commit/date. Don't delete — future reviewers benefit from seeing what was closed and when. Add new items above Tier 4 when a fresh audit surfaces them.
