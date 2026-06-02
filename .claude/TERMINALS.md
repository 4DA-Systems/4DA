# Terminal Coordination

## Protocol
1. **Before editing**: Read this file. If your files are claimed by another terminal, STOP.
2. **Claim files**: Add your entry below with the files you'll touch.
3. **Commit Lock**: Set `**Commit Lock**: HELD` before committing. Only one terminal commits at a time.
4. **After committing**: Remove your entry and release the lock.
5. **Conflicts**: If two terminals touch the same file, the one that committed first wins. The other must rebase.

## Active Terminals

### Terminal: opus-gate-and-builtin (started 2026-06-02, continues opus-p2-polish)
Working on: (1) enforcement gate for the proxy-derived-state antibody — refactor the 2 remaining
guarded sites (channel_render.rs, settings/manager.rs) through compute_has_llm, add
scripts/check-llm-gate-honesty.cjs wired into pre-commit (with // llm-gate-ok: escape hatch);
(2) onboarding Built-in persistence — saveLlmProvider never handles 'builtin', so selecting Built-in
never sets provider=builtin; persist it (with downloaded model) when chosen + a model is downloaded.
**Claims:**
- src-tauri/src/channel_render.rs, src-tauri/src/settings/manager.rs (route through compute_has_llm)
- scripts/check-llm-gate-honesty.cjs (NEW) + .husky/pre-commit (wire it)
- src/components/onboarding/use-quick-setup.ts, setup-ai-provider.tsx, QuickSetupStep.tsx,
  quick-setup-utils.ts (builtin persistence)
**Commit Lock**: HELD (opus-gate-and-builtin) — two atomic commits (gate enforcement;
builtin persistence), one push. Both live-verified. Staging explicit paths only.

<!-- opus-p2-polish (2026-06-02): DONE — committed + PUSHED (origin/main @ 851fa416,
     36f82fbb..851fa416, rev-list 0/0). Commit Lock RELEASED, claims cleared.
     P2 polish (aed3ee7e) + AOS immune pass (851fa416), all cold-start verified:
     • Choice-gate stray "AI Provider" chip → shown only when genuinely configured. BONUS: caught +
       fixed Onboarding.tsx hasProviderConfigured (has_api_key||ollama → provider-driven) — same
       stale-key false-positive class as P1 #3, surfaced by live verify.
     • Calibrate: active_signal_axes mapped to friendly labels ×13 (no raw context/ace IDs); P0/P1
       priority code → colored urgency dot. Stack %-badges left honest (only detected stacks get a %).
     • IMMUNE PASS: antibody 2026-06-02-proxy-derived-state.md (gitignored ops memory). Scan found the
       proxy-derived-state class in 4 MORE backend gates — all routed through the now-pub(crate)
       compute_has_llm (the single source of truth): content_translation_commands.rs (HIGH user-facing),
       monitoring_jobs.rs, digest_commands.rs, content_commands.rs. Also fixed inverse builtin
       false-negative. immuneScanPending cleared. cargo test + 36 onboarding tests + tsc/eslint green.
     Open follow-up (lower priority): picking Built-in doesn't persist provider="builtin" unless the
     user downloads+starts the model — see memory project_first_run_signal_trial.md. -->


<!-- opus-p1-false-state (2026-06-02): DONE — committed + PUSHED (origin/main @ 36f82fbb,
     0f46a3d9..36f82fbb, rev-list 0/0). Commit Lock RELEASED, claims cleared.
     The 5 P1 first-run "false-state lies" are fixed and COLD-START VERIFIED LIVE (fresh
     FOURDA_DATA_DIR throwaway instance, Victauri REST :7373):
     (1) calibrate "Setup complete" now says "Private semantic search active" (was the lie
         "AI provider configured" driven by embeddingMode) — screenshot-confirmed, old string gone.
     (2) Setup-complete tech/interest counts now from authoritative get_user_context (was
         optimistic store state); fresh profile correctly shows "No technologies / Default interests".
     (3) get_personalization_context_summary returns has_llm:false/llm_tier:"none" on no-provider
         (was true/"cloud" from a stale key) — live-invoke confirmed + unit test compute_has_llm.
     (4) removed the silent auto-pull of ollama models on "optional" setup mount; added explicit
         "Download required models" button (this machine's ollama was fully provisioned so the
         missing-model branch couldn't re-fire, but no auto-pull UI appears + path is gone).
     (5) provider mutual-exclusivity: Anthropic→1 key field, click Built-in→0 (key field hidden) — verified.
     Touched: context.rs, CalibrationStep.tsx, setup-ai-provider.tsx, use-quick-setup.ts,
     QuickSetupStep.tsx, locales/*/ui.json (13). Did NOT touch pre-existing Cargo.lock /
     untracked fourda-infer-proto/.gitignore. P2 polish (stray "AI Provider" gate heading,
     "P1" internal-ID leak on calibrate rec, stack %-badges) still open — see memory. -->


<!-- opus-calibration-honesty (2026-06-02): DONE — committed + pushed (origin/main @ 8c88032e,
     b03e19bf..8c88032e). Calibration "System Health" honesty fixes shipped + verified.
     ⚠ @opus-p1-false-state: WE OVERLAP on src/components/onboarding/CalibrationStep.tsx and
     src/locales/*/ui.json — I landed FIRST (8c88032e), so please rebase/pull and PRESERVE my
     edits, don't clobber them:
       • CalibrationStep.tsx: added `ONBOARDING_ACTIONABLE` const + gated the rec "Fix" button to
         it (give_feedback/open_settings_* now render as guidance, no dead button) + added a
         `result.grade_score < 70` day-one caption (t('calibration.onboarding.gradeStartingPoint')).
         Your authoritative-counts + honest-AI-line edits are a different region → should merge clean.
       • locales: I changed calibration.onboarding.summaryProjects/summaryNoProjects wording
         ("projects"→"technologies") and ADDED calibration.onboarding.gradeStartingPoint +
         language.change across all 13 locales. Your summaryAI rewording is a different key → clean.
     Backend (calibration_commands.rs / calibration_probes.rs / probes_engine.rs): real-embedding
     discrimination+audit, provider-agnostic embedding_available (built-in fastembed no longer 0 infra),
     real hardware_detect GPU/RAM, interest diminishing-returns, + deterministic regression test.
     All gates green (tsc/eslint/13-locale i18n/10 probe tests/full pre-push). No lock held. -->


### Terminal: opus-scoring-accuracy (started 2026-05-31)
Working on: scoring accuracy — P0 search relevance (RRF-normalize→true semantic cosine), P1 stale
re-score trigger decoupling. Coordinating on analyzer.rs (owned by scan-fixes) — will NOT touch it.
**Claims:**
- src-tauri/src/db/hybrid_search.rs (surface vec distance)
- src-tauri/src/natural_language_search_engine.rs (relevance = semantic cosine, not rank-ratio)
- src-tauri/src/analysis_status.rs (P1: decouple stale re-score from new_items.is_empty)
**Commit Lock**: RELEASED (opus-scoring-accuracy) — P0 search relevance + P1 stale-drain pushed;
deep-link off-feed "inspect" fix committed locally (c19aa110), push held to avoid build contention.
Apology + correction re the cargo-fmt/git-checkout incident recorded; will never run cargo fmt
(whole tree) or git checkout/reset/stash on this shared tree again.

<!-- opus-command-search-v2: DONE — command search v2 (i18n 12 locales, deep-linking,
     frecency, responsive collapse, LRU cache) committed (bce4ae4b, afb74f7b) and pushed.
     Note: a parallel terminal also added the i18n as f1b93169; bce4ae4b reconciled key
     placement with identical translations (no duplicate keys). Lock released. -->

### Terminal: opus-scan-fixes (started 2026-05-31)
Working on: scan-driven fixes (scoring ceiling, dep-alert normalization, API key UX/telemetry, briefing prompt).
**Wave 1 claims:**
- src-tauri/src/db/dependencies/alerts.rs (severity/ecosystem normalization)
- src-tauri/src/scoring/pipeline_v2.rs (score ceiling soft-spread)
- src-tauri/src/scoring/mod.rs (PIPELINE_VERSION bump)
- src-tauri/src/settings/manager.rs (api key trim)
- src-tauri/src/settings/settings_commands_llm/mod.rs (key trim/validate)
- src-tauri/src/digest_commands.rs (actionable error message)
- src-tauri/src/analysis_rerank.rs (honest failed-call telemetry)
- src-tauri/src/monitoring_briefing.rs (briefing prompt: title refs not index)
- src-tauri/src/embeddings_providers/fastembed.rs (dynamic-quant batching regression fix)
- (no frontend files touched — all fixes are backend Rust; error banner is backend-sourced)
- STATUS: 4 commits landed locally (d10d59f8 embeddings, 28130912 scoring, 5d6fb063 alerts,
  968da2e0 llm/briefing); all pre-commit gates passed; backend test suites green.
- ⚠ PUSH BLOCKED by pre-push `tsc`: errors are entirely in opus-command-search's UNTRACKED
  src/components/search/* WIP (command-search-*.ts, CommandSearch.tsx, use-command-search.ts)
  + platform.ts. Not my files — won't touch. My commits push cleanly once those typecheck.
  @opus-command-search: please get search/* + platform.ts tsc-clean so the shared gate passes.
**Commit Lock**: RELEASED (committing complete; awaiting clean working tree to push)
- ✅ RESOLVED by opus-command-search: search/* + platform.ts are committed (e2a2ee17, plus a 1-line
  tweak swept into c37baa15) and fully tsc-clean — typecheck 0, ESLint 0, file-sizes 0, 13/13 tests
  green, live-verified via Victauri. The pre-push tsc gate is unblocked; safe to push.

<!-- opus-battle-findings: DONE — Victauri battle-findings #1-#8 + immune pass.
     Ran the AOS immune pass on @opus-scan-fixes' 4 commits (968da2e0/5d6fb063/28130912/d10d59f8):
     antibody at .claude/wisdom/antibodies/2026-05-31-parallel-fixes.md.
     • Extended the api-key trim (968da2e0) with DEFENSE-IN-DEPTH .trim() at the 5 send sites it
       missed (openai.rs, llm.rs:509/739, llm_stream.rs x2) — commit 7c4092b8. @opus-scan-fixes FYI:
       your save-side trim + my send-side trim now fully cover the phantom-401 class.
     • Class B (severity/ecosystem casing, 5d6fb063): flagged a possible one-time backfill for rows
       written BEFORE the write-path normalization — see antibody. Owner: @opus-scan-fixes.
     • Verified d10d59f8 chunk-local embedding rework is OOM-safe (bounded by 32-chunk). Lock released. -->

<!-- opus-scan-fixes — Wave 2 (C1+H1-H4 adversarial audit): DONE — committed + pushed
     560a6fe4..2f32b7c5. Terminal closed; continued in a fresh terminal (see Wave 3). -->

<!-- opus-audit-mediums — Wave 3: DONE — ALL committed + pushed (origin/main @ 13d5efbe, rev-list 0/0).
     **Commit Lock: RELEASED** (opus-audit-mediums). Terminal closing.
     @opus-provider-side: the lock is free and ALL my claimed files are clean (committed+pushed) —
     app_setup.rs, Onboarding.tsx, PreemptionTierSection.tsx, llm.rs region, runtime_paths.rs are
     safe for you to claim/edit. Working tree clean apart from pre-existing Cargo.lock +
     fourda-infer-proto/.gitignore (not mine). My isolated cold-start dev instance is being torn down.
     Shipped:
     • MEDIUMs M1 (5816?/1509e4f3 panel caps) · M2 (1a99fc41 DB retention, app_setup.rs startup sweep)
       · M3 (d654b415 onboarding skip→choice gate, Onboarding.tsx) · M4 (5570f945 PDF catch_unwind)
     • Env override ecf85191 + resolver-coherence 7bb336c6 (runtime_paths.rs + state.rs get_db_path)
     • First-run UX: 96b5d674 scroll · 5816e75c honest celebration + scan ticker · bd911a51 Add-stack
       deep-link to Projects · 1049159b CS-B fresh-picks ranking · 13d5efbe test updates
     NOTE for your provider-side work: the "Unknown provider: none" + Signal-trial + graceful-paywall
     plan is captured in memory project_first_run_signal_trial.md. The biggest cold-start lever is
     ACE auto-discovery running IN-SESSION on skip (app_setup.rs:1935 currently defers to next launch). -->

<!-- opus-provider-side (2026-06-01): DONE — committed + PUSHED (origin/main @ 3e65e382, 13d5efbe..3e65e382).
     Commit Lock RELEASED, claims cleared. Provider-side skip-setup cold-start fixes shipped & verified:
     • Wave 1 (d06e697a): instant 14-day Signal reverse-trial auto-starts on first launch
       (manager_init.rs) — VERIFIED live via cold-start FOURDA_DATA_DIR: 4da::license log line fired +
       settings.json wrote trial_started_at, tier=free, empty license. Graceful no-provider (llm.rs, all
       5 paths — no more raw "Unknown provider: none"). Clean paywall (gating.rs signal_feature_label —
       no command-name leak). Unit test green.
     • Wave 2 (3e65e382): builtin local-model sidecar auto-starts on launch when provider=="builtin" and a
       model is downloaded (app_setup.rs) — mirrors Ollama auto-warm, never auto-downloads, non-blocking.
     NOT DONE (handed to a fresh session — provider side, lower-priority follow-ups):
     • Surface the builtin local model IN onboarding (today it's Settings-only; only Ollama shows in the
       onboarding LOCAL section). BYOK is already the recommended/obvious onboarding option (green
       "Best accuracy" badge) — user confirmed BYOK should stay obvious + recommended, local = hybrid fallback.
     • ACE in-session auto-discovery on skip (app_setup.rs:1935 defers to next launch) — the biggest lever
       for "0 relevant"/uniform-23% on empty profile. The *profile* side of cold-start (vs provider side).
     • NOTE: screenshots 2827/2828 were a STALE build — current code already routes HealthBanner via
       friendlyError() and Preemption/BlindSpots via ProGate; live-verify those render clean before any edits. -->

### Terminal: opus-provider-side (2026-06-01) — Wave 1-2 pushed (see comment above). NOW: Wave 3 ACE in-session.
Working on: ACE in-session auto-discovery on skip (the PROFILE side of cold-start — biggest lever for
"0 relevant"/uniform-23%) + (paired) surface built-in local model in onboarding LOCAL section.
**Wave 3 claims (consent-based ACE + onboarding local model — NOT auto-scan; INV-004 respected):**
- src/components/onboarding/OnboardingChoiceGate.tsx (add recommended "Scan my projects — 100% local" path)
- src/components/Onboarding.tsx (handleScanAndComplete → ace_auto_discover then complete)
- src/components/onboarding/setup-ai-provider.tsx (surface built-in local model in LOCAL section)
- src/locales/en/ui.json + src/types/i18n-resources.d.ts (new onboarding keys; rely on fallbackLng=en)
- onboarding tests as needed (OnboardingChoiceGate.test.tsx)
- NOTE: app_setup.rs ACE startup logic UNCHANGED — we are NOT auto-scanning on skip (privacy). The
  scan is an explicit, consented, one-click choice at the gate.
- src/components/first-run/LoadingState.tsx (unique scan-ticker keys — fix dup rows)
- src/components/onboarding/Onboarding.tsx layout (in-flow progress header — fix sun/heading overlap)
- src-tauri/src/ace_commands/scanning.rs (single-flight guard on ace_auto_discover — fix 92% stall)
**Commit Lock**: RELEASED (opus-provider-side) — Wave 3 committed: cf5dcc79 (consent gate + builtin
card + overlap fix) + 3468c4ce (single-flight ACE guard + unique ticker keys). All gates green.
Doing final live verify (layout overlap + stall-free single scan), then push. NOT pushed yet.

### Terminal: opus-onboarding-scrollbar (2026-06-01)
Working on: kill the dead document scrollbar behind the full-screen first-run overlays (splash +
onboarding). The overlap (sun/4 over the step circles) is ALREADY fixed by @opus-provider-side
(cf5dcc79) — verified live, not duplicating. Only remaining issue from the user's screenshot is the
second, non-functional scrollbar (documentElement overflows ~80px past the viewport while the
`fixed inset-0` overlay also scrolls). Fixing at the App level to AVOID the claimed Onboarding.tsx.
**Claims:**
- src/App.tsx (scroll-lock effect: lock documentElement overflow while splash/onboarding overlay open)
- NOT touching src/components/Onboarding.tsx (claimed by @opus-provider-side) or any onboarding/* file.
**Status:** DONE in working tree (uncommitted). typecheck 0, eslint(App.tsx) 0. Mechanism live-verified
via CDP: documentElement overflow:hidden removes the dead 8px doc scrollbar (docScrollbarPx 8→0→8),
overlay's own overflow-y-auto scroll untouched. Holding commit until @opus-provider-side pushes their
unpushed cf5dcc79/3468c4ce so my App.tsx commit doesn't entangle with their in-flight onboarding push.
**Commit Lock**: not held.
