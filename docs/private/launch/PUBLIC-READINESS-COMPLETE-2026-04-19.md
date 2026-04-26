# Complete Public-Readiness Assessment — 2026-04-19

This is the comprehensive answer to "is the entire repo ready for public use, think of everything." It catalogs every axis a visitor hitting `github.com/runyourempire/4DA` sees, every gate a contributor or paying customer would care about, and every remaining launch blocker. It supersedes `HONEST-ASSESSMENT-2026-04-19.md` (session-focused) with a full-perimeter audit.

The verdict is at the end. Read the axes first, or skip to **§11 Verdict** if you only want the bottom line.

---

## §0 Scope of this assessment

"Public use" has three distinct meanings that get conflated:

1. **Source repository is public-safe.** No secrets, no PII, no internal plans leaking. Anyone can clone, read, build.
2. **End-user installable.** A developer in another country downloads the signed installer from GitHub Releases and has a working app within minutes.
3. **Commercial launch-ready.** The business side (trademarks, banking, signing certs, pricing) is in place to accept money and defend the brand.

All three are graded separately below. The repo passes (1) today, is one manual step from (2), and waits on external parties for (3).

---

## §1 Repository hygiene (public-safe code)

| Check | State | Evidence |
|---|---|---|
| Root-level doc allowlist | ✓ PASS | `scripts/public-readiness-audit.cjs` reports 0 findings over 1875 tracked files |
| PII sweep (personal emails / phone / ABN outside legal) | ✓ PASS | `check-doc-location.cjs` gate + audit, both green |
| Secret scan | ✓ PASS | pre-commit hook runs 20+ regex patterns over every staged file; commit history clean per `filter-repo` run 2026-04-18 |
| Mixed-dir allowlist (`.ai/`, `docs/strategy/`) | ✓ PASS | per-directory allowlist in `scripts/doc-allowlist.json` enforced pre-commit |
| SPDX headers on every source file | ✓ PASS | 840 files stamped this session (Waves 12-2a through 12-3f) |
| License provenance | ✓ PASS | `LICENSE` + `NOTICE` + `NOTICE?raw` imported live into third-party-licenses modal — no hand-maintained drift |
| Stashes / orphan debris | ✓ PASS | 2 orphan stashes saved to `.claude/plans/` and dropped this session (Wave 24) |
| Branch state | ⚠ 24 unpushed | Clean fast-forward to origin/main; commits are pure cleanup + Wave 12-27 work. Push at operator discretion |

**Verdict for §1:** Repo is public-safe. The `audit:public-ready` gate passes. Nothing in the tracked tree should embarrass the owner on a public fork.

---

## §2 Required public-facing files

| File | Present | Quality | Notes |
|---|:-:|---|---|
| `README.md` | ✓ | strong | Hero image, tagline, screenshot, install/download, MCP usage, 387 lines |
| `LICENSE` | ✓ | FSL-1.1-Apache-2.0 | Correct for the source-available model; auto-converts to Apache-2.0 after 2 years |
| `LICENSE.md` | ✗ | n/a | NOT missing — project uses `LICENSE` (GitHub recognizes either) |
| `NOTICE` | ✓ | 3rd-party attributions | Rendered live in-app |
| `CONTRIBUTING.md` | ✓ | present | Worth one polish pass before announcements land; not a launch blocker |
| `CODE_OF_CONDUCT.md` | ✓ | present | Standard boilerplate acceptable for v1 |
| `SECURITY.md` | ✓ | 116 lines | Disclosure process + supported-versions; reviewed during war-room wave 12 |
| `CHANGELOG.md` | ✓ | 70 lines, Keep-a-Changelog format | Needs a final v1.0.0 entry pre-tag |
| `CLAUDE.md` | ✓ | AI-agent instructions | Contains internal conventions; vetted for public-safety via document-hygiene framework |
| `.github/CODEOWNERS` | ✓ | present | — |
| `.github/FUNDING.yml` | ✓ | present | — |
| `.github/ISSUE_TEMPLATE/` | ✓ | directory present | — |
| `.github/PULL_REQUEST_TEMPLATE.md` | ✓ | present | — |
| `.github/dependabot.yml` | ✓ | present | — |
| `.github/workflows/{validate,release,nightly-audit}.yml` | ✓ | present | `release.yml` still has a `PLACEHOLDER_SHA256_FILL_IN` — see §8 |
| `package.json` repository / bugs / keywords | ⚠ → ✓ | fixed this session | Added `repository`, `bugs`, 12 keywords — was missing, now complete |

**Verdict for §2:** Complete. Every file a visitor or tooling will look for is present and non-trivial.

---

## §3 Tests & build health

| Axis | Count | Status |
|---|---|:-:|
| Rust lib tests — default features | 3109 | ✓ all green |
| Rust lib tests — `--features team-sync` | 3183 | ✓ all green |
| Rust lib tests — `--features enterprise` | 3243 | ✓ all green |
| Rust lib tests — `--all-features` | 3364 | ✓ all green |
| Frontend tests (vitest) | 1293 | ✓ all green |
| **Combined** | **4657** | **all green** |
| Cargo compile warnings — default | 0 (ex ts-rs cosmetic from dependency) | ✓ |
| Cargo compile warnings — `--all-features` | 36 (feature-gated `pub` APIs not called in test; expected for Tauri invoke-handler pattern) | ✓ |
| ESLint warnings | 695, all actionable | ⚠ incremental |
| TypeScript `tsc --noEmit` | 0 errors | ✓ |
| `cargo fmt --check` | clean | ✓ |
| pre-commit gate | green on every commit this session | ✓ |
| Installer smoke test (`scripts/verify-installer.cjs`) | PASS on local dev build | ✓ |

**Verdict for §3:** Test breadth is strong. Four feature-combination matrices all green is unusual for a project this size and a legitimate differentiator. The 695 lint warnings are all real signals that won't be fixed overnight; they don't gate launch.

---

## §4 Security posture

| Item | State | Notes |
|---|---|---|
| User API keys in keychain | ✓ | LLM, OpenAI, X, license, translation all via `keyring` crate with graceful plaintext fallback |
| Webhook signing secrets in keychain | ✓ | Wave 15 migration; write-then-read-back verify prevents silent loss on flaky keyring backends |
| Team-crypto keys (X25519 + symmetric) in keychain | ✓ | Wave 16 migration; same posture; retired the `_enc`-schema-naming-lie P1 |
| SQLCipher at-rest DB encryption | ✗ deferred to v1.1 | Keys are out of the DB; content remains — `SECURITY.md` advises treating `4da.db` as sensitive |
| Secret scanning in pre-commit | ✓ | 20+ regex patterns per staged file |
| GitHub secret scanning (server-side) | ✗ **operator-only** | `LAUNCH-ACTIONS.md` 0.3 — toggle in GitHub Settings → Code security |
| Dependabot alerts | ✗ **operator-only** | `LAUNCH-ACTIONS.md` 0.3 — same toggle group |
| Code scanning (CodeQL) | ✗ **operator-only** | Same toggle group |
| Dompurify CVE | ✓ patched this war-room | `3.3.3 → ^3.4.0` (GHSA-39q2-94rc-95cp) |
| Authenticode code signing | ⚠ **pending external** | SSL.com EV cert: IV ✓, EV org validation pending. First-install SmartScreen warning until resolved |
| macOS hardened runtime | ✓ | Wired (commit f69c0537) |
| Crash reporter | ✓ opt-in | Sentry off by default; sovereignty-grade privacy default |

**Verdict for §4:** Code-side security is production-grade. Three items remain — two require the operator to click toggles on GitHub (5 minutes), one waits on SSL.com's EV validation queue (external).

---

## §5 User-facing quality

| Item | State | Notes |
|---|---|---|
| Zero-config first run | ✓ | ACE auto-scans user projects; briefing surfaces within 60 seconds without API keys |
| Activity tracking OFF by default | ✓ | Privacy invariant INV-004; enforced in onboarding |
| Single-instance lock | ✓ | Crash-loop protection, DB-corruption recovery, Ollama pre-warm all wired |
| Installer size | ⚠ 28 MB | Above comfort zone; worth profiling for v1.1 but not a launch blocker |
| Social preview image (GitHub card) | ✗ **operator-only** | `LAUNCH-ACTIONS.md` 0.2 — upload 1280×640 PNG in Settings → General |
| Release tag on GitHub | ⚠ **operator-only** | `v0.0.5-test` showing as "latest" — looks like a dead alpha; re-tag needed (`LAUNCH-ACTIONS.md` 0.1) |
| Locale coverage | ⚠ incremental | 291 untranslated strings in lint output; every one is "a lost user in a non-English market" per feedback memory |
| Release runbook | ✓ | `docs/launch/RELEASE-RUNBOOK.md` (Wave 17) covers the manual-VM gate pre-tag |

**Verdict for §5:** First-install UX is strong. The public-facing GitHub presentation has three cheap-to-fix polish items that only the operator can execute.

---

## §6 Immune system & ops hygiene

| Item | State | Notes |
|---|---|---|
| Sentinel (session-start) | ✓ | 0 critical / 0 warning / 7 OK after Wave 25 fix |
| Bug-fix classifier | ✓ fixed Wave 25 | Was false-positiving on `chore: apply --fix`; now conventional-commits-aware |
| Antibody catalog | 3 | `2026-04-12-ghost-ipc-and-idempotency-amnesia`, `2026-04-12-silent-cli-failures`, `2026-04-19-validate-gate-breakage` (new this session) |
| Sovereignty score | 78/100 | Includes build health, test health, source pipeline, dependency freshness, invariant compliance, file-size compliance, decision debt, strategic alignment, memory health, metabolism |
| Git hygiene | ✓ 0 unclaimed | All 841 unclaimed files from session start committed |
| TERMINALS.md | ✓ current | Reflects Wave 12-27 session narrative |
| Orphan worktrees | ✓ clean | Previous session had a cleanup script; nothing pending |

**Verdict for §6:** Ops infrastructure is healthy and self-reinforcing. The immune-scan classifier is now tight enough not to cry wolf on routine chores.

---

## §7 Legal & commercial readiness

| Item | State | Notes |
|---|---|---|
| Company registration (4DA Systems Pty Ltd) | ✓ | ACN 696 078 841 registered 11 Mar 2026 |
| Banking | ✓ | Transaction + Saver accounts opened 13 Mar 2026 |
| Domain | ✓ | 4da.ai live |
| Landing page | ✓ live | 4da.ai |
| Shopify storefront | ✓ live | shop.4da.ai (merch) |
| AU trademarks | ✓ 2 of 3 | TM 2631247 "4DA" word + TM 2631246 logo both ACCEPTED early 24 Mar 2026; TM 2629468 (bare "4") being abandoned per strategy |
| US trademarks (USPTO) | ⚠ in-flight | Serial 99736230 (word) + 99736238 (logo) filed 31 Mar 2026; normal 5-9 day TSDR wait |
| Paris Convention priority deadline | ⚠ **2026-08-27** | File US/EU before this to claim AU priority dates — ON TRACK but don't let slip |
| Madrid Protocol | ⏸ pending | Basket = 2631247 + 2631246 only (not 2629468) |
| FSL-1.1-Apache-2.0 license | ✓ | Correct choice for source-available model; converts to Apache-2.0 after 2 years |
| Trademark enforcement | ✓ ready | `TRADEMARKS.md` + `CLA.md` present; enforcement unlocks when AU marks register fully |
| Apple Developer signing identity | ✓ | Team ID HVZS8TM5C5, Developer ID Application cert ready for macOS CI |
| SSL.com EV Code Signing | ⚠ **blocking** | IV verified; EV org validation pending; Windows SmartScreen will show "Unknown publisher" until this clears |
| Keygen (license validation) | ⚠ cosmetic | Account slug is the operator's personal GitHub handle; `LAUNCH-ACTIONS.md` item — rename for brand consistency |

**Verdict for §7:** Business infrastructure is in place for Australia and in-flight for US. The one practical launch gate is SSL.com EV — until it resolves, announcing "public download now" creates a first-install SmartScreen problem that kills conversion.

---

## §8 Known gaps that need a decision (not just code)

In rough priority order:

1. **SSL.com EV code signing validation** — External. No action possible until SSL.com finishes org review. Check status weekly.
2. **Re-tag GitHub release** — `LAUNCH-ACTIONS.md` 0.1. Delete the four `v0.0.x-test` drafts, re-tag current build as `v1.0.0-rc.1` or `v1.0.0` per readiness, write a proper release body. Operator-only (GitHub UI).
3. **Upload social preview** — `LAUNCH-ACTIONS.md` 0.2. 1280×640 PNG in Settings → General. Operator-only.
4. **Enable GitHub security features** — `LAUNCH-ACTIONS.md` 0.3. 4 toggles in Settings → Code security (secret scanning + push protection + dependabot + CodeQL). Operator-only, 2 minutes.
5. **CodeSignTool SHA-256 pin in `release.yml`** — `LAUNCH-ACTIONS.md` 0.4. Replace `PLACEHOLDER_SHA256_FILL_IN` with the real hash of the version of CodeSignTool we ship with. Could be automated; today still manual.
6. **AWE sidecar tag pin** — `LAUNCH-ACTIONS.md` 0.5. `release.yml` clones `runyourempire/awe --branch v0.3.0`; verify that tag exists in the AWE repo and is what we want to ship.
7. **Keygen account rename** — `LAUNCH-ACTIONS.md` 1.x. Public observable slug is personal handle; rename to `4da-systems` (or similar) and update every tracked reference (README, CHANGELOG, AI instructions).
8. **MCP tool count consistency** — `LAUNCH-ACTIONS.md` 2.x. README says 33, npm description says 30, npm v3.2.0 says 13. Pick one truth, update everywhere, re-publish npm package.
9. **Paddle webhook** — `LAUNCH-ACTIONS.md` 2.x. Move to private repo (separates payment surface from public client).
10. **The 695 ESLint warnings** — especially the 291 hardcoded English strings. Incremental cleanup. Not launch-blocking.
11. **SQLCipher at-rest encryption of `4da.db`** — v1.1 roadmap. Keys are out of the DB already; data in DB is derivable from network content.
12. **CI installer-on-VM smoke test** — v1.1 roadmap. Artifact-level check lands this session; full VM install-and-launch cycle is a future CI job.

---

## §9 What's ready to ship

In one paragraph: the **source tree** is public-safe, **compiles cleanly**, **passes 4657 tests across four feature combinations**, **enforces privacy-first defaults**, **signs with HMAC where appropriate**, **stores secrets in the OS keychain across every user-secret axis**, **ships with SPDX headers on every source file**, **passes the public-readiness audit with zero findings**, **has a seven-step release runbook** and **an artifact-level installer smoke test** that does PE validation and Authenticode verification on Windows. The 24 unpushed commits (SPDX pass + keychain migrations + 641-line dead-code purge + 4 test fixes + lint rebalance + docs + classifier fix + package.json gap-closure) are all pure positive — pushing them makes the repo more public-ready, not less.

---

## §10 What's NOT ready (the honest part)

- **Authenticode signing.** Until SSL.com EV clears, every first-time installer runs trip SmartScreen's "Unknown publisher" wall. You can ship — people will not install.
- **GitHub release tag.** `v0.0.5-test` is what `runyourempire/4DA/releases` shows as "latest." Anyone glancing at releases sees a dead alpha. 5-minute fix, operator-only.
- **GitHub security controls.** Secret scanning, push protection, dependabot, CodeQL — all off. Four clicks. Ship-blocker only in the weak sense ("you'd feel dumb when a leak happens"), but you'd feel dumb.
- **Keygen account slug.** Personal GitHub handle is publicly observable in every license-validation call. Not a secret, but inconsistent branding. 30 minutes to rename + update references.
- **Consistent MCP tool count.** README / npm description / npm v3.2.0 disagree on whether it's 13, 30, or 33. Pick one and propagate. Credibility-level issue.

None of these are technical debt in the repo. They are all operational / external / brand-consistency items the operator alone can execute.

---

## §11 Verdict

**Can the entire repo be public-use today? Yes, conditionally.**

Three gates in order of reversibility (easiest to do first):

- **Gate A (5 minutes, operator-only):** re-tag the GitHub release, upload the social preview, enable the four GitHub security toggles, pin the CodeSignTool SHA-256. After this the repo is fully **public-safe and discoverability-ready**.

- **Gate B (30-60 minutes, operator-only):** Keygen account rename + MCP tool count unification + Paddle webhook extraction. After this the repo is **brand-consistent across the commercial perimeter**.

- **Gate C (wait, external):** SSL.com EV validation completes. After this the installer is **first-install conversion-ready** and an active "go download 4DA" message is honest.

**The code is ready.** The 24 unpushed commits safely fast-forward to `origin/main` — they make the repo more public-ready, not less. Push at your timing.

**The launch is gated by Gate C.** Everything else is polish the operator can do in a single sitting.

If someone were standing next to me saying "ship it now" — I would say: push the 24 commits; do Gate A today; do Gate B this week; announce when Gate C lands. Nothing more.

---

## §12 Machine-checkable artifacts produced this session

- `scripts/verify-installer.cjs` — re-runnable artifact smoke test
- `docs/launch/RELEASE-RUNBOOK.md` — seven-step release playbook
- `docs/launch/HONEST-ASSESSMENT-2026-04-19.md` — session-level assessment (superseded by this doc at the perimeter level)
- `.claude/wisdom/antibodies/2026-04-19-validate-gate-breakage.md` — immune system record
- `.claude/plans/archive-2026-04-19-stashes/` — orphan stash archive (gitignored)

---

## §13 Next re-assessment trigger

Re-run this assessment:
- When SSL.com EV validation lands (Gate C resolves).
- Before any public announcement / Product Hunt / Show HN post.
- After each release tag cut.
- On a 30-day cadence as long as the repo stays public.

Write the next file as `PUBLIC-READINESS-COMPLETE-<YYYY-MM-DD>.md` rather than overwriting this one — the "what we thought we knew at the time" history is load-bearing for future audits.
