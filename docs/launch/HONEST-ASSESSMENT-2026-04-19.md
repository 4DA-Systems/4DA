# Honest Pre-Launch Assessment — 2026-04-19

This is a candid snapshot of the state of 4DA on the day the war-room pass (Waves 1–17) closed. It's a complement to `LAUNCH-ACTIONS.md` (which enumerates the human-only tasks that remain) and to `release.sh` (which automates the hard technical gate). The goal is to write down what *we actually know* vs. what *we hope*, so the release decision is made against reality.

---

## What's landed this session

The war-room pass ran in two batches.

**Waves 1–10** (earlier sessions, commits `49ed7022` … `af4353d1`): single-instance lock, crash-loop protection, persistent diagnostic logs, license-gating audit, cloud-sync detection, third-party-licenses modal wired to `NOTICE?raw`, installer build fix, content sophistication test suite, adversarial-audit doc, UI-only launch checklist. These are catalogued in the commit log and in `LAUNCH-ACTIONS.md`.

**Waves 11–17** (this 2026-04-19 session): 17 new commits. Concretely:

- **Wave 12** — FSL-1.1-Apache-2.0 SPDX headers added to every source file (840 files, 12 commits, `9ca584a1` → `f88d436b`). License provenance is now unambiguous on every file the user ships.
- **Wave 13** — ESLint severity rebalance (`bf4ec8ad`). 7073 → 733 warnings, 10× noise reduction. Every remaining warning is actionable (real bugs, missing i18n, type-safety gaps). Style rules that no tool was enforcing (quotes/semi/comma-dangle) are off; aspirational rules nobody was cleaning up (strict-boolean-expressions) are off.
- **Wave 14** — migrations.rs + broader unwrap audit (no commit — clean finding). `migrations.rs` production code is clean. The broader sweep found 18 `unwrap()`/`expect()` calls in production code across 9 files; 14 are in test/bench/simulation modules and 4 are legitimate unrecoverable panics (hardcoded URL parse, Tauri build failure, CARGO_MANIFEST_DIR parent, global semaphore close). Each of the 4 has a descriptive message and is documented in-line. Audit tooling was temporary; deleted after run.
- **Wave 15** — Webhook signing secrets → OS keychain (`6f644efa`). `webhook_secret__<id>` keychain keys with write-then-read-back verify; plaintext DB fallback for hosts where the keychain is unreliable. 6 new tests, all 8 in the module pass. Retires `G-P1-11` from the pre-launch audit.
- **Wave 16** — Team-crypto X25519 private + symmetric keys → OS keychain (`0b8eba73`). `team_privkey__<id>` and `team_symkey__<id>` with the same write-then-read-back posture. Retires the `_enc`-schema-naming-lie P1 from `ADVERSARIAL-AUDIT-2026-04-19.md`. All 17 crypto module tests pass. `FAILURE_MODES.md` entry flipped to **RESOLVED**.
- **Wave 17** — Installer smoke test + release runbook + this assessment. `scripts/verify-installer.cjs` validates PE header, size, SHA-256, Authenticode on a built NSIS installer. `RELEASE-RUNBOOK.md` is the step-by-step human playbook that wraps `scripts/release.sh`.

**Load-bearing defensive measure discovered during Waves 15–16**: on at least one Windows Credential Manager configuration, `keyring::Entry::set_password()` returns `Ok(())` but the next `get_password()` returns `NoEntry`. Trusting the write would have silently dropped the webhook signing secret or the team-crypto private key. The write-then-read-back verify pattern now in both modules is not a nice-to-have — it's what keeps us from shipping a silent-loss bug on that class of host.

---

## What's ready to ship

- **Cryptographic posture.** All user secrets (LLM API keys, license key, webhook signing secrets, team X25519 private keys, team symmetric keys, translation API key) now route through the OS keychain with graceful plaintext fallback. The fallback is explicit, logged, and intentional; it is not a bypass.
- **Privacy default.** Activity tracking OFF by default. Sentry crash reporting opt-in. Zero telemetry without explicit consent. The onboarding flow honors this.
- **Startup resilience.** Single-instance lock, crash-loop protection, DB-corruption recovery, Ollama pre-warm gate, Vite-dep-update-while-running postinstall guard are all in place (see `.ai/FAILURE_MODES.md` for the specific incidents each one prevents).
- **Test breadth.** 3483 tests total (2215 Rust lib + 1268 frontend as of the baseline; the session may have added a few). `cargo test --features enterprise` and the frontend suite both run green.
- **License hygiene.** Every tracked source file carries the SPDX header. `NOTICE` is imported live from raw source into the in-app third-party-licenses modal — no more hand-maintained drift.
- **Document hygiene.** Pre-commit gate, mixed-dir allowlist, PII sweep, public-readiness audit. 120+ internal docs purged from public-reachable paths; history force-pushed.

---

## What is intentionally deferred (known, not forgotten)

- **CI installer smoke test on Windows.** The artifact-level smoke test landed this session; an automated "install in a clean Windows VM and launch" CI job is on the v1.1 roadmap. For now, step 5 of `RELEASE-RUNBOOK.md` is the human-executed equivalent and is non-negotiable.
- **SQLCipher at-rest encryption of `4da.db`.** Not shipping in v1.0. The keys that previously lived in the DB as plaintext now live in the keychain; everything else in `4da.db` is content derived from the network (rankings, scraped text) or user-generated data the user could produce themselves. Users are advised in `SECURITY.md` to treat `4da.db` as sensitive. SQLCipher is a v1.1 candidate.
- **The remaining 733 ESLint warnings.** Kept as warn, not off. Each one is genuinely actionable. Cleanup is incremental; no single one blocks launch.
- **AWE sidecar pinning.** `CodeSignTool` SHA-256 pin and AWE sidecar pin are in `LAUNCH-ACTIONS.md` 0.4 as human-only tasks that happen at release-cut time.
- **Forget-team-keys wiring.** The keystore scrub helper for team-crypto exists (`team_sync_crypto::forget_team_keys`) but isn't invoked yet because there is no "leave team" command. When that command lands it should call this helper.

---

## Known risks

- **First-install user experience on Windows SmartScreen.** Pre-signing, the installer triggers the full SmartScreen warning ("Windows protected your PC — Unknown publisher"). Post-signing with the SSL.com EV cert (pending EV org validation — see `LAUNCH-ACTIONS.md`) this becomes a brief "verified publisher" prompt. Do not ship publicly before EV is complete; the SmartScreen warning kills first-install conversion.
- **Keychain-lying backends.** We've observed Windows Credential Manager setups where the write-then-read verify is necessary. We haven't characterized which configurations. On those hosts, the plaintext DB fallback is what keeps webhooks signing and team sync working — it's not a degraded mode the user will see, but it does mean the secret is on disk. The warn-level log is there so a support engineer can triage.
- **NSIS installer size (28 MB).** Above the 20 MB comfort zone for "curious user tries it." Not a launch blocker, but worth profiling in v1.1 — half of the size is probably Tauri's WebView2 bootstrapping + debug symbols that `cargo build --release` missed.
- **Missing locale coverage.** 291 `i18next/no-literal-string` warnings = 291 strings that render in English even when the user has picked another locale. Not a correctness bug, but every untranslated string is a lost user in a non-English market. Incremental fix; tracked as the biggest remaining category in the lint rebalance.

---

## What I'd want to verify before the public tag

In order of reversibility-of-mistake (irreversible first):

1. `scripts/release.sh` reports green on a freshly-cloned checkout (not the development one).
2. `scripts/verify-installer.cjs` reports Authenticode: Valid on the final signed artifact.
3. Step 5 of `RELEASE-RUNBOOK.md` (manual VM install) completes on a Windows 11 fresh snapshot.
4. A second human has read the release notes and the SHA-256 published in them matches the SHA-256 of the uploaded artifact (step 7).
5. `LAUNCH-ACTIONS.md` Tier-0 items (0.1 through 0.9) are ✓ (GitHub security features on, social preview uploaded, SSL.com CodeSignTool pinned, etc.).

I would not ship with any of 1–5 unresolved. I would ship with any of the items in *What is intentionally deferred* or in *Known risks* above, provided the user is informed.

---

## Write-down format for future sessions

When the next pre-launch assessment runs (likely pre-v1.1), overwrite this file at `docs/launch/HONEST-ASSESSMENT-<YYYY-MM-DD>.md` rather than editing this one. The history of "what we thought we knew at the time" is worth keeping.
