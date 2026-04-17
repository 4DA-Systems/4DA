# Terminal Coordination

## Protocol
1. **Before editing**: Read this file. If your files are claimed by another terminal, STOP.
2. **Claim files**: Add your entry below with the files you'll touch.
3. **Commit Lock**: Set `**Commit Lock**: HELD` before committing. Only one terminal commits at a time.
4. **After committing**: Remove your entry and release the lock.
5. **Conflicts**: If two terminals touch the same file, the one that committed first wins. The other must rebase.

## Active Terminals

### T-INTEL-RECON (Phase 12 — Evidence Tab)
**Started:** 2026-04-17
**Prior RECON commits:** Phases 0-11 shipped.
**Scope:** Build the Evidence tab that replaces the old Momentum slot.
Three sections, fixed:
  1. This Week — honest one-line claim with real numbers
  2. Active Commitments — open Commitment Contracts + refutation status
  3. Recent Items — latest EvidenceItems from all lenses
No vanity metrics. No gauges. Every number informs an action.
**Files claimed:**
- `src/components/evidence/EvidenceView.tsx` (NEW)
- `src/components/ViewRouter.tsx` (EDIT — add evidence route)
- `src/components/ViewTabBar.tsx` (EDIT — add evidence tab)
- `src/store/types.ts` (EDIT — add 'evidence' to ViewId)
- `src/store/ui-slice.ts` (EDIT — add to tier views)
**Done when:** Evidence tab renders real data from commitment contracts
+ preemption + blind spots feeds. No empty-state pseudo-gauges. All
tests green.
**Commit Lock:** HELD at end of phase

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
