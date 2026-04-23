# Terminal Coordination

## Protocol
1. **Before editing**: Read this file. If your files are claimed by another terminal, STOP.
2. **Claim files**: Add your entry below with the files you'll touch.
3. **Commit Lock**: Set `**Commit Lock**: HELD` before committing. Only one terminal commits at a time.
4. **After committing**: Remove your entry and release the lock.
5. **Conflicts**: If two terminals touch the same file, the one that committed first wins. The other must rebase.

## Active Terminals

### T-SIGNAL-HARDENING-W4-2026-04-23

**Scope:** Signal hardening Phases 2-6 — remaining recommendations.

**Wave 4:** `src/components/ResultsView.tsx`, `src/components/KnowledgeGapsPanel.tsx`, `src-tauri/src/scoring/pipeline_v2.rs`
**Wave 5:** `src/components/result-item/FeedbackButtons.tsx`, `src-tauri/src/commands.rs`
**Wave 6:** `src-tauri/src/blind_spots.rs`, `src/components/result-item/ResultItemExpanded.tsx`

**Commit Lock:** not held

---

<!-- T-TITANICA-INTEGRATION-2026-04-24 DONE (77a01c06). Archetype penalties
     wired into PASIFA V1+V2, adversarial deliberation into blind spots,
     grounded reasoning soft gate in evidence validation. -->

### T-WAR-ROOM-RECOVERY-2026-04-19 (Waves 12-27 COMPLETE, pushed to origin)

**Scope:** Recovery of stuck Wave 12 SPDX commit. Previous terminal hung on
pre-commit secret-scan over 431 staged Rust files (secret-scan runs ~25 regex
patterns over each file's full contents — O(n) per batch). Strategy: split
into 80-file batches (matched Part 1 precedent: 79 files in ~60s).

Wave 12 results — 12 new commits, 840 files, pre-commit green every time:

- Rust backend (431 files, 6 commits):
  - 75fe0c97 part 2a/3 (80)
  - 7244b87a part 2b/3 (80)
  - 28518252 part 2c/3 (80)
  - 73d29591 part 2d/3 (80)
  - 2300f99f part 2e/3 (80)
  - 14f074e6 part 2f/3 (31 — final Rust)
- Frontend src/ (409 files, 6 commits):
  - bb136097 part 3a/3 (80)
  - 3365de2d part 3b/3 (80)
  - f884b285 part 3c/3 (80)
  - cbe56947 part 3d/3 (80 + CRLF→LF normalization on 4 files)
  - 5057df4b part 3e/3 (80 + CRLF→LF on 10 files)
  - f88d436b part 3f/3 (9 — final Wave 12)

Waves 13-17 landed sequentially after 12. Final session tally: **17 commits**
(12 Wave-12 batches + 1 TERMINALS record + Waves 13/15/16/17 = four feature
commits; Wave 14 was a clean-audit finding with no commit). Working tree clean.

  - bf4ec8ad Wave 13 ESLint rebalance (7073 → 733 warnings, 10x signal)
  - (no commit) Wave 14 unwrap audit — migrations.rs clean, broader scan
    found 18 prod unwraps: 14 are test/bench/sim, 4 are legitimate
    unrecoverable panics, all documented
  - 6f644efa Wave 15 webhook signing secrets → keychain (6 new tests)
  - 0b8eba73 Wave 16 team-crypto X25519 + symmetric keys → keychain,
    retires the _enc-schema-naming-lie P1; FAILURE_MODES.md entry
    flipped to RESOLVED
  - 4d44c52f Wave 17 installer smoke test + release runbook +
    HONEST-ASSESSMENT-2026-04-19.md

### Waves 18–23 — deep-audit sweep (continuation)

  - (no commit) Wave 18 deep audit: Rust warnings mapped (66 with
    --all-features), ESLint auto-fixable (38), full test suite state,
    sentinel immune-scan pending, file-size compliance, outstanding TODOs
  - 4e62fdb0 Wave 19 remove 3 unused pub re-exports in glyph_integration
  - 52df2bcd Wave 20 apply ESLint --fix (733 → 695 warnings; 26 files)
  - 3cdda4c0 Wave 21 unbreak 4 pre-existing test failures
    (commitment_contracts tightened refutation validator, preemption
    added 'express' to suppression list, sso port bumped 4445 → 4446,
    organization added 7-day retention grace period)
  - (no tracked commit; gitignored) Wave 22 sentinel immune scan for
    commit 49ed7022 — antibody recorded at
    .claude/wisdom/antibodies/2026-04-19-validate-gate-breakage.md,
    ops-state.json cleared of immuneScanPending
  - 5a6a8621 Wave 23 delete 641 lines of unwired dead code —
    source_reputation.rs + classify_content_for_source_with_reputation
    + 22 tests for deleted code; fixed ParsedCommit visibility;
    silenced doctrine-bound evidence scaffolding with allow(dead_code)

### Waves 24–27 — final cleanup + public-readiness push

  - (gitignored state) Wave 24 save-and-drop two 2026-04-05/06 orphan
    stashes whose base commits were rewritten away by the 2026-04-18
    filter-repo run. Patches preserved in .claude/plans/.
  - 978a8679 Wave 25 fix immune-scan bug-fix classifier. Old regex
    `\b(fix|bug|patch|...)\b` false-triggered on `chore(lint): apply
    ESLint --fix`. Tightened to conventional-commits form
    ^<sha>\s+(fix|hotfix)[(!:] Verified with 9/9 test cases. Cleared
    the false-positive from ops-state.json.
  - 89cc841b Wave 26 complete public-readiness assessment covering 13
    perimeter sections + package.json discoverability gaps (added
    repository, bugs, keywords fields that were missing).
  - Wave 27 pushed 37 commits to origin/main as a clean fast-forward.
    Pre-push ran the full test matrix green; upstream tracking
    configured.

Final state across the 2026-04-19 session (27 commits, 37 pushed
together with previous-session work):
  - Rust: 25 default-feature warnings → 0 (just the ts-rs cosmetic
    from a dependency we don't control); 66 with --all-features → 36
    (remaining are feature-gated pub APIs expected by Tauri's invoke
    handler pattern)
  - ESLint: 7073 warnings → 695 (90% reduction, every remaining one
    actionable)
  - Tests: 3109 default + 3364 all-features + 1293 frontend = 7766
    total, all green across the matrix
  - Sentinel: 0 critical / 0 warning / 7 OK
  - Public-readiness audit: 0 findings over 1875 tracked files
  - origin/main: fast-forwarded to 89cc841b

Launch gating (from PUBLIC-READINESS-COMPLETE-2026-04-19.md):
  Gate A (5 min operator-only): GitHub release re-tag, social preview
         upload, enable 4 security toggles, pin CodeSignTool SHA-256
  Gate B (30-60 min operator-only): Keygen slug rename, MCP tool-count
         unification, move Paddle webhook to private repo
  Gate C (external wait): SSL.com EV validation — THE launch gate;
         without it first-install SmartScreen warning kills conversion

Key finding surfaced in Waves 15/16: on some Windows Credential Manager
configurations `set_password` returns Ok but the next `get_password`
returns NoEntry. Trust-the-write would have silently lost the webhook
signing secret AND the team-crypto private key. The write-then-read-back
verify pattern in both modules is what keeps us from shipping a silent-
loss bug on that class of host — documented in the assessment doc so
future readers questioning the verify overhead know why it stays.

**Commit Lock:** not held

<!-- T-PUBLIC-READY done (a3301906 + filter-repo → da8c87fa, 2026-04-18).
     Document Hygiene v2: mixed-dir allowlist + PII gate + public-readiness
     audit. Moved 120+ internal docs from .ai/, docs/strategy/,
     docs/marketing/, docs/ ops, merch-print-ready/. History purged.
     Force-pushed to origin. pnpm run audit:public-ready passes clean. -->

<!-- T-DOC-HYGIENE done (c677362b → filter-repo → 90c5d934, 2026-04-18).
     v1 framework: 6 layers, 30 legacy plan docs purged from root. -->

<!-- T-INTEL-RECON Phase 13b done (5d19f6c4). All 5 visual QA issues
     fixed: buttons functional, template killed, confidence honest,
     Evidence tab unique, naming fixed. App running live with fixes. -->

<!-- T-DOC-HYGIENE done (c677362b 2026-04-18). Six-layer Document Hygiene
     framework: location doctrine, gitignore patterns, pre-commit gate,
     CLAUDE.md rule, retrospective move of 30 legacy plan docs, audit
     one-liner. Prevents PLAN*.md / STRATEGY / AUDIT / etc leaking to
     repo root. Gate tested working. -->

<!-- T-INTEL-RECON Phase 13b done (5d19f6c4). All 5 visual QA issues
     fixed: buttons functional, template killed, confidence honest,
     Evidence tab unique, naming fixed. App running live with fixes. -->

<!-- T-INTEL-RECON Phase 13a done (7e41aae0). All 5 gaps from honest
     assessment addressed: auto-seed, monitoring, persistence, i18n, unwrap.
     Phase 13b = visual QA (founder soak). Phase 14 = ship. -->

<!-- T-INTEL-RECON (Phases 0-12 done 2026-04-17):
     Intelligence Reconciliation — 12 of 14 phases shipped.

     Phase 12 03defeb9 — Evidence Tab replaces Momentum.
     Tab model: Briefing · Preemption · Blind Spots · Evidence · Results.

     Next: Phase 13 (pre-launch hardening), Phase 14 (ship). -->

<!-- T-INTEL-RECON (Phases 0-11 done 2026-04-17):
     Intelligence Reconciliation — 11 of 14 phases shipped.
     Phase 11 67a8b99b — Commitment Contract + refutation watcher.
     Next: Phase 12 Evidence Tab, Phase 13 hardening, Phase 14 ship. -->

<!-- T-INTEL-RECON (Phases 0-10 done 2026-04-17):
     Intelligence Reconciliation — Phases 0-10 shipped in one session.

     Session commits:
       cd5e31c8 — Phase 0  plan lock
       11a9fc41 — Phase 1  dead code purge (19 files)
       7f873d42 — Phase 2  EvidenceItem canonical type
       223d810e — Phase 3  Preemption → EvidenceFeed
       e1dde0f4 — Phase 4  Blind Spots → EvidenceFeed
       60fb25c9 — Phase 5  Knowledge Decay + Signal Chains → EvidenceFeed
       94551289 — hygiene: brand tagline rollout
       8a085554 — hygiene: source reputation feature
       31fa095d — Phase 6  AWE Context Bridge (5 → 17 fields)
       68e18a37 — Phase 7  Git Decision Miner (Cold Start Layer 1)
       62fce050 — Phase 8  Curated Seed Corpus (Cold Start Layer 2)
       c162f7b3 — Phase 9  AWE Spine (precedent attachment)
       46412794 — Phase 10 Confession Box + Decision Brief

     Next (Phase 11): Commitment Contract
     Next (Phase 12): Evidence Tab rebuild
     Next (Phase 13): Pre-launch hardening
     Next (Phase 14): Ship -->


<!-- T-INTEL-RECON (Phases 0-8 + 2 hygiene recoveries done 2026-04-17):
     Intelligence Reconciliation + Cold Start complete.

     Session commits:
       cd5e31c8 — Phase 0 plan lock
       11a9fc41 — Phase 1 dead code purge (19 files)
       7f873d42 — Phase 2 EvidenceItem canonical type
       223d810e — Phase 3 Preemption → EvidenceFeed
       e1dde0f4 — Phase 4 Blind Spots → EvidenceFeed
       60fb25c9 — Phase 5 Knowledge Decay + Signal Chains → EvidenceFeed
       702222c0 — mid-session lock release
       94551289 — hygiene: brand tagline rollout (5 files)
       8a085554 — hygiene: source reputation feature (8 files)
       31fa095d — Phase 6 AWE Context Bridge (5 → 17 fields)
       fe99ae68 — lock release
       68e18a37 — Phase 7 Git Decision Miner (Cold Start Layer 1)
       62fce050 — Phase 8 Curated Seed Corpus (Cold Start Layer 2)

     Rust test deltas (intelligence scope):
       Phase 0 baseline: 2933
       After Phase 8:    3099 (+166 tests net)

     Next (Phase 9): AWE Spine Wiring — every EvidenceItem gets its
     explanation + calibrated confidence + precedent lookup from AWE,
     not hand-rolled in each materializer. -->


<!-- T-INTEL-RECON (Phases 0-6 + 2 hygiene recoveries done 2026-04-17):
     Intelligence Reconciliation — collapse phase + bridge complete.

     Session commits:
       cd5e31c8 — Phase 0 plan lock
       11a9fc41 — Phase 1 dead code purge (19 files)
       7f873d42 — Phase 2 EvidenceItem canonical type
       223d810e — Phase 3 Preemption → EvidenceFeed
       e1dde0f4 — Phase 4 Blind Spots → EvidenceFeed (with score)
       60fb25c9 — Phase 5 Knowledge Decay + Signal Chains → EvidenceFeed
       702222c0 — mid-session lock release
       94551289 — hygiene recovery: brand tagline rollout (5 files)
       8a085554 — hygiene recovery: source reputation feature (8 files)
       31fa095d — Phase 6 AWE Context Bridge (5 → 17 fields)

     Hygiene status: working tree clean. 13 unclaimed files cleared.

     Next (Phase 7): Cold Start Layer 1 — Git history miner. Scans user
     repos for decision-shaped commits, populates AWE wisdom graph with
     seeded priors so Day 0 transmutations return ≥3 precedents. -->


<!-- T-INTEL-RECON (Phases 0-5 done 2026-04-17, session pause):
     Intelligence Reconciliation — collapse phase COMPLETE.

     Phase 0 cd5e31c8 — plan lock
     Phase 1 11a9fc41 — dead code purge (19 files)
     Phase 2 7f873d42 — EvidenceItem canonical type
     Phase 3 223d810e — Preemption → EvidenceFeed
     Phase 4 e1dde0f4 — Blind Spots → EvidenceFeed (with score)
     Phase 5 60fb25c9 — Knowledge Decay + Signal Chains → EvidenceFeed

     Four lenses emit canonical EvidenceItem. Legacy types remain
     internally where still needed (monitoring_briefing, preemption's
     own detect_chains usage). IPC boundary is 100% canonical.

     Test totals (Rust / Frontend):
       Phase 0:  2933 / 1257
       Phase 5: 3035 / 1276 (+102 Rust, +19 FE)

     Next (Phase 6): AWE Context Bridge — DeveloperContext 5→17 fields
     populated from 4DA's actual data. Unblocks meaningful AWE
     transmutations for downstream phases.
-->

**Note to next session:** there are 13 unclaimed files accumulating from
other terminals/sessions (CLAUDE.md, content_dna.rs, lib.rs, source_reputation.rs,
SourceConfigPanel.tsx, SourcePreview.tsx, source-input-parser.{ts,test.ts},
tauri.conf.json, ui.json, 02-landing-page-copy.md, faq.html, index.njk).
Not T-INTEL-RECON's claim. Owner should commit or discard before more work lands.

<!-- T-INTEL-RECON (Phases 0-4 done 2026-04-17, session pause):
     Intelligence Reconciliation — 12→5 tab collapse, AWE→spine reframe,
     one EvidenceItem canonical type.

     Phase 0 cd5e31c8 — plan lock (3 strategy docs + doctrine)
     Phase 1 11a9fc41 — dead code purge (19 files, Momentum + 5 AWE panels)
     Phase 2 7f873d42 — EvidenceItem canonical type + materializer trait
     Phase 3 223d810e — Preemption emits EvidenceFeed of EvidenceItems
     Phase 4 e1dde0f4 — Blind Spots emits EvidenceFeed (score on feed.score)

     Test deltas per phase (frontend → Rust):
       Baseline:  1257 → 2933
       Phase 2:   1257 → 2969 (+36 evidence tests)
       Phase 3:   1257 → 2984 (+12 preemption conversion + 3 feed)
       Phase 4:   1276 → 3019 (+19 FE blindspot bucket, +8 RS conversion, +3 evidence)

     Next (Phase 5): Knowledge Decay + Signal Chains collapse.
     KnowledgeGap + MissedItem → EvidenceItem { kind: Gap | MissedSignal }
     SignalChainWithPrediction → EvidenceItem { kind: Chain }
     Wire into existing KnowledgeGapsPanel and its command.
-->


<!-- T-INTEL-RECON (Phases 0-2 done 2026-04-16, paused for session review):
     Intelligence Reconciliation — 12→5 tab collapse, AWE→spine reframe,
     one EvidenceItem canonical type.

     Phase 0 commit cd5e31c8 — plan lock:
       - docs/strategy/INTELLIGENCE-RECONCILIATION.md
       - docs/strategy/EVIDENCE-ITEM-SCHEMA.md
       - .claude/rules/intelligence-doctrine.md
       - CLAUDE.md pointer
     Phase 1 commit 11a9fc41 — dead code purge:
       - 19 files deleted (Momentum tab, 5 AWE UI panels, 2 vanity
         components, CategoryChapterView, related tests)
       - 12→10 tab model (removed 'insights' + 'chapters')
       - 1257/1257 frontend tests green
     Phase 2 commit 7f873d42 — EvidenceItem canonical type:
       - src-tauri/src/evidence/ (5 files, ~900 LOC)
       - EvidenceMaterializer async trait
       - Runtime schema validator
       - 36/36 evidence:: tests green
       - 10 ts-rs bindings exported

     Next (Phase 3): collapse preemption.rs PreemptionAlert → EvidenceItem,
     refactor PreemptionView.tsx to consume canonical type. Done-when:
     Preemption tab visually identical, backed by shared type. -->


<!-- T-INTEL-MESH Phase 2 (done 2026-04-15, recovered after host crash):
     Decoupled the 50/50 blend via the reconciler (Layer 3 of the mesh).
     Pipeline is now authoritative; advisors bounded to ±0.15 adjustment.
     Disagreement (|pipeline - advisor| > 0.30) surfaces as a UI flag,
     never an override. No hard rejects in the reconciler path.
     - new module src-tauri/src/reconciler.rs (295 lines, 15 unit tests
       including cap boundaries, multi-advisor ensemble, disagreement
       threshold exclusive/inclusive, constants-vs-design-doc guards)
     - new types: DisagreementKind enum + disagreement field on
       ScoreBreakdown (ts-rs exports generated)
     - analysis_rerank.rs forks on settings.rerank.reconciler_enabled
       (default true). Legacy 50/50 + hard-reject path retained for
       A/B and rollback. New path uses reconciler, tallies skeptical
       disagreements as "rejected" for telemetry.
     - settings: reconciler_enabled flag with serde back-compat
     - test harness fix: settings_commands_tests.rs RerankConfig literal
       updated for the new field
     Tests: 15 reconciler + 6 rerank + 23 scoring::pipeline all green.
     2810 lib tests compile clean.
     Recovery context: host crashed mid-session before commit; files
     were intact on disk. Recovery session verified, regenerated bindings,
     and shipped. -->

<!-- T-INTEL-MESH Phase 3 (done 2026-04-15):
     Provenance substrate landed.
     - Phase 56 DB migration: provenance table + 4 indexes
     - new module src-tauri/src/provenance.rs (Provenance, ModelIdentity,
       ArtifactKind, record/batch/query helpers; 14 unit tests)
     - new struct AdvisorSignal + advisor_signals field on ScoreBreakdown
     - analysis_rerank.rs stamps advisor signals + persists provenance
     - llm_judge.rs: PROMPT_VERSION constant exposed
     - ts-rs bindings: AdvisorSignal, Provenance, ArtifactKind,
       ModelIdentity all generated
     Tests: 458 scoring + 26 provenance + 31 rerank + 2 migration = all green -->

<!-- T-INTEL-MESH Phase 1 (done 2026-04-15):
     Commits:
       29250000 — feat(privacy): opt-in Sentry crash reporting with aggressive scrubbing
       252be640 — feat(onboarding): pre-warm Ollama gate before first deep-scan embedding
       [pending] — feat(mesh): Phase 1 injection hardening + Intelligence Mesh design doc
     AWE transmute recorded: dc_019d8d0c-c376-75c1-bfac-40397ff2157c (reversibility 65%, act_now=false — patience).
     Full pivot is 6-7 weeks; this session landed Phase 1 only. -->

<!-- T-BRIEFING-FLAGSHIP (done 2026-04-15):
     Commits: 7839ba86 (quality gate + groundedness + prompt rewrite + dedupe),
              9b214124 (UI abstention rendering — snapshot path),
              85dcb337 (end-to-end pipeline tests + free-briefing abstention).
     Net: 2760/2760 Rust tests, 1237/1237 frontend tests, zero regressions. -->

---

## Completed in recent sessions (historical record, no active claim)

- **T2** — Phases 0-3 execution (10 commits: 3834a557, 792ec1ad, 20ab8271, 93d7295f, e9dbc459, c9d31173, f8a3a5b9, 09f6078d). Total 75+ files, ~12,000 lines, 3,734 tests.
- **T-PREEMPTION-FIX** — preemption feed 248s→2.9s (85x speedup), blind_spots + project_health_dimensions schema fixes. Commit `dd71762b`. Worktree removed.
- **T-SCORING** — experience UI + direct/transitive deps + tighter threshold + anti-gaming recal. Commits `26ef0e48`, `700ab104`.
- **T-GLYPH** — Glyph Envelope Protocol foundation (4 docs commits) + Phase 2 integration module (commit `7548a690`, feature-gated behind `glyph_audit`). 4 passing tests.
- **T-PRELAUNCH-HARDENING** — All four pre-launch risk classes (a-d) mitigated. WebView2 + Ollama version checks, DB corruption recovery (wired), static-CRT verified. Commits `15f2c708`, `96ba9fed`, `2b59be0d`, `d0b5070d`, `76de616b`. Strategy doc: `docs/strategy/PRELAUNCH-HARDENING.md`. Key rotation runbook: `docs/strategy/UPDATER-KEY-ROTATION.md`.
- **T-HYGIENE** — orphaned worktree cleanup + prevention script. Commit `5eea8b1e`. Deleted 11 dead branches + 6 stale directories (reflog recoverable 90 days).
- **T-DOCS-HYGIENE** — strategy docs batch (7 files, 932 insertions). Commit `d6fc22a0` (was `9f62eb7c` pre-rebase). Later relocated out of product repo by `7b081c1f`.
- **T-WAR-ROOM (recovered)** — AWE app_handle threading: register_awe_app_handle + cached_awe_app_handle + run_awe_autonomous_now command, 102 insertions. Committed by T-LOCK-CLEANUP as `0f0ae5aa` after user confirmed T-WAR-ROOM was no longer active. cargo check clean.
- **T-LOCK-CLEANUP** — stalled-terminal recovery + diverged branch rebase. Removed stale stash lock, committed T-WAR-ROOM's work, rebased 13 local commits onto origin/main (picked up `dd71762b`), pushed to origin. Tip: `0f0ae5aa`. User authorized all steps.
- **T-SILENT-FAILURE-DEFENSE** — Silent-Failure Defense Architecture Layer 1 + Layer 2 foundations. Strategy doc (`docs/strategy/SILENT-FAILURE-DEFENSE.md`), Layer 2 validator (`scripts/validate-boundary-calls.cjs`, caught ops-session-end.cjs idempotency-amnesia bug on first run), Layer 1 typed wrapper skeleton (`src-tauri/src/external/awe.rs` with 5 passing guard tests encoding Bug #1/#2/binary-not-found defenses), lib.rs `mod external;` declaration, fixed ops-session-end.cjs hook amnesia. 48 unverified `Command::new` sites remain as documented migration backlog. User authorized commit-lock override.
