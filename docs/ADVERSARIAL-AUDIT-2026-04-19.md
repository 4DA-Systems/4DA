# 4DA Adversarial Audit

Date: 2026-04-19

Scope: Whole-repo adversarial review of the public `4DA` codebase, with emphasis on release credibility, security posture, privacy claims, validation health, and what a professional engineering team is likely to criticize.

## Executive Summary

This is a real product with substantial implementation behind it. The frontend typechecks, builds, and its test suite passes. The problem is not "there is nothing here." The problem is that the repo currently overclaims on privacy and engineering rigor relative to what the code and validation state support.

The biggest issues are:

1. The Rust backend does not pass its own published validation gate.
2. Sensitive material is stored unencrypted at rest in multiple places.
3. SSRF protections exist but are applied inconsistently.
4. Public privacy, telemetry, and credential-storage claims are materially stronger than the implementation.
5. The maintenance surface is too large and too noisy for high-confidence review.

If a strong external team reviewed this repo today, the likely conclusion would be:

`"Promising product, real implementation, but trust claims and backend quality gates are not yet release-grade."`

## What Was Verified

Commands run during this audit:

- `pnpm run typecheck`
- `pnpm run build`
- `pnpm run lint`
- `pnpm run test`
- `pnpm run test -- --silent`
- `pnpm run validate:sizes`
- `pnpm audit --prod`
- `cargo test`
- `cargo clippy -- -D warnings`
- `cargo fmt --check`

Observed state:

- TypeScript typecheck: passed
- Frontend build: passed
- Frontend tests: passed when run quietly (`107` files, `1293` tests)
- Frontend tests in normal mode: extremely noisy stderr
- ESLint: `7073 warnings`, `0 errors`
- File size validation: `65` warning-level violations
- Rust tests: failed to compile
- Rust clippy with warnings denied: failed
- Rust format check: failed
- Dependency audit: `dompurify@3.3.3` vulnerable, patched in `>=3.4.0`

## Findings

### P0: Rust backend validation is broken under the repo's own release contract

The repo advertises `validate:rust` and includes it in `validate:all`, but the current backend does not pass that contract.

Evidence:

- `package.json:33`
- `package.json:44`
- `README.md:340`
- `src-tauri/src/preemption.rs:322`
- `src-tauri/src/preemption.rs:1529`
- `src-tauri/src/preemption.rs:1540`

Details:

- `cargo test` fails because tests reference `super::SUPPRESSED_GENERIC_NAMES`, but the constant is declared in-module, not in the parent module.
- `cargo clippy -- -D warnings` fails across multiple files on private interface errors, dead code, copy-clone misuse, debug impl issues, and redundant slicing.
- `cargo fmt --check` fails with a large pending formatting diff.

What a professional team will say:

`"Your published validation story is false right now. If the backend gate is red, the release gate is red."`

### P1: Team sync stores "encrypted" keys as plaintext bytes in SQLite

The local team private key and team symmetric key are written directly into SQLite, even though the schema names imply encryption.

Evidence:

- `src-tauri/src/team_sync_commands.rs:473`
- `src-tauri/src/team_sync_commands.rs:479`
- `src-tauri/src/team_sync_commands.rs:480`
- `src-tauri/src/team_sync_commands.rs:694`
- `src-tauri/src/team_sync_commands.rs:699`
- `src-tauri/src/db/migrations.rs:2496`
- `src-tauri/src/db/migrations.rs:2497`

Details:

- `our_private_key_enc` receives `crypto.private_key_bytes().to_vec()`.
- `team_symmetric_key_enc` receives `team_key.to_vec()`.
- The schema naming strongly implies encryption that is not actually happening.

What a professional team will say:

`"Relay encryption is not the same as at-rest protection. If the local DB is copied, the team's most sensitive material is already exposed."`

### P1: Webhook signing secrets are stored plaintext in SQLite

This is explicitly acknowledged in the code and not yet fixed.

Evidence:

- `src-tauri/src/webhooks.rs:146`
- `src-tauri/src/webhooks.rs:149`
- `src-tauri/src/webhooks.rs:153`

Details:

- The code comment says webhook signing secrets are stored in plaintext SQLite.
- The implementation generates a UUID secret and inserts it directly into the `webhooks` table.

What a professional team will say:

`"If you document this as debt, that's good. If you ship enterprise-style webhooks with plaintext shared secrets, that's still a security finding."`

### P1: SSRF protections exist but are not enforced in webhook registration

The repo has strong URL hardening helpers, but webhook registration bypasses them.

Evidence:

- `src-tauri/src/webhooks.rs:827`
- `src-tauri/src/webhooks.rs:835`
- `src-tauri/src/webhooks.rs:137`
- `src-tauri/src/ipc_guard.rs:204`
- `src-tauri/src/url_validation.rs:188`

Details:

- `register_webhook_cmd` accepts a URL and forwards it to storage without applying `validate_url_safe_for_request` or `validate_not_internal`.
- That leaves the backend relying on frontend behavior instead of backend enforcement.

What a professional team will say:

`"You already wrote the SSRF defense. Not using it on a high-risk outbound path is the kind of gap that gets exploited."`

### P1: Credential storage claims are stronger than the implementation

The docs say credentials are never written plaintext, but the settings flow preserves plaintext when keychain storage is unavailable or unverified.

Evidence:

- `SECURITY.md:56`
- `src-tauri/src/settings/manager.rs:202`
- `src-tauri/src/settings/manager.rs:244`
- `src-tauri/src/settings/manager.rs:298`
- `src-tauri/src/settings/manager.rs:300`
- `src-tauri/src/settings/manager.rs:303`
- `src-tauri/src/settings/keystore.rs:37`
- `src-tauri/src/settings/keystore.rs:47`

Details:

- If migration to keychain fails, the code logs that keys remain in plaintext.
- Save logic only strips fields when key presence is confirmed in keychain.
- `store_secret` returns `Ok(())` even when keychain storage fails, which masks the security posture downgrade from callers.

What a professional team will say:

`"Either the docs need to say plaintext fallback exists, or the fallback needs to stop existing. Right now the trust boundary is ambiguous."`

### P1: Database encryption is not on by default and silently degrades

The encryption scaffolding exists, but the default public build is plaintext SQLite.

Evidence:

- `src-tauri/Cargo.toml:74`
- `src-tauri/Cargo.toml:79`
- `src-tauri/src/db/encryption.rs:22`
- `src-tauri/src/db/mod.rs:109`
- `src-tauri/src/db/mod.rs:112`

Details:

- `rusqlite` is built with `features = ["bundled"]`, not SQLCipher.
- The DB encryption helper explicitly documents that keychain failure means the DB runs unencrypted.
- The DB open path logs a warning and continues unencrypted if key application fails.

What a professional team will say:

`"Privacy-first and plaintext-by-default at rest is a messaging mismatch unless you explain it extremely clearly."`

### P1: Public telemetry and crash-reporting claims are materially false as written

The public docs say zero telemetry, zero tracking, and no crash reporters. The codebase contains local telemetry and opt-in Sentry reporting.

Evidence:

- `README.md:23`
- `README.md:176`
- `SECURITY.md:95`
- `src/hooks/use-telemetry.ts:5`
- `src/hooks/use-telemetry.ts:13`
- `src/hooks/use-telemetry.ts:23`
- `src/App.tsx:203`
- `src/lib/sentry-init.ts:94`
- `src/lib/sentry-init.ts:112`
- `src/components/settings/PrivacySection.tsx:137`
- `src-tauri/src/telemetry.rs:281`

Details:

- The frontend emits `track_event` calls on launch and on view open/close.
- The backend persists telemetry into SQLite.
- Sentry initialization exists and is user opt-in, but that still contradicts the blanket statement that there are no crash reporters.

What a professional team will say:

`"This is not a minor wording issue. Privacy products live or die on exactness. If the claims are broader than the code, people stop trusting everything else too."`

### P2: Internal privacy invariants and runtime behavior are out of alignment

The internal invariants say activity tracking must be off by default, but telemetry is recorded automatically on app startup and on view lifecycle.

Evidence:

- `.ai/INVARIANTS.md:34`
- `src/App.tsx:203`
- `src/hooks/use-telemetry.ts:23`
- `src-tauri/src/settings/types.rs:611`
- `src-tauri/src/settings/types.rs:626`

Details:

- Privacy config exposes `crash_reporting_opt_in`, but there is no corresponding activity-tracking gate in this path.
- The app records telemetry before any explicit activity-tracking consent check is visible in the reviewed flow.

What a professional team will say:

`"If your own invariant says tracking must default off, then launch telemetry without an explicit gate is a policy failure."`

### P2: "Privacy by architecture" copy overstates the actual network model

The UI claims data physically cannot leave the machine because there is no server to send it to. That is not true in a product that directly talks to multiple external services.

Evidence:

- `src/locales/en/ui.json:2605`
- `src/locales/en/ui.json:2626`
- `src/locales/en/ui.json:2630`
- `src-tauri/tauri.conf.json:35`
- `src-tauri/tauri.conf.json:47`

Details:

- The CSP allowlist explicitly includes OpenAI, Anthropic, GitHub, Reddit, HN, Keygen, Ollama localhost, and updater endpoints.
- The app is local-first, not networkless.
- The current copy is too absolute to survive security review.

What a professional team will say:

`"Say local-first and direct-to-provider. Do not say data physically cannot leave the machine when the app has multiple outbound network paths."`

### P2: SSRF hardening is inconsistent in team relay setup

The team relay paths use basic scheme validation rather than the stronger internal-address blockers already present in the codebase.

Evidence:

- `src-tauri/src/team_sync_commands.rs:427`
- `src-tauri/src/team_sync_commands.rs:428`
- `src-tauri/src/team_sync_commands.rs:443`
- `src-tauri/src/team_sync_commands.rs:645`
- `src-tauri/src/team_sync_commands.rs:663`
- `src-tauri/src/ipc_guard.rs:60`
- `src-tauri/src/ipc_guard.rs:204`
- `src-tauri/src/url_validation.rs:188`

Details:

- `validate_url_input` checks length and scheme.
- `validate_url_safe_for_request` additionally parses and blocks internal/private targets.
- Relay setup currently chooses the weaker helper.

What a professional team will say:

`"If self-hosted private relays are intended, document that as an explicit tradeoff. If not, this is an avoidable SSRF gap."`

### P2: Lint is not acting as a quality gate

The lint script succeeds with thousands of warnings, so the signal is largely unusable.

Evidence:

- `package.json:16`
- `pnpm run lint` result: `7073` warnings, `0` errors

Observed warning classes:

- `@typescript-eslint/no-floating-promises`
- `@typescript-eslint/no-misused-promises`
- `@typescript-eslint/strict-boolean-expressions`
- `i18next/no-literal-string`
- `quotes`

What a professional team will say:

`"If everything is warning-level, nothing is a gate. This is noise, not enforcement."`

### P2: Frontend tests pass, but the default test signal is noisy and brittle-looking

The suite passes, but the default run emits large amounts of stderr that make it harder to see real problems.

Evidence:

- `pnpm run test -- --silent`: passed
- `pnpm run test`: noisy stderr

Observed issues during normal test output:

- jsdom canvas `getContext` not implemented errors from `src/lib/fourda-components/briefing-atmosphere.js`
- repeated React `act(...)` warnings
- multiple intentionally logged backend and state errors

What a professional team will say:

`"Passing tests are good. Passing tests that look broken are bad. CI output needs to be boring."`

### P2: The codebase exceeds its own size limits across many core files

The repo has explicit size policies, but the main subsystems substantially exceed them.

Evidence:

- `pnpm run validate:sizes`
- `src-tauri/src/scoring/simulation/corpus.rs` (`1466` lines)
- `src-tauri/src/ace/scanner.rs` (`1446`)
- `src-tauri/src/signals.rs` (`1444`)
- `src-tauri/src/db/sources.rs` (`1369`)
- `src-tauri/src/scoring/benchmark.rs` (`1334`)
- `src-tauri/src/team_sync_commands.rs` (`1027`)
- `src-tauri/src/webhooks.rs` (`906`)

Summary:

- `65` files are over warning thresholds.
- Many are central, high-change modules, which amplifies regression risk.

What a professional team will say:

`"This is hard to review, hard to reason about, and hard to safely change."`

### P2: Rust formatting discipline has drifted badly

The repo says format checking is part of validation, but current state shows a large `cargo fmt --check` diff.

Evidence:

- `package.json:33`
- `cargo fmt --check` output during this audit

What a professional team will say:

`"Broken formatting gates usually mean nobody is running the full release checks before merging."`

### P3: Public crate metadata still references optional local path dependencies

The Rust manifest includes optional path dependencies into a sibling private repo.

Evidence:

- `src-tauri/Cargo.toml:64`
- `src-tauri/Cargo.toml:67`
- `src-tauri/Cargo.toml:71`

Details:

- These are feature-gated, so they may not break the default build.
- They still make the public manifest look less self-contained and less contributor-friendly.

What a professional team will say:

`"Optional or not, public manifests should avoid environment-specific repo paths unless there is a very strong reason."`

### P3: Known vulnerable dependency remains in the shipped frontend graph

The current production dependency tree includes a published DOMPurify advisory.

Evidence:

- `package.json:65`
- Advisory: `GHSA-39q2-94rc-95cp`

Audit result:

- vulnerable: `<= 3.3.3`
- patched: `>= 3.4.0`

What a professional team will say:

`"This is easy to fix. Leaving it in the public repo signals weak dependency hygiene."`

### P3: Clippy failures show architecture sprawl and unused abstractions

The denied-warning clippy run exposes a pattern of partially integrated subsystems and dead code.

Examples from the audit run:

- `src/git_decision_miner.rs`: private interface violations
- `src/awe_spine.rs`: dead code, clone-on-copy
- `src/content_dna.rs`: unused function
- `src/evidence/materializer.rs`: unused trait and unused context
- `src/source_reputation.rs`: multiple unused structs and functions
- `src/settings/types.rs`: incomplete manual `Debug`
- `src/signals.rs`: redundant slicing

What a professional team will say:

`"There is a lot of intelligence-layer expansion here, but the repo needs a consolidation pass before it looks disciplined."`

## What Is Actually Good

This audit found real strengths too:

- The frontend type system is intact.
- The frontend build passes.
- The frontend test suite passes end-to-end when run in a quiet mode.
- The repo does not appear to be publicly tracking runtime DB files or `data/settings.json`.
- There is evidence of serious thought around privacy, local-first behavior, and outbound request hardening, even if the implementation and messaging are not yet fully aligned.

## What Strong External Teams Will Likely Say

The likely professional reaction, stripped of politeness, is:

- `The product is real.`
- `The public trust narrative is currently too absolute.`
- `The backend quality gate is not credible until Rust validation is green.`
- `Sensitive material handling at rest needs another pass.`
- `The codebase is growing faster than its enforcement mechanisms.`
- `The docs currently promise more than the implementation guarantees.`

The damaging version of that review is not:

`"This is vaporware."`

It is:

`"This is ambitious software whose strongest claims are ahead of its current operational discipline."`

## Immediate Credibility Fixes

If the goal is to improve external perception fast, the highest-leverage sequence is:

1. Fix `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check`.
2. Correct README, SECURITY, and privacy UI claims to match actual behavior.
3. Fix plaintext-at-rest handling for team crypto and webhook secrets.
4. Enforce backend URL hardening for webhooks and decide whether team relays should allow private/internal targets.
5. Upgrade DOMPurify.
6. Reduce lint noise until warnings mean something again.
7. Start splitting the largest Rust modules that are still under active development.

## Bottom Line

4DA is not failing because it lacks substance. It is failing the public-repo standard because the repo currently asks reviewers to trust a stronger story than the code, validation state, and storage behavior justify.

That is fixable. But until the trust claims and release gates are brought back into alignment with reality, serious teams are going to focus on that gap first.
