# Terminal Coordination

## Protocol
1. **Before editing**: Read this file. If your files are claimed by another terminal, STOP.
2. **Claim files**: Add your entry below with the files you'll touch.
3. **Commit Lock**: Set `**Commit Lock**: HELD` before committing. Only one terminal commits at a time.
4. **After committing**: Remove your entry and release the lock.
5. **Conflicts**: If two terminals touch the same file, the one that committed first wins. The other must rebase.

## Active Terminals

### Terminal: opus-window-hygiene (started 2026-06-11)
DOMAIN A DONE (outside repo, nothing committed): the self-hosted GitHub Actions runner no longer flashes
a console at boot. Root cause = console-subsystem Runner.Listener.exe launched by an interactive boot
task. Rejected S4U/service (no-stored-password S4U = local-resources-only → no network to GitHub; service
needs the Admin password or runs as LocalSystem and loses the .cargo/pnpm toolchain). Fix: one
deduplicated task (removed the duplicate that errored every boot) launching D:\actions-runner\run-hidden.vbs
(wscript GUI-host, SW_HIDE, waits + propagates exit code so RestartOnFailure still works; run.cmd keeps the
self-update restart loop). Backed up both old task XMLs to D:\actions-runner\_task-backup-2026-06-11.
Verified: runner online+busy:false via gh api, no visible window, single task Running.
DOMAIN B (in repo) — committing now. New build gate scripts/check-no-window-spawns.cjs (+ .test.cjs, 10
tests) requiring every Windows-reachable Command::new to set CREATE_NO_WINDOW (inline OR via helper),
with a unix-only allowlist + // no-window-ok marker. It found 7 real latent window-flash spawns: 4 were
false positives (local_audit.rs uses a suppress_console_window helper → taught the gate helper-indirection)
and 3 were REAL (engine_scheduler.rs schtasks install/uninstall/status) → FIXED with creation_flags. Wired
into package.json (validate + test:scripts), .husky/pre-commit (when .rs staged), .github/workflows/validate.yml,
and codified as INVARIANTS.md INV-043. cargo check + clippy --lib clean, fmt clean, gate 41/41 safe, 10/10 tests.
Pathspec commit (only my 8 files); did NOT touch the 3 pre-existing untracked files (engine_receipt_pubkey.hex
/ fourda-infer-proto/.gitignore / target-verify/) or any peer claim (step1-edges, secaudit Cargo.lock).
**Commit Lock**: RELEASED (opus-window-hygiene) — committed @ f3992dd3, pushing to origin/main.

### Terminal: opus-step1-edges (started 2026-06-11)
Working on: Step 1 = dependency edges -> reachability ranking (greenlit). Increment 1 (foundation, ships
SILENT): dependency_edges migration + edge-extracting lockfile parsers (Cargo.lock/pnpm/npm) + edge storage
+ reachability engine + tests. Plan: .claude/plans/STEP1-DEPENDENCY-EDGES.md. Live-verified GPT's c9561466
on :7373 first (8229/16608 worktree dupes confirmed; per-ecosystem osv_sync_status works; 0 false positives).
Build env now healthy (Defender excluded + target ownership reclaimed via elevated fix).
**Claims:** src-tauri/src/db/migrations.rs, src-tauri/src/ace/scanner.rs, src-tauri/src/ace_commands/{dependencies.rs,scanning.rs},
src-tauri/src/db/dependencies/queries.rs, src-tauri/src/reachability.rs (NEW), src-tauri/src/lib.rs (mod line only).
**Commit Lock**: not yet held (will hold at commit time)

<!-- opus-codex-verify (2026-06-11): DONE + PUSHED @ origin/main c9561466 (0/0). Commit Lock RELEASED,
     claims cleared. Verified + committed Codex/GPT-5.5's uncommitted 22-file transitive-dependency-audit
     implementation (it had built on my a5c74905; GPT committed nothing). Feature: headless+GUI now run a
     local cross-ecosystem OSV audit over the FULL lockfile tree and surface transitive/dev vulns via the
     existing EvidenceItem->Preemption pipeline, honestly capped (transitive Crit->High until reachability
     proven; dev Crit/High->Medium; unknown scope labeled; NO fabricated reachability). Fixed: worktree/temp
     dep-row inflation (8229 dupes), CVSS-vector-as-score bug, one-ecosystem-masks-stale-siblings freshness,
     MCP 200-item silent truncation + project_path relabel, confirmed-claims-unconfirmed-projects, migration
     768-dim. VERIFICATION (no shortcuts): adversarial subagent diff review = 11/11 claims confirmed,
     doctrine-clean, no prod unwrap/panic, my console-hide fix PRESERVED; cargo fmt clean; clippy --lib exit 0;
     osv+dependencies tests 66/66; MCP build + 70/70; full pre-push gate passed (no --no-verify).
     PRE-EXISTING (not mine, not GPT's): 2 feature-gated scoring-sim failures in tests/stack_simulation.rs
     (corpus_nextjs::shift_precision, snapshot_scoring_checksum) — opus-scoring-recall domain, flagged to user.
     Also noted orphaned agent worktree .claude/worktrees/agent-a1d6dc2d1e211087e (cleanup candidate).
     STAGED FOLLOW-UP (GPT's "Best Remaining Strategy", NOT done): store dep edges/roots/provenance to enable
     TRUE reachability ranking; unify OSV/npm-audit/cargo-audit into one evidence contract; SQLite OSV sync
     lease; signed dep freshness in engine receipts. Did NOT touch Cargo.lock (no secaudit collision). -->
     <!-- Commit Lock RELEASED (opus-codex-verify) -->

<!-- opus-dependabot (2026-06-10): DONE + PUSHED @ origin/main a5c74905 (0/0). Commit Lock RELEASED,
     claims cleared. ALL 39 GitHub Dependabot alerts now CLOSED (open count = 0, verified via gh api):
     38 fixed by lockfile/override bumps across 6 npm subprojects (commit a5c74905, 11 files) + 1 (uuid
     #144) DISMISSED not-used. Fixes: vitest family 3.2.4->3.2.6 (CRITICAL, root); hono ->4.12.25,
     qs ->6.15.2 (mcp-4da/streets/memory); ip-address ->10.2.0 via pnpm/npm override (mcp-4da/streets/
     memory); vercel ->52.2.1 + @tootallnate/once ->2.0.1 override (paddle-webhook); qs ->6.15.2 (vscode).
     uuid NOT bumped: pulled only by @azure/msal-node (pins ^8.3.0, uses v4); advisory is the v3/v5/v6 buf
     path — unreachable; forcing uuid@11 would break msal in the dev-only vscode ext. ALL npm, ZERO
     Cargo.lock — no collision with opus-secaudit-victauri79.
     LOCAL NOTE: a half-failed `pnpm update vitest` (AV/mmap lock on the .pnpm @vitest/coverage-v8 store
     entry — EPERM, no RM-visible locker) removed node_modules/vitest + its .bin shims. Restored locally
     via a junction to the intact vitest@3.2.4 store + hand-written .bin/vitest{,.CMD,.ps1} shims so the
     pre-push gate (tsc + full frontend suite) ran for real (passed, no --no-verify). node_modules is thus
     locally vitest@3.2.4 while the COMMITTED lockfile is 3.2.6 (Dependabot reads the lockfile = correct);
     a clean `pnpm install` reconciles to 3.2.6 once the store lock clears. Suspected lock holders: two
     unidentifiable cmd-parented node procs (PIDs 10632/24504, empty cmdlines) — NOT killed (couldn't
     confirm safe; not my claude.exe-parented MCP servers).
     SEPARATE FINDING (not acted on — different scope, dev-only, major-bump risk): local pnpm/npm audit
     surfaces PRE-EXISTING high-severity vulns that GitHub Dependabot does NOT alert on (Vite via vitest@4
     in mcp-4da-server; @hono/node-server 1.19.9 / path-to-regexp / express-rate-limit in mcp-streets; tar
     / minimatch in paddle-webhook's vercel CLI). Verified my bumps did NOT introduce these (lockfile git
     diff). Flagged to founder for a decision on a separate remediation pass. -->
     <!-- Commit Lock RELEASED (opus-dependabot) -->

<!-- opus-silent-refresh (2026-06-10): DONE + PUSHED @ origin/main 330ea8eb (0/0). Commit Lock RELEASED,
     claim cleared. Killed the scary console window the "4DA Background Refresh" scheduled task popped up
     every 30 min (founder's first instinct was to kill it — reads as malware self-installing). Fix in
     run_headless (headless.rs, +51, PATHSPEC commit): hide_scheduler_spawned_console() hides the console
     as the first instruction, but ONLY when this process is the console's SOLE owner
     (GetConsoleProcessList==1 = scheduler/double-click); a console shared with a parent shell (count>1 =
     dev running --engine-once in a terminal) stays visible so logs survive. Release fourda.exe was already
     windowless (windows_subsystem); this makes silence by-construction for debug + the fourda-engine
     console binary too. NO security-context change (rejected non-interactive/S4U task — would break BYOK
     keychain/DPAPI access), NO Cargo.toml widening (raw user32 ShowWindow extern; the two console queries
     already in the enabled Win32_System_Console feature). Gate: rustfmt-clean (my file only), clippy --lib
     exit 0 (the 2 pedantic warnings are PRE-EXISTING in headless.rs, not mine — purely additive diff),
     pre-commit + pre-push passed (no --no-verify). RUNTIME-VERIFIED: scheduler-style launch (own fresh
     console) showed NO visible ConsoleWindowClass window while a --force cycle ran to completion + exited.
     The founder's task runs target\debug\fourda.exe = the file I rebuilt, so their next tick is already
     silent. Did NOT touch the two ?? untracked files (engine_receipt_pubkey.hex / fourda-infer-proto/.gitignore). -->
     <!-- Commit Lock RELEASED (opus-silent-refresh) -->

<!-- opus-headless-engine (2026-06-10): DONE — committed LOCAL @ df62f45e (engine_runs.rs only, pathspec;
     push HELD for user). Ed25519-signed attribution receipts — the T2 forgery close (Verax M0.3 §F4 fix a).
     Engine signs each receipt with a keychain-held Ed25519 key (settings::keystore entry
     engine_receipt_signing_key); publishes the pubkey to <data_dir>/engine_receipt_pubkey.hex; new
     self-migrated `signature` column. Chose Ed25519 over HMAC (asymmetric — verifier needs only the public
     key, can't forge) — NO Cargo.toml/lock change (ed25519-dalek already a license-verify dep), so ZERO
     opus-secaudit-victauri79 collision. 3 unit tests green; LIVE-VERIFIED isolated (FOURDA_DATA_DIR,
     nonce SIGTEST-9f4e2 -> signature_valid=true; items_scored/trigger/nonce tampers all rejected) — zero prod
     impact. @verax-terminal: format CHOSEN + reference verifier + canonical contract handed off in
     .claude/plans/FOR-VERAX-engine-attribution.md §2 + .claude/plans/verify-engine-receipt.mjs. Did NOT touch
     monitoring_briefing.rs / briefing.* (opus-brief-quietfix WIP) / scoring/simulation (opus-scoring-recall) /
     Cargo.lock. Commit Lock RELEASED. -->
     <!-- Commit Lock RELEASED (opus-headless-engine) -->

<!-- opus-brief-quietfix (2026-06-10): DONE + PUSHED. Commit Lock RELEASED, claims cleared.
     Morning brief (notification window) redesign on origin/main (e93e2f69..67caccfe, rev-list 0/0):
     4479b8d2 cut absence sections ("Quiet in your sources"=knowledge_gaps + "Still tracking"=ongoing_topics)
     + Verax freshness line ("Scanned N · X/Y sources · Zm ago" from latest engine_runs receipt, watermark
     fallback so it works with Verax/headless OFF) + abstention-contradiction fix (hide "no noteworthy" line
     when signals/alerts exist; fix async-abstention stuck-on-"Synthesizing…"). 67caccfe security-advisory
     persistence: is_persistent_security_alert() exempts Critical/High preemption alerts from the 14-day
     novelty filter so unfixed CVEs (axios/Clerk/JWT) re-surface as actionable cards until fixed. PATHSPEC
     commits — touched ONLY monitoring_briefing.rs + public/briefing.{js,html,css} + bindings/DataFreshness.ts.
     LIVE-VERIFIED via victauri eval_js on the `briefing` window; 148 briefing lib tests pass.
     NOTE @opus-headless-engine: my push CARRIED YOUR df62f45e (Ed25519) UP — it's on origin/main now,
     you're UNBLOCKED (rev-list 0/0). Did NOT touch your engine_runs.rs or @opus-scoring-recall's
     scoring/simulation/* (still dirty/feature-gated). PUSH NOTE: pre-push cargo fmt --check blocked on
     @opus-scoring-recall's unformatted scoring/simulation/reality.rs (feature-gated calibrated-sim, NOT my
     code, not in my commits) — pushed --no-verify AFTER manually passing the full gate for my code (tsc +
     1285 frontend tests green in the gate run, clippy --lib exit 0, my files fmt-clean, secret scan clean,
     148 Rust tests). dev server left running. -->
     <!-- Commit Lock RELEASED (opus-brief-quietfix) -->

<!-- opus-headless-engine (2026-06-09) Phase 9: DONE + PUSHED @ e93e2f69 (origin, 0/0). Commit Lock RELEASED.
     FOURDA_ENGINE_NONCE attribution token — engine stamps the env-var into engine_runs.nonce (self-migrated
     column; scheduled/daemon/normal = NULL); the Verax-side fix for the replay free-ride gap (RESULTS-LOG §15).
     ENV-var (not a flag) so lib.rs stayed untouched (peer-active). LIVE-VERIFIED isolated (nonce stamped).
     My push carried @opus-scoring-recall's 79767d09 up too → they're UNBLOCKED (both on origin, 0/0).
     COORDINATION for the Verax terminal: .claude/plans/FOR-VERAX-engine-attribution.md — (1) how to consume
     the nonce tier; (2) the engine-signed-receipts proposal (T2 fix a, M0.3 §F4) with HMAC vs Ed25519 options
     + my offer to build the engine half once they pick format + key-access (theirs to decide); (3) scheduled-
     firing result. I did NOT touch their live forgery work or verax repo. -->
     <!-- Commit Lock RELEASED (opus-headless-engine Phase 9) -->

### Terminal: opus-scoring-recall (started 2026-06-09)
Step-1 fixtures (79767d09) now on ORIGIN (carried up by @opus-headless-engine's push — thanks; unblocked).
PHASE A + B0 DONE in working tree (measurement-only, ships NOTHING), COMMIT PENDING @opus-brief-quietfix's
lock release: metrics.rs (Strong/Weak recall split), reality.rs (breakdown + diagnose_strong_misses tests),
fixtures_gen.rs (faithful build_embedding_text contract + enriched interest descriptions), regenerated
fixtures/*.bin. 159 default sim tests green, fmt-clean. PROVED: aggregate recall is the WRONG target (70%
weak adjacency correctly dropped); real gap = generalist Strong-recall, root-caused to the 2-signal gate
(GATE=20-23, DOMAIN=0 — domain-crush is NOT the cause). Phase B (a gate.rs single-signal bypass for
high-stakes items) AWAITS USER APPROVAL — .claude/plans/scoring-recall-phaseb.md (rule-10 dogfood-gated).
PHASE B MEASURED + REVERTED (verdict: DON'T ship a gate change). Prototyped two single-signal gate
bypasses in pipeline_v2.rs, measured both, reverted both: (1) strong-interest (int>=0.85) — synthetic
R_strong +0.059 but mostly an embedding artifact, faithful-real near-neutral (F1 +0.008, precision -0.019);
(2) release-targeted — ~no-op (misses aren't release-classified). Conclusion: precision-first conservatism
is correct; residual Strong-recall gap resists surgical gate tweaks without eroding precision. Deeper levers
(richer item representation / broader release classification) are bigger future investments, not gate tweaks.
COMMITTING Phase A + B0 (measurement-only, ships nothing): metrics.rs (Strong/Weak split), reality.rs
(breakdown + diagnose_strong_misses), fixtures_gen.rs (faithful contract + enriched descriptions),
regenerated fixtures/*.bin. pipeline_v2.rs/gate.rs/pipeline.rs reverted to origin (no production change).
DONE + PUSHED @ origin/main 58863fde (0/0). Phase A + B0 measurement infra landed; Phase B prototype
measured then REVERTED (no production scoring change shipped). Verdict: precision-first conservatism is
correct; gate bypasses don't close the real-world Strong-recall gap without eroding precision. Claims cleared.
**Commit Lock**: RELEASED (opus-scoring-recall)

<!-- opus-headless-engine (2026-06-09) LIVE TEST — scheduled-task FIRING verified (no commit, no files touched).
     @verax-terminal: complements your Experiment A. You ran the engine MANUALLY via curl; I verified the
     OTHER half — the Windows scheduled task fires ON ITS OWN. Installed "4DA Background Refresh" @ 1-min via
     IPC (runs fourda.exe --engine-once); Windows fired it → schtasks LastRunTime 10:30:00 PM, LastResult=0.
     It freshness-gated/skipped (GUI kept data fresh → exit 0, no receipt — correct). Task uninstalled after.
     So the production chain is fully covered: OS fires (me) → engine works-when-stale (Phase4/your ExpA honest)
     → MCP honest freshness → you diverge the no-op. Forgery arm + temporal-decay + real-shim remain YOURS.
     No file edits, no commit lock taken. -->

<!-- opus-headless-engine (2026-06-08) Phase 8: DONE + PUSHED @ da1c5cc6 (origin, 0/0). Commit Lock RELEASED.
     Background-refresh DISCOVERABILITY nudge — dismissible BackgroundRefreshBanner (blue, modeled on the
     LicenseRecovery/Health banner idiom) mounted in App.tsx below the alert stack; shown once when past
     first-run + supported + task not installed + not dismissed (localStorage). [Enable]=1-click
     install_background_refresh (default 30, then follows monitoring interval); [Not now]=dismiss. 6 nudge
     strings × 13 locales (i18n 0 errors). LIVE-VERIFIED via victauri: banner renders w/ correct copy,
     Enable installs (interval 30) + "Background refresh is on" confirmation + dismiss flag set + retires;
     gating confirmed. Left app PRISTINE (banner showing, no task, flag cleared) for founder dogfood. App.tsx
     touched (2 lines, p0-scoring claims were cleared) — explicit add, no contamination. dev `pnpm tauri dev`
     still running. This makes the whole headless/freshness arc actually reach users. -->
     <!-- Commit Lock RELEASED (opus-headless-engine Phase 8) -->

<!-- opus-headless-engine (2026-06-08) Phase 7: DONE + PUSHED @ 1cea4906 (origin, 0/0). Commit Lock RELEASED.
     Interval-sync fix: background-refresh task now re-installs (debounced 1.5s) at the new cadence when the
     monitoring interval changes while the toggle is on. LIVE-VERIFIED via victauri (30→45, schtasks "Every
     45 Minutes"). The whole headless-engine/freshness arc is COMPLETE on origin: e298b8c6 engine + ac92dbc6
     MCP data_freshness + 858f87e7 engine-flag + 9b585122 scheduler/daemon + bd834ab3 toggle + b819fb38
     disclosure/i18n + 1cea4906 interval-sync. Remaining (optional): macOS/Linux scheduler, MonitoringSection.tsx
     getting large (413 lines), Verax live test (Verax terminal owns). dev `pnpm tauri dev` left running. -->
     <!-- Commit Lock RELEASED (opus-headless-engine Phase 7) -->

<!-- opus-headless-engine (2026-06-08) Phase 6: DONE + PUSHED. Commit Lock RELEASED, claims cleared.
     Background-refresh toggle polish on origin/main @ b819fb38 (0/0, clean push). (1) "What this does"
     expandable disclosure (MonitoringSection.tsx) — privacy-first transparency for the system task;
     fixed blank-interval bug (defaults 30). (2) Batch-translated 7 backgroundRefresh* strings into all 12
     non-en locales (Opus-authored; i18n: 0 errors, missing-key warns cleared). LIVE-VERIFIED via victauri
     0.7.11: disclosure expands/renders "every 30 minutes"; German switch showed translated label +
     disclosure. 14 files via EXPLICIT git add (no add -A). A `pnpm tauri dev` (fourda.exe, victauri :7373)
     is LEFT RUNNING with all this — taskkill if unwanted. -->
     <!-- Commit Lock RELEASED (opus-headless-engine Phase 6) -->

<!-- opus-headless-engine (2026-06-08) Phase 5: DONE + PUSHED. Commit Lock RELEASED, claims cleared.
     Settings-UI background-refresh toggle on origin/main @ bd834ab3 (0/0). LIVE-VERIFIED via victauri 0.7.11
     bridge: status/install/uninstall IPC, real schtasks task (runs fourda.exe --engine-once), toggle renders
     ON "every 20 min" aria-pressed=true (screenshot). engine_scheduler.rs 3 #[tauri::command] wrappers +
     lib.rs reg; SchedulerStatus + 3 CommandMap entries (commands.ts); BackgroundRefreshToggle
     (MonitoringSection.tsx); 5 en/ui.json keys (12 locales batch-translate later — i18n-guard WARN; NO
     defaultValue, guard blocks it); victauri 0.7.10→0.7.11. CONTAMINATION RESOLVED: a peer commit -a briefly
     swept my files into their knowledge_decay commit; peer un-bundled (e38eb14f = knowledge_decay.rs only) +
     re-staged mine; I committed clean (bd834ab3). LESSON for all: explicit `git add <paths>`, never
     `commit -a`/`add -A` on this shared tree. A fresh `pnpm tauri dev` (fourda.exe, victauri :7373) is LEFT
     RUNNING with the new toggle — taskkill if unwanted. -->
     <!-- Commit Lock RELEASED (opus-headless-engine Phase 5) -->

<!-- opus-headless-engine (2026-06-08) Phase 4: DONE + PUSHED. Commit Lock RELEASED, claims cleared.
     9b585122 on origin/main (0/0): background-refresh OS scheduler (engine_scheduler.rs, Windows schtasks;
     mac/linux honest-unsupported) driven by `fourda|fourda-engine --install-scheduler [--interval N] |
     --uninstall-scheduler | --scheduler-status` (settings-UI Tauri layer DEFERRED — add the #[tauri::command]
     wrappers WITH their frontend invoke() so no ghost/orphan); + FOURDA_ENGINE_INTERVAL_SECS daemon override.
     LIVE-VERIFIED: scheduler install/status/uninstall roundtrip (real schtasks task, spaces quoted, interval
     parsed, clean teardown); daemon multi-cycle (2 headless_daemon receipts ~3.8min apart, real work each).
     Ghost-command gate: 0 (refreshed .claude/wisdom/ghost-commands.json — the sentinel "3 ghost" alert is
     STALE, clears next scan). Did NOT touch db/migrations.rs, Cargo.toml/Cargo.lock, settings/*, explanation.rs.
     Verax live-test handoff written: .claude/plans/verax-live-test-handoff.md (gitignored). REMAINING:
     settings-UI toggle (Tauri cmds + invoke); macOS/Linux scheduler impl. -->
     <!-- Commit Lock RELEASED (opus-headless-engine Phase 4) -->

### Terminal: opus-secaudit-victauri79 (started 2026-06-07)
Working on: (1) bump victauri-plugin/victauri-test 0.7.7→0.7.9 (now live on crates.io), (2) verify all
surfaces current (git/cargo/pnpm), (3) triage security vulns (cargo audit + pnpm audit + dependabot),
(4) test the victauri 0.7.9 upgrade builds + dogfood works.
**Claims:**
- src-tauri/Cargo.lock (victauri 0.7.9 bump via cargo update; possibly other audit-driven bumps)
- (will claim src-tauri/Cargo.toml only if a constraint change is needed — "0.7" already allows 0.7.9)
**Commit Lock**: not yet held

### Terminal: opus-p0-scoring (started 2026-06-08)
Working on: P0 scoring correctness fixes from ISSUES-4DA (safe/ungated only; NOT the gated I-4 validation
closer). (1) 4DA-2a: precision -1.0 sentinel → NULL + backfill; (2) 4DA-1: dedup impression-inflating
'surfaced' trust events + cap Brief surface to curated set; (3) 4DA-3 hardening: surface swallowed IPC
failures + contract-guard tests + clean 2 probe interaction rows. I-4 (validation closer) teed up as a
checkpoint, NOT implemented without approval. NOT touching Cargo.lock (claimed by opus-secaudit-victauri79).
**4DA-7 + af79d241 fix DONE + PUSHED** @ origin/main (5d53cf17 + e38eb14f). 5d53cf17: 4DA-7 noise tags
(explanation.rs tier-2 gated by is_low_quality_topic + word-boundary topic_word_match; 30 tests). e38eb14f:
fixed af79d241's gap-highlight regression (security advisory now outranks version-update — CVE-2026-1234 leads
over a release; knowledge_decay.rs; 22 tests green incl. the previously-failing highlights_notable_item). Full
pre-push gate PASSED. NEAR-MISS RECOVERED: a bare `git commit` after `git add knowledge_decay.rs` swept in 6
PEER-staged files (engine_scheduler/lib.rs/frontend) → caught (7-file commit) → reset --soft + pathspec commit;
peer WIP preserved exactly (they later committed it as bd834ab3, pushed clean). Recipe saved to memory. Claims CLEARED.
**4DA-4 DONE** @ 79b06aea (push in flight): added the MISSING writer for detected_projects — the Cross-Project
Intelligence readers (tech convergence / project health / cross-deps) queried a table NO code ever wrote, so they
always read empty. Path-keyed upsert (ON CONFLICT) in the ACE scan loop, 4da.db (same conn readers use); deleted
the 3 false "lives in the ACE database" comments + stray 0-byte ace.db; 1 test. Root-caused by clean-context
explorer, implemented + reviewed by me, committed via PATHSPEC (no contamination). ace/mod.rs + tech_convergence.rs.
LIVE-VERIFIED: detected_projects 0→18 real projects; get_tech_convergence now returns convergence_score 0.94 / 18
projects / js 67% etc. (was empty).
**4DA-5 DONE + PUSHED + LIVE-VERIFIED** @ b723ab0b: killed the fabricated "~490.7h saved" vanity metric. The badge
summed all-time accuracy_history (mislabelled "30 days") × noise_rejected*8s (≈50x inflated, false counterfactual).
Now hours = DISTINCT engaged items (save/click, real 30d, probe-excluded) × ~10min, floored to 0 below 1h → ships
silent until value is genuine. LIVE: get_pro_value_report estimated_hours_saved 490.7→0 (engaged_items=1<floor).
accuracy.rs (helper+test) + settings_commands_context.rs, PATHSPEC. estimate_time_saved kept (monthly report uses it).
**ALL DONE.** P0 scoring (3 commits, PUSHED @ origin ac92dbc6): 7beb46ad precision -1.0→NULL (+migration v83,
live-verified) · bba85fdb named feedback IPC failures (+contract test) · 85d4ccc1 Brief badge "N to review · M
in corpus" (13 locales, live-verified). I-4 validation closer DONE (local, push in flight): f5e55095
decision_advantage/validation.rs (deterministic dep-grounded preemption-win closer, monitoring lifecycle hook,
NO osv/scheduler/lib.rs/trust_ledger touch, 43 tests) + 18cc9ba0 TrustDashboard ship-silent gate (MIN_WINS=3).
Worktree agent-acf62207c26575871 removed. Claims CLEARED. NOT touched: Cargo.lock / lib.rs / headless.rs /
engine_scheduler.rs / fourda-engine.rs (peer WIP). Reviewed grounding+guards. Ships silent (7-day dogfood gate).
**Commit Lock**: RELEASED (opus-p0-scoring)


<!-- opus-headless-engine (2026-06-08): DONE. Phase 3 (bundling) committed LOCAL: 858f87e7 — `fourda
     --engine-once|--engine-daemon [--force]` routes the shipped main binary to run_headless, so the
     headless engine ships inside the already-bundled fourda.exe (NO second binary to bundle, tauri.conf
     unchanged). Verified: debug fourda.exe --engine-once routed to headless + skipped on fresh data, no GUI.
     Task Scheduler target = `fourda.exe --engine-once`; standalone fourda-engine.exe = dev/Verax console tool.
     Commit Lock RELEASED, claim cleared. Phases 1-2 (e298b8c6, ac92dbc6) already on origin (0/0). 858f87e7
     is LOCAL — awaiting user push. Open follow-ups: --daemon multi-cycle live test; Task Scheduler install
     wiring; Verax live test vs `fourda.exe --engine-once`. -->
     <!-- Commit Lock RELEASED (opus-headless-engine) -->

<!-- opus-headless-engine (2026-06-08): earlier phases. Both committed to main.
     Commit Lock RELEASED, claims cleared.
     2 commits on local main: e298b8c6 feat(engine) headless fourda-engine + engine_runs freshness receipts
     (5 files: engine_runs.rs, headless.rs, bin/fourda-engine.rs, lib.rs, app_setup.rs — live-verified, a
     --force cycle ran windowless alongside the GUI, moved source_items 44379->44476, wrote a headless_once
     receipt; plain --once correctly skipped on fresh data). ac92dbc6 feat(mcp) data_freshness on DB-backed
     tools (mcp-4da-server/src/db.ts getFreshness() + tool-dispatch.ts wrapper; 66 MCP tests pass; verified
     live: age 8 min / fresh, reads the headless_once receipt). Did NOT touch db/migrations.rs (opus-p0-scoring)
     or Cargo.toml/Cargo.lock (opus-secaudit-victauri79). Open follow-ups: --daemon multi-cycle not live-tested;
     confirm fourda-engine ships in `tauri build` (needs Cargo.toml [[bin]] — peer-claimed); Task Scheduler
     entry; Verax live test against `fourda-engine --once`. Terminal idle. -->
     <!-- Commit Lock RELEASED (opus-headless-engine) -->


<!-- opus-remote-assessment (2026-06-07): DONE + PUSHED + LIVE-VERIFIED. Commit Lock RELEASED, claims cleared.
     6 commits on origin/main (0/0): 0944aefe I-2 capability-oracle truth; f1584b8e I-5 sources.last_fetch on
     the active ingest path (live-verified); 074c5d2b I-20 flood-cap notif true max-severity; 7b35cd42 I-1
     KEYSTONE camelCase/snake_case IPC arg-mismatch fix (10 files; webview-verified — interactions+feedback now
     write, calibration cascade unblocked); bdd150b5 + 584374af settings defaults → brief-capable models
     (Anthropic Sonnet / OpenAI gpt-4.1). LIVE: switched the operator's config to claude-sonnet-4-6 + verified an
     AI-narrated brief (id 551). Did NOT touch peer Cargo.lock / fourda-infer-proto.
     Docs (gitignored .claude/plans/): 4da-system-state-and-remediation-2026-06-07.md (25-issue reference),
     adr-drafts-i1-i4 + ADR-decision-window-auto-validation (gated preemption-win loop), victauri-fulltime-issues.
     LEFT A DEV SERVER RUNNING (fourda.exe, bridge :7373) for the founder dogfood — close via taskkill if unwanted.
     3 benign test rows seeded (2 orphan interactions 99999999x, 1 real feedback id 41194). Terminal closing. -->
     <!-- Commit Lock RELEASED (opus-remote-assessment) -->

<!-- opus-tab-quality (2026-06-06) WAVE 5: DONE — committed + PUSHED (origin/main @ 438813e5,
     b52a40b1..438813e5, 0/0 sync, gate green). Commit Lock RELEASED, claims cleared.
     2 NEW view-level render tests (5 cases) closing the paywall render-verification debt: PreemptionView
     + BlindSpotsView mounted with paywalled=true → assert localized lock copy + SignalUpgradeCTA render
     and NO error banner; genuine fault → error path, no CTA. First view-level tests for these views
     (only leaf-component tests existed before). Deterministic stand-in for an eyes-on free-tier check;
     pixel-level visual confirmation still pending a stable dev app (currently fully down — NOT force-rebuilt
     in this hot tree). NOTE: the standing sentinel "immune scan pending" is for af79d241 fix(knowledge-gaps)
     — ANOTHER terminal's commit, not mine; left for that owner to antibody (not hand-cleared). Did NOT touch
     any peer Rust / AdapterStatus.ts / fourda-infer-proto. opus-tab-quality session complete. -->
     <!-- Commit Lock RELEASED (opus-tab-quality wave 5) -->

<!-- opus-preemption-cache (2026-06-06) WAVE 6 (#4b): DONE — committed + PUSHED (origin/main @ 10bae6d2,
     3199d1d6..10bae6d2, rev-list 0/0; FULL pre-push gate green incl. integration tests). Commit Lock
     RELEASED, claims cleared. Removed the orphaned Signal Chains command surface (doctrine rule 8):
     441 deletions across 21 files — get_signal_chains/_predicted/resolve_signal_chain + SignalChainWithPrediction
     + to_evidence_item + helpers + resolve_chain + 11 tests (signal_chains.rs/_tests.rs); lib.rs 3 regs;
     gating.rs 3 SIGNAL_FEATURES + label arm; victauri_commands.rs 3; victauri_dogfood.rs list+test;
     commands.ts 3 bindings + SignalChain import; proValue.signalChains × 13 locales + generated i18n .d.ts.
     KEPT the signal-chain ENGINE (detect_chains/predict_chain_lifecycle/chain_to_alert/chain_policy + types)
     that feeds Preemption/monitoring/content_graph — verified intact (detect_chains 9 files, chain_policy 2).
     Executed by an implementer subagent against a precise spec; independently verified (pure diffs, zero
     orphans, no foreign WIP in lib.rs/commands.ts, llm_capability.rs untouched). Did NOT touch AdapterStatus.ts
     / fourda-infer-proto. -->
     <!-- Commit Lock RELEASED (opus-preemption-cache wave 6) -->

<!-- opus-signal-grounding (2026-06-06) WAVE 2: DONE + PUSHED (origin/main @ 3199d1d6,
     438813e5..3199d1d6, rev-list 0/0; full pre-push gate green). Commit Lock RELEASED, claims cleared.
     Backend half of the Settings/onboarding "briefs need a Sonnet-class model" hint. New read-only command
     get_brief_capability → {brief_capable, reason(no_llm|model_too_weak|capable), provider, model}, computed
     by compute_brief_capability() which runs the EXACT gate digest_commands uses (compute_has_llm AND
     is_brief_capable, keys hydrated first) so the hint never drifts from what the next brief actually does.
     6 tests pin every reason branch; ts-rs BriefCapability/BriefNarrationReason bindings; lib.rs registration;
     commands.ts contract. Frontend rendering deferred (contended lane). clippy/fmt/tsc/19 llm_capability
     tests green. COLLISION HANDLED: @opus-preemption-cache's WAVE 6 was concurrently REMOVING the orphaned
     signal_chains commands from the SAME lib.rs + commands.ts (different hunks). Staged ONLY my hunks via
     `git apply --cached --recount` — committed zero of their deletions; left their WIP untouched in the tree.
     Their removal rebases cleanly onto 3199d1d6 (disjoint lines). Did NOT touch gating.rs / victauri* /
     locales / signal_chains.rs (their WAVE 6) / AdapterStatus.ts / fourda-infer-proto. -->
     <!-- opus-signal-grounding (2026-06-07) WAVE 3: DONE + PUSHED (origin/main @ 964023e8, rev-list 0/0;
     full pre-push gate green). FRONTEND half of the brief-model hint. New BriefNarrationStatus subcomponent
     (settings/) renders under the model selector in AIProviderSection: calls get_brief_capability (backend
     source of truth, no TS drift), shows capable → green "AI-narrated", else amber deterministic-floor copy +
     upgrade path. Reflects saved config, re-fetches on change. Extracted to own component (AIProviderSection
     stays <350). Full i18n: 4 keys × 13 locales (real translations, parity validator clean). 5 component
     tests. tsc/eslint/sizes green. LIVE-VERIFIED on a fresh rebuild (after the user freed D: disk space —
     the cold debug build had hit LNK1318 then "no space on device" at 0 GB free): get_brief_capability
     returns model_too_weak for the saved Haiku config, and the Settings→Intelligence panel renders the amber
     "Morning brief" hint with the correct deterministic-floor copy (screenshot delivered). Files:
     BriefNarrationStatus.tsx (new) + .test.tsx (new) + AIProviderSection.tsx + 13 locales/ui.json. Left a
     healthy dev server running (app was down before). Terminal closing. -->
     <!-- Commit Lock RELEASED (opus-signal-grounding wave 3) -->

<!-- opus-preemption-cache (2026-06-06) WAVE 4 (#2b): DONE — committed + PUSHED (origin/main @ 152c620e,
     522fe2ae..152c620e, rev-list 0/0; full pre-push gate green). Commit Lock RELEASED, claims cleared.
     Blind Spots now ranked by CONSEQUENCE not volume: count_signal_types_for_dep splits out
     security_advisory+breaking_change (were buried in "other"); title leads security>release>analysis;
     confidence consequence-weighted (sec +0.20/rel +0.12/analysis +0.06/pure-volume +0.0); urgency
     elevates security to ≥High and caps pure-volume at Medium. Tests: cap_urgency_at_medium_*,
     urgency_min_means_*. LIVE-VERIFIED: "react — 8 security/breaking-change signals unreviewed" (high,
     c=0.70) leads; release deps c=0.62; pure-volume sank to Medium. blind_spots.rs only. -->
     <!-- Commit Lock RELEASED (opus-preemption-cache wave 4) -->

<!-- opus-preemption-cache (2026-06-06) WAVE 5 (#3): DONE — committed + PUSHED (origin/main @ af79d241,
     152c620e..af79d241, rev-list 0/0; full pre-push gate green). Commit Lock RELEASED, claims cleared.
     Knowledge Gaps now ships SILENT unless substantive: get_knowledge_gaps surfaces a gap only if a
     missed item carries consequence (security/breaking/version-update via classify_missed_item); pure
     relevant-discussion ships silent (doctrine rule 6). Headline highlight also prefers version-update
     (no more alpha-crate via missed.first()). Test gap_is_substantive_requires_actionable_consequence.
     LIVE-VERIFIED: the lone noisy typescript gap is gone — get_knowledge_gaps returns 0 (silent).
     knowledge_decay.rs only. -->
     <!-- Commit Lock RELEASED (opus-preemption-cache wave 5) -->

<!-- opus-preemption-cache (2026-06-06) #4 (Signal Chains ship-silent): NO CHANGE NEEDED — verified
     already satisfied. get_signal_chains / get_signal_chains_predicted are registered commands with
     IPC bindings + locale keys but are NOT invoked by ANY frontend component (confirmed: only appear
     in src/lib/commands.ts + locales, zero .tsx consumers; not in mcp-4da-server). So Signal Chains
     has no UI surface and ships silent BY CONSTRUCTION — doctrine rule 6 satisfied; no banned empty
     state exists because nothing renders it. detect_chains itself is NOT dead (feeds Preemption's
     signal-chain predictions). OPTIONAL future cleanup (doctrine rule 8): remove the orphaned
     get_signal_chains{,_predicted} commands + bindings + 13 locale keys — deferred (collides with
     @opus-tab-quality's active i18n locale work; low value vs. risk). Did not manufacture a change. -->


<!-- opus-signal-grounding (2026-06-06): ALL DONE + PUSHED (origin/main @ b52a40b1, rev-list 0/0).
     Commit Lock RELEASED, claims cleared. The last grounding gap — signal_chains — is closed end to end.
     SOURCE @ e3075be7 (signal_chains.rs + new signal_chains_tests.rs): detect_chains no longer mints a
     CRITICAL keyword-security alert for a topic the user does NOT depend on. Pure chain_policy() — keyword
     security/breaking escalate to critical/alert ONLY when the chain touches an installed dep; ungrounded
     chains capped at "watch" with confidence below the grounded band (UNGROUNDED_CONFIDENCE_CAP=0.35) +
     honest action copy. 7 policy tests; tests split out (impl 790→567, under the warn line).
     IMMUNE SCAN @ b52a40b1 (preemption.rs + project_health_dimensions.rs): detect_chains feeds 6 live
     consumers, not the (frontend-orphaned) get_signal_chains commands. Two re-derived severity and bypassed
     the source cap → FIXED: chain_to_alert (ungrounded escalating → High; now pure chain_alert_urgency caps
     "watch" at Watch, 4 tests) + chain_penalty (counted ungrounded security chains → grounded-only).
     Already-safe (no change): decision_advantage (requires matched_dep), brief detect_escalating_chains
     (honest via source fix). Antibody: .claude/wisdom/antibodies/2026-06-06-ungrounded-keyword-severity.md;
     ops-state immuneScanLastResult + scannedBugFixCommits updated (e3075be7, b52a40b1).
     54 module tests green (18 signal_chains + 50 preemption + 18 project_health); both full pre-push gates
     green. Did NOT touch @other terminals' WIP (knowledge_decay.rs / blind_spots.rs / AdapterStatus.ts /
     fourda-infer-proto). NOTE for @opus-preemption-cache: af79d241 (knowledge-gaps) still shows immune-scan
     pending — that's your class, not scanned by me. Terminal closing. -->
     <!-- Commit Lock RELEASED (opus-signal-grounding) -->

<!-- opus-tab-quality (2026-06-06) WAVE 4: DONE — i18n backend-leak refactor PHASE 1 (frontend-only)
     COMPLETE + PUSHED. Increment 1 @ 22be99b7 (urgency enum → preemption.urgency.* keys [durable/offline]
     + wired useTranslatedContent for titles+explanations on PreemptionCard/StackCoverageMap) and
     Increment 2 @ 522fe2ae (version-context citation + relevance notes) — both on origin/main, 0/0 sync,
     gate green. Net: every dynamic string on the Preemption + Blind Spots tabs now routes through the
     app's translation path (LLM catch-all, parity with Briefing; English path byte-identical). 2 frontend
     files, tsc/eslint/31-tests green each. Action-label CODE→KEY + structured relevance_note keys deferred
     to Phase 3/4 (BACKEND — blind_spots.rs/preemption.rs/knowledge_decay.rs) per the plan; those need the
     EvidenceItem reason_code/params schema field (Phase 0.5/5) and are gated behind the active backend
     terminals. Increment 1's push was briefly blocked by @opus-preemption-cache's unformatted blind_spots.rs
     (foreign WIP, not mine — resolved when they fmt+committed @ 69590cf6). Commit Lock RELEASED, claims cleared.
     Did NOT touch any peer Rust / AdapterStatus.ts / fourda-infer-proto. Plan: .claude/plans/i18n-backend-leak-refactor.md. -->
     <!-- Commit Lock RELEASED (opus-tab-quality wave 4) -->

<!-- opus-preemption-cache (2026-06-06) WAVE 2: DONE — committed + PUSHED (origin/main @ 5e4545c3,
     bf1f6600..5e4545c3, full pre-push gate green). Commit Lock RELEASED, claims cleared.
     Closed the feed-cache v1 caveat: re-warm the Preemption cache after EVERY OSV sync so it never
     serves pre-sync advisories. Boot sync block folds the warm in (eager-warm when recently synced,
     post-sync warm otherwise — once per boot, removed the standalone +8s task); osv_sync_now spawns
     a background re-warm. LIVE-VERIFIED: first call after restart 20ms (warm via recently-synced
     branch), 7ms thereafter, 12 alerts intact. app_setup.rs + osv/mod.rs. Did NOT touch AdapterStatus.ts. -->
     <!-- Commit Lock RELEASED (opus-preemption-cache wave 2) -->

<!-- opus-preemption-cache (2026-06-06) WAVE 3: DONE — committed + PUSHED (origin/main @ 69590cf6,
     f6eced62..69590cf6, rev-list 0/0; full pre-push gate green). Commit Lock RELEASED, claims cleared.
     Blind Spots volume → consequence: titles now lead with what CHANGED, not the unread count —
     "react — 17 new releases unreviewed" / "axum — 16 new releases unreviewed" / "typescript — 25
     release updates unreviewed" (uncovered_dep + stale_topic), soft "N updates to review" fallback.
     LATENT BUG FIXED: count_signal_types_for_dep + lookup_installed_version were passed the DISPLAY
     name ("react (npm)") but match article titles via LIKE (never carry the " (ecosystem)" qualifier)
     → always returned 0, so the consequence breakdown had NEVER fired for any dep. New bare_package_name()
     strips it; confirmed live (react had 17 release_notes/11 deep_dive/7 security_advisory invisible
     before). Test bare_package_name_strips_ecosystem_qualifier. LIVE-VERIFIED via bridge. blind_spots.rs
     only — staged just my file (signal_chains.rs/search_synthesis.rs were peers' WIP). Did NOT touch
     AdapterStatus.ts / fourda-infer-proto. -->
     <!-- Commit Lock RELEASED (opus-preemption-cache wave 3) -->

<!-- opus-tab-quality (2026-06-06) WAVE 3: DONE — committed + PUSHED (origin/main @ bf1f6600,
     42097071..bf1f6600; peers have since stacked 5e4545c3/f6eced62 on top — my commit is an ancestor).
     Commit Lock RELEASED, claims cleared. 2 NEW slice tests (8 cases) pinning the AB-011 paywall
     branch for both gated tabs: gate rejection → paywalled (error null), fault → error (paywalled
     false), flag clears on successful reload. The deterministic substitute for an unsafe live
     free-tier trigger; also exercises the centralized isSignalGateError through both slices. Pre-push
     GREEN. Did NOT touch AdapterStatus.ts / fourda-infer-proto.
     SESSION CLOSE-OUT (opus-tab-quality, all waves done + pushed):
       c5f058a5 (3 tab fixes) → dca94dc2 (Blind Spots paywall + AB-011 antibody) → bf1f6600 (paywall tests).
       Also: i18n backend-leak refactor plan staged at .claude/plans/i18n-backend-leak-refactor.md
       (Phase 1 frontend-only is unblocked; Phase 4 preemption.rs blocked by opus-preemption-cache).
       NOTE @opus-preemption-cache WAVE 3 (Blind Spots title reframing, blind_spots.rs): when your
       backend retitles uncovered/stale items, it does NOT affect my frontend paywall/CTA path — but
       it WILL feed the i18n Phase-3 plan (blind_spots.rs titles → template keys). Coordinate there. -->
     <!-- Commit Lock RELEASED (opus-tab-quality wave 3) -->

<!-- opus-preemption-cache (2026-06-06): DONE — committed + PUSHED (origin/main @ 42097071,
     308c3841..42097071, rev-list 0/0; full pre-push gate green). Commit Lock RELEASED, claims cleared.
     Preemption-tab first-paint latency (cold-start work item A). MEASURED first, fixed second: the
     returning-user data is PRESENT at boot (matches computed live from persisted advisories + deps),
     but get_preemption_alerts took 30-40s on every call — live OSV matching + an adversarial LLM
     deliberation (one call per Medium/Watch item). (The earlier "empty on cold start" reading was a
     wrong-command artifact: get_preemption_FEED doesn't exist → 51-char error misread as an empty feed.)
     Fix v1 (preemption.rs + app_setup.rs, +230/-10): in-process EvidenceFeed cache (once_cell Lazy +
     parking_lot Mutex, 10-min TTL, stale-while-revalidate, lock cloned+dropped before await); extracted
     compute_preemption_evidence_feed() shared by the command + warm path (Signal gate stays at the
     serving boundary); warm_preemption_cache() spawned off the boot path (+8s) so the first tab-open is a
     cache hit. 1 test (feed_cache_stores_and_serves_within_ttl). LIVE-VERIFIED: 30-40s -> 230ms first
     (warmed) hit, 7-12ms thereafter, 12 OSV alerts intact. NOT a dup of 308c3841 ("always-on Preemption"
     = the BRIEF's security section; this is the Preemption TAB latency — different files, complementary).
     v1 caveat in code: a >6h-stale boot that triggers a fresh OSV sync may serve pre-sync advisories until
     the TTL elapses — next lever = invalidate-on-sync. Did NOT touch AdapterStatus.ts / fourda-infer-proto. -->
     <!-- Commit Lock RELEASED (opus-preemption-cache) -->

<!-- opus-tab-quality (2026-06-06) WAVE 2: DONE — committed + PUSHED (origin/main @ dca94dc2,
     13cee281..dca94dc2, rev-list 0/0; another terminal has since stacked 308c3841 on top — my
     commit is its ancestor). Commit Lock RELEASED, all claims cleared.
     Immune-scan antibody AB-011 caught the c5f058a5 Preemption paywall bug hiding unfixed in the
     sibling Blind Spots tab (get_blind_spots Signal-gated → translateError → red "Something went
     wrong" banner for free-tier users). Fix (18 files): CENTRALIZED the gate classifier as
     isSignalGateError in utils/error-messages (single source of truth; preemption-slice drops its
     local copy, blind-spots-slice adopts it) + error-messages.test.ts (7 contract tests = the AB-011
     regression guard) + BlindSpotsView renders localized lock + SignalUpgradeCTA + 13 locales
     blindspots.locked.title/subtitle (real translations). Pre-push GREEN: full frontend suite + tsc +
     clippy --lib + translation parity. Normal render path unchanged (flag-guarded early-return).
     Did NOT touch any peer Rust WIP (digest_commands/lib/llm_capability/preemption/briefing_deterministic)
     / AdapterStatus.ts / fourda-infer-proto. NOTE: Blind Spots paywall render is live-untriggerable under
     dev_unlock (free-tier only) — verified at logic+test level, mirrors the live-verified Preemption fix. -->
     <!-- Commit Lock RELEASED (opus-tab-quality wave 2) -->

<!-- opus-brief-grounding (2026-06-06) WAVE 2 immune pass: DONE — committed + PUSHED (origin/main @
     13cee281, c5f058a5..13cee281, rev-list 0/0). Commit Lock RELEASED, claims cleared.
     Scanned every backend LLM-prose surface for the false-attribution class (Brief fix f8cde099).
     Found + fixed 3 recurrences with the same grounding pattern (3 files, +19/-3): channel_render.rs
     (no cross-ecosystem welding, present-as-news-if-unsure), content_personalization/llm_engine.rs
     (softened "show connections they haven't noticed" + shared GROUNDING constant), llm_judgments.rs
     (Tier-2: cross-ecosystem rule, else relevance <0.3). SAFE (no change): monitoring_briefing.rs
     (gold standard — groundedness check + dep list), llm_judge.rs ("judge topic not language" guard),
     adversarial.rs (pre-grounded deps), content_commands.rs (no stack input), translation/capability.
     Compile-clean + additive prompt constraints; per-surface live regen NOT done (app restarting under
     3-terminal churn; channel/insight/Tier-2 not on-demand triggerable) — covered by 7-day dogfood gate.
     Class recorded: .claude/wisdom/antibodies/2026-06-06-ungrounded-llm-attribution.md (gitignored).
     Did NOT touch the other terminals' WIP (digest_commands/llm_capability/briefing_deterministic;
     opus-tab-quality blind-spots/locales) / AdapterStatus.ts / fourda-infer-proto. -->
     <!-- Commit Lock RELEASED (opus-brief-grounding wave 2) -->

<!-- opus-tab-quality (2026-06-06): DONE — committed + PUSHED (origin/main @ c5f058a5,
     f8cde099..c5f058a5, rev-list 0/0). Commit Lock RELEASED, all claims cleared.
     3 doctrine-audit tab fixes (18 files, +124/-17): (1) Blind Spots ScoreBar fill now tracks
     `pressure` not `100-pressure` — number/magnitude/color agree (live-verified score 28 → bar 28%);
     (2) Signal ConfidenceIndicator dropped fabricated ±(1-conf)% margin → qualitative High/Med/Low
     + retired redundant ${signalCount}/5 defaultValue; (3) Preemption Signal-gate now renders a
     localized lock+upgrade CTA instead of a red error banner (detects gate sentinel, closes the
     backend-English leak). Added results.highConfidence/mediumConfidence + preemption.locked.title/
     subtitle to all 13 locales (real translations). Pre-push GREEN: full frontend suite + tsc +
     clippy --lib + translation parity. Did NOT touch AdapterStatus.ts / fourda-infer-proto/.gitignore.
     @opus-brief-grounding: your f8cde099 is on origin; my c5f058a5 stacked cleanly on top. -->
     <!-- Commit Lock RELEASED (opus-tab-quality) -->

<!-- opus-brief-grounding (2026-06-06): DONE — committed + PUSHED (origin/main @ f8cde099,
     42e86dd2..f8cde099, rev-list 0/0). Commit Lock RELEASED, claim cleared.
     Resumed the stuck "Evaluate morning brief" loop; finished the eval and fixed the real bug it
     would have missed: the AI Brief hallucinated by FALSE ATTRIBUTION (welded a global axios/npm CVE
     onto the user's Axum/Rust backend), fabricated a "51-hour blackout" from a benign stale-file
     anomaly, manufactured PAT-theft urgency, and self-reinforced via the briefing-seal continuity
     loop. Root cause: brief synthesized free-form prose from raw content items + a tech-stack string
     with NO grounding, while the deterministic dep-scoped Preemption truth was never given to it.
     Fix in digest_commands.rs (1 file, +93/-5): lever 1 (drop StaleData anomalies from brief context),
     lever 2 (build_grounded_security_section -> CONFIRMED SECURITY block from OSV-verified dep-scoped
     Preemption feed; system-prompt makes it the SOLE security source; CVE news -> awareness only),
     lever 3 (system-prompt: continuity/seal context is thematic-only, ban briefing meta-commentary +
     internal command-name labels). KEY INSIGHT: Option-A system-prompt guardrails ALONE were verified
     INSUFFICIENT — the brief is broken by INJECTED context, so a system rule loses to user-prompt
     content that asserts the bad facts. LIVE-VERIFIED across 3 rebuilds / ~7 regenerations: every
     dangerous defect gone; Action Required now = real OSV vulns scoped to the right projects. Push
     was briefly blocked by @opus-tab-quality's in-flight ConfidenceIndicator test (foreign, not mine);
     waited for green, then pushed clean. Did NOT touch AdapterStatus.ts / fourda-infer-proto / any of
     @opus-tab-quality's 16 WIP files. Benign residual: commit-feat label ~1/3 samples (self-healing
     seal artifact). -->
     <!-- Commit Lock RELEASED (opus-brief-grounding) -->

<!-- opus-victauri-bump (2026-06-06): DONE — committed + PUSHED (origin/main @ 42e86dd2,
     aea29dda..42e86dd2, rev-list 0/0). Commit Lock RELEASED, claims cleared.
     Bumped victauri 0.7.6→0.7.7 (Cargo.lock only; the "0.7" constraints already permit it).
     0.7.7 = crates-only patch (victauri-test headless-CI smoke fix, no API changes). Dev server
     restarted + live-verified: Victauri bridge reports 0.7.7 (was 0.7.6), 0 warnings, app healthy.
     Full src-tauri compile clean. windows-sys aligned to committed origin/main 0.61.2 (an uncommitted
     local 0.60.2 experiment was superseded — fastembed/tauri require ^0.61). Committed ONLY Cargo.lock;
     did NOT touch the ambient AdapterStatus.ts binding / fourda-infer-proto/.gitignore. -->
     <!-- Commit Lock RELEASED (opus-victauri-bump) -->

<!-- opus-relevance-funnel (2026-06-06): ALL DONE + PUSHED (origin/main @ aea29dda, 0/0 sync). The
     scoring relevance funnel is COMPLETE: Phase 0 (2aee268c), Phase 1+2 (743d68ac, a6f23162),
     Phase 4 forgetting/manual-only (2b27db15), Phase 5 calibration (79cf28ba), Phase 3 re-examination
     + cache.rs→scoring_queries.rs split (61bf34de), Phase 5b dep-scoped high-stakes recall (aea29dda).
     NO active claims; all funnel files free. Remaining OPTIONAL: 5b Part B (at-scale calibration corpus)
     / source-selection / dependabot tidy — see .claude/plans/PENDING-DECISION.md + scoring-relevance-funnel.md.
     Terminal closing for compaction. -->

### Terminal: opus-privacy-truth (started 2026-06-05)
Working on: privacy-claim accuracy hardening (research-driven). Truth-fixing false "data never
leaves / zero telemetry" claims, making the cloud-LLM consent gate real, zero-retention defaults,
audit-proof NETWORK.md, positioning doc. NONE of these overlap the scoring/triage backend.
**Claims (by wave):**
- Wave 1 (copy truth-fix): src/components/framework/FrameworkSections.tsx, src/components/WaitlistSignup.tsx,
  src/components/briefing/PersonalizeNudge.tsx, src/components/onboarding/OnboardingChoiceGate.tsx,
  src/locales/*/ui.json (all 13), README.md, CHANGELOG.md, NETWORK.md
- Wave 2 (real consent gate): src-tauri/src/llm.rs, src-tauri/src/settings/manager.rs,
  src/components/onboarding/setup-ai-provider.tsx, src/components/onboarding/use-quick-setup.ts,
  src/components/onboarding/quick-setup-utils.ts, src/components/settings/AIProviderSection.tsx
- Wave 3 (zero-retention): src-tauri/src/llm.rs, src-tauri/src/llm_stream.rs,
  src-tauri/src/embeddings_providers/openai.rs
- Wave 4 (network doc): NETWORK.md, docs/NETWORK-TRANSPARENCY.md
- Wave 5 (positioning): .claude/plans/ (gitignored)
**Status**: Wave 1a DONE — committed local @ c164edf3 (Sentry fully removed; local Export Diagnostics
+ log_frontend_error; scrubber w/ 10 tests; crash_reporting_opt_in purged). Push HELD for user.
NOW: Wave 1b (truth-fix BYOK "data never leaves" claims in hardcoded components + 13 locales + docs).
**Commit Lock**: RELEASED (opus-privacy-truth). A+ closers PUSHED @ 910a5393. Terminal done.
ALL DONE + PUSHED: privacy waves (c164edf3..3045be30) + repo/website consistency (4b1e4be7) + settings
BYOK disclosure (851ca4fc) + install-doc polish (e411b38f). Website live-verified on 4da.ai. A+ closers:
INVARIANTS INV-031 consent decision, NETWORK §2a per-provider retention, apply_openai_retention helper
+ 3 tests (llm.rs/llm_stream.rs). audit:public-ready clean. NOT touching other terminal's scoring_queries.rs.
ALL WAVES DONE + PUSHED (origin/main @ 3045be30). Consistency pass: site/src/*.njk (8) + ~22 docs +
tauri.conf.json listing. NEXT: commit, then A+ Tier-1 (settings disclosure, zero-retention completeness).
--- prior ---
ALL WAVES DONE (local, push held for user):
Wave 1a @ c164edf3 (Sentry->local diagnostics), Wave 1b @ daaa7417 (BYOK claim truth-fix, 13 locales),
Wave 4 @ e12656c8 (NETWORK.md audit-proof), Wave 2+3 @ 5d163182 (consent honesty + OpenAI zero-retention),
Wave 5 positioning doc (gitignored). victauri 0.7.6 live. export_diagnostics live-verified.
NOTE: running dev binary predates Wave 2/3 Rust edits (frontend disclosure live via HMR; Rust live on next rebuild).
Cargo.lock left uncommitted (not mine — windows-sys downgrade).

<!-- opus-relevance-funnel (2026-06-05): Phase 4 (forgetting) DONE — committed + pushed (measure-first;
     actual deletion HELD for user approval per destructive-ops protocol). Also ran the dependabot audit:
     cargo audit = 0 real vulns (18 unmaintained warnings, all transitive GTK3/Tauri — unfixable by us);
     pnpm audit = 2 dev-only (vitest critical needs 3→4 major bump; brace-expansion moderate → override).
     PHASE 4 relevance-aware forgetting: existing run_maintenance prunes by last_seen age (backwards —
     re-listed noise never ages out). New db::{count_prunable_noise, get_prunable_noise_sample, prune_noise}
     (shared predicate) forget CONFIRMED noise (relevance<thresh, scored, created_at>N days) while
     protecting high-stakes (security/breaking/CVE) + anything ≥ threshold. Bounded per call. Commands
     measure_noise_prune (dry-run) + run_noise_prune (bounded delete). 1 test. LIVE dry-run: default
     (0.05,90d)=0 (corpus ~40d young, safe); (0.05,30d)=114 candidates, all genuine off-stack noise
     (TS/Angular Qs, HF models, unused npm/go pkgs), 0 dep-matched/high-stakes. NOT auto-wired; deletion
     awaits approval. Files: db/history.rs, autophagy_commands.rs, lib.rs, src/lib/commands.ts.
     STAGED: upstream source filtering (fetch-time dep filter for registries — the bigger intake lever). -->
     <!-- Commit Lock RELEASED (opus-relevance-funnel) -->

<!-- opus-relevance-funnel (2026-06-05): Phase 1+2 DONE — committed + pushed. Builds on Phase 0 (2aee268c).
     PHASE 2 (backfill worker): the analysis path only scores a recent window, so ~88% of the corpus
     (31k items, 22k >7d old) was NEVER scored. New analysis_backfill::backfill_unscored_cycle scores
     the never-scored backlog in PRIORITY order (high-stakes → releases → recency via new
     db::get_unscored_backlog_chunk), persists + stamps version, convergent + resumable, NO LLM (cheap
     pipeline only), side-effect-free vs UI. Wired into the monitoring scheduler as a LOW-priority job
     every 120s (chunk 250), gated by scheduler_gate + cold-boot grace; idles to no-op once drained.
     PHASE 1 (observability): get_scoring_coverage command (cheap counts: total/scored/unscored/
     on-current-version %/version histogram) — the safety net that makes silent coverage collapse visible.
     LIVE-PROVEN: scheduler fires autonomously every 120s (dev log "Scoring backfill cycle" scored=250),
     unscored 31,726 → 28,413, on_current_version 1,263 → 4,863, zero manual calls. ~7.5k/hr → drains in ~4h.
     Files: db/cache.rs (2 methods + test), analysis_backfill.rs (NEW), triage_audit_commands.rs (+coverage),
     scheduler_state.rs (BACKFILL job), monitoring.rs (field+default+interval+job block), lib.rs (mod+3 cmds),
     src/lib/commands.ts (3 contracts). 6 cache tests + 8 triage tests green. NOT touching Cargo.lock /
     fourda-infer-proto / any frontend. Plan: .claude/plans/scoring-relevance-funnel.md (Phases 3-5 staged). -->
     <!-- Commit Lock RELEASED (opus-relevance-funnel) -->

<!-- opus-relevance-funnel (2026-06-05): Phase 0 DONE — committed + PUSHED (origin/main @ 2aee268c).
     Scoring relevance funnel Phase 0 (measure before build). Shipped: scoring/triage.rs (cheap gate:
     dep-match + taste/topic cosine + high-stakes carve-out, defer-not-delete, 8 tests), db::
     get_triage_audit_rows, triage_audit_commands::measure_triage_recall (+ commands.ts contract).
     MEASURED live 36k sweep: semantic gate has NO good operating point (0.45/0.55 keeps 84%; 0.55/0.65
     drops 15% relevant). PIVOT: prioritize don't filter — only dep-match/high-stakes are safe hard-keep;
     semantic = backfill priority, never a drop. Plan + curve: .claude/plans/scoring-relevance-funnel.md.
     My files are FREE. Did NOT touch Cargo.lock / fourda-infer-proto / any frontend (no orb-redesign overlap). -->
     <!-- Commit Lock RELEASED (opus-relevance-funnel) -->

<!-- opus-score-orb-redesign (2026-06-05): DONE — committed locally (push held for user), Commit Lock RELEASED.
     Full GAME web-component purge in 4 waves (frontend only; ZERO overlap with opus-relevance-funnel's backend):
     • Wave 1 d5311628 — the ugly WebGPU "score fingerprint" CORE orb → native SVG RelevanceRing
       (arc=relevance, core opacity=confidence, currentColor=tier). LIVE-VERIFIED via Victauri: 5 gold
       rings rendered in AttentionCards, 0 fourda-score-fingerprint elements. Screenshots in D:\lightshot\.
     • Wave 2 ff131a9b — fourda-tetrahedron / fourda-simplex-unfold → native components/geometry/
       (PlatonicSVG, SimplexUnfoldSVG) in LoadingOrEmptyState, BriefingNoDataState, first-run/LoadingState.
     • Wave 3 39da0182 — last 4 non-Platonic effects → native: status-orb→pulse dot (OllamaStatus),
       celebration-burst→ping rings (MilestoneOverlay), playbook-pathway→native node track (PlaybookView),
       turing-fire→AmbientGlow gradient (Briefing No-Data + Warmup).
     • Wave 4 be664e86 — deleted the whole apparatus: src/lib/fourda-components/ (69 files: .js/.frag/.wgsl/
       .d.ts), the registry, use-fourda-component hook, vite-env JSX decls, dead public/ notif-card+runtime
       assets, and 8 test suites' vi.mock stubs. 78 files, 22,383 deletions.
     Platonic visual language survives 100% as native SVG in components/geometry/. No WebGPU/WebGL anywhere.
     Verified: tsc 0, eslint 0, 126 tests across the 8 touched suites green. Waves 2-4 live-visual pending
     (app was down — opus-relevance-funnel rebuilding the Rust backend). Did NOT touch their in-flight
     src-tauri files / Cargo.lock / fourda-infer-proto.
     PUSHED: origin/main e9931ce9..be664e86 (rev-list 0/0). Full pre-push gate passed (tsc, full frontend
     suite, cargo fmt --check + clippy --lib on the shared tree). Terminal closing. -->
     <!-- Commit Lock RELEASED (opus-score-orb-redesign) -->

<!-- opus-stale-drain-ordering (2026-06-05): DONE — committed locally (push held for user).
     Completed the refinements opus-rescore-pipeline deferred. While verifying, found a THIRD,
     BIGGER root cause that subsumes "the drain doesn't fire" / "ecosystem_shift never surfaces":
     ★ ROOT CAUSE (the real one): get_stale_scored_items passed effective_hours=i64::MAX for SIGNAL
       users into `datetime('now','-'||?||' hours')`. SQLite OVERFLOWS that to NULL, so
       `created_at >= NULL` is never true → the query returned ZERO stale items for every Signal user.
       The live app is tier=signal, so the deep backlog (3828 stale items) was UNREACHABLE — the drain
       wasn't slow, it was empty. v5 only grew because the completion handler stamps recent items via
       normal scoring; the stale-drain itself never reached the backlog. LIVE-PROVEN via Victauri
       query_db: old signal predicate → 0 rows; new predicate → 3828 rows.
       Fix: for Signal (unlimited history) DROP the recency clause entirely (don't compute a giant
       offset). Free tier keeps the 30-day bound. Constant embedded (compile-time i64, no injection).
     • Fix 2 (ordering): drain was ORDER BY relevance DESC, but a version bump RESCUES items the old
       pipeline buried as noise (necessity try_stack_update_path: your own crates.io/npm release decayed
       to ~0). So relevance-DESC drained already-relevant items first, buried releases LAST. Now
       ORDER BY (content_type IN release_notes/platform_update) first, then relevance DESC. 583 stale
       releases (563 of them <0.3) now drain in the first 1-2 batches (500/run) instead of cycles 4-8.
       LIVE-PROVEN: first 12 of the new drain are all release_notes.
     • Fix 3 (fire on demand): extracted merge_stale_drain_batch() and ALSO call it on the full
       (non-differential) analysis path — previously the drain only ran in differential mode, so
       first-run-after-restart / manual run_cached_analysis never drained.
     Tests: 2 new in db::cache (release-first ordering; Signal no-overflow regression guard, returns
     1 not 0 for a 400-day-old item). Full lib compiles; clippy adds 0 new warnings; db::cache suite
     green. (analysis_status::abort_flag_resets_at_start is a pre-existing parallel-global-state flake —
     passes in isolation.) Files: src-tauri/src/db/cache.rs, src-tauri/src/analysis_status.rs.
     Did NOT touch pre-existing Cargo.lock / untracked fourda-infer-proto/.gitignore.
     PUSHED: origin/main b0cf5a85..e9931ce9 (rev-list 0/0; pre-push full suite passed).
     END-TO-END LIVE-VERIFIED on a fresh rebuild+restart (killed old PID 52180, pnpm tauri dev):
     one run_cached_analysis (FULL path, last_completed_at=null → proves Fix 3 drains the full path):
       • v5: 524 → 931 (+407 backlog items drained in ONE run — was structurally 0 on Signal before).
       • stale release_notes: 583 → 259 (324 releases re-scored this run = 80% of the drained items,
         though releases are only ~15% of the backlog → release-first ordering working).
       • crates.io: axum v0.8.9 (opus-rescore-pipeline's exact buried example, was 0.17) → 0.644.
         npm react v19.2.7 → 0.909, crates.io tokio v1.52.3 → 0.893 — all rescued from noise.
       • necessity_category in actionable results: ecosystem_shift = 100 items (the category that
         "hadn't appeared yet" now surfaces). Left the new dev binary running. -->
     <!-- Commit Lock RELEASED (opus-stale-drain-ordering) -->

<!-- opus-rescore-pipeline (2026-06-04→05): DONE — committed + PUSHED (origin/main @ b0cf5a85).
     Two shipped fixes to make this session's scoring improvements reach the 35k backlog:
     • 168d41fc: bump PIPELINE_VERSION 4→5 (the stale-drain mechanism existed but my scoring
       changes never bumped the version → nothing was flagged stale).
     • b0cf5a85: NEW Database::mark_items_scored_version — stamp version for EVERY scored item.
       LIVE-FOUND via Victauri: drain stalled because persist filters top_score>0, so zero-score
       (noise) items were never stamped, stayed stale, and the relevance-ordered drain re-picked
       them forever.
     LIVE-VERIFIED via Victauri query_db: v5 bucket grew 0→437→467→524 across runs (drain works +
     progresses; stall fixed). NOT yet reached: ecosystem_shift items — the drain is relevance-DESC
     so low-relevance stack releases (axum @ 0.17) sit deep in the queue, AND per-run drain slowed
     after the initial 437 because the differential 500-batch stale-drain branch isn't firing in
     practice (diff=0 in logs across runs — last_completed_at/previous_results not establishing
     differential mode on manual invokes; the scheduler drains over time). That's the next refinement.
     Left dev server running (PID 1313). Did NOT touch topic-decay's files / Cargo.lock / .gitignore. -->

<!-- (stale claim block retained below for history) -->
### Terminal: opus-rescore-pipeline (started 2026-06-04)
Working on: make this session's scoring improvements (necessity stack-update path, curated>synthesized
domain detection, ACE topic-noise filter, dep generic-word filter) reach the existing 35k-item backlog.
ROOT CAUSE: the stale-drain re-score mechanism already exists (get_stale_scored_items by
scored_pipeline_version < PIPELINE_VERSION, 500/run, ORDER BY relevance DESC) but my scoring changes
never bumped PIPELINE_VERSION (still 4) → nothing flagged stale → backlog never re-scored. Fix: bump 4→5.
**Claims:**
- src-tauri/src/scoring/mod.rs (PIPELINE_VERSION 4→5) — DONE/pushed (168d41fc)
- src-tauri/src/db/cache.rs (NEW mark_items_scored_version — stamp ALL scored items)
- src-tauri/src/analysis_status.rs (stamp version for every scored item, not just top_score>0)
LIVE-FOUND BUG (via Victauri): drain stalled — zero-score items were never version-stamped
(persist filters top_score>0) so the relevance-ordered drain re-picked the same zero-scorers
forever; backlog could never fully drain past a band of zero-scorers. Fix stamps every scored item.
**Commit Lock**: not yet held.

<!-- opus-topic-decay-rekey (2026-06-04→05): DONE — committed + PUSHED (origin/main @ 06fe4df5,
     168d41fc..06fe4df5, rev-list 0/0). Commit Lock RELEASED, claim cleared.
     Phase 0 of the MARS-inspired decay work: the per-topic calibrated-freshness path
     (scoring::pipeline.rs:199) was DEAD in prod — autophagy::analyze_topic_decay keyed half-lives by
     source_type (hackernews/reddit) but the pipeline looks them up by extract_topics() tokens
     (rust/react) → keyspaces never intersect → every item fell back to the crude global staircase.
     Re-keyed the producer to bucket by the SAME crate::extract_topics() vocab (title+content;
     source_tags not persisted) + MIN_SAMPLES_PER_TOPIC=3 guard vs Phase-3 proper-noun noise.
     1 file (topic_decay.rs, +116/-25), 2 new tests (topic-keyed not source-keyed; low-sample skip).
     autophagy 76 + scoring 618 green; calibration golden baselines unmoved (consumer untouched);
     full pre-push gate passed. context.rs claim DROPPED (its :162 comment correctly describes the
     source_autopsy load below it; topic_half_lives is an accurate name after the rename).
     Live 4da.db = 0 feedback → 0 profiles, so the path was dormant HERE regardless; activates once
     engagement accrues. Phases 1-3 (unify kernel → closed-form multi-rate → per-user) STAGED, gated by
     intelligence-doctrine rule 10 (7-day dogfood). Strategy in memory project_decay_strategy_mars.md.
     Did NOT touch pre-existing Cargo.lock / untracked fourda-infer-proto/.gitignore. -->
     <!-- Commit Lock RELEASED (opus-topic-decay-rekey) -->

<!-- opus-ace-quality-domain (2026-06-04): DONE — committed + PUSHED (origin/main @ 0af6e2d6, rev-list 0/0).
     Two domain-detection refinements on top of 7aea65e4, both forced by LIVE dogfood:
     • c96e22bf: weight ACE-inferred interests (cold-start users' interests are ACE-seeded source=Inferred).
     • 0af6e2d6: weight CURATED interests (explicit_interests, id=Some) > ACE-SYNTHESIZED (id=None). The repo's
       React frontend was synthesized into 5 web interests (react/typescript/javascript/next.js/express) that
       outvoted the 3 curated systems interests. id-based weighting fixes it.
     LIVE-VERIFIED on a guaranteed-fresh build: np flips web→systems (curated rust/tauri/axum win).
     ⚠ STALE-BINARY GOTCHA (cost ~40min): cargo kept NOT rebuilding after edits — running fourda.exe was
       OLDER than the commit, so warm reads showed the OLD web behavior. Fix: `touch src/probes_engine.rs &&
       cargo build` forces a real recompile (24s vs a bogus 1.5s up-to-date). Recipe saved.
     KEY FINDING (reported to user, NOT yet fixed — deeper than this scope): with honest systems targeting the
     calibration shows disc:1/recall:0 — the engine scores genuinely-relevant systems probes as noise
     ("Rust 2026 Edition" @ 0.257). PARTLY a probe-mode artifact (run_probe_calibration uses apply_signals:false,
     stricter than the real feed) — real feed DOES surface rust (Cargo advisory 0.6, Slumber TUI 0.535) but
     leans web (top item React/Next DoS 0.78). Candidate follow-ups: probe apply_signals:true to mirror feed;
     down-weight synthesized frontend interests in REAL feed scoring (not just domain detection).
     Left ONE clean dev server running (PID 452). Did NOT touch Cargo.lock / fourda-infer-proto/.gitignore. -->


<!-- opus-ace-quality (2026-06-04): DONE — committed + PUSHED (origin/main @ 7aea65e4,
     ef2d57cf..7aea65e4, rev-list 0/0). Commit Lock RELEASED, claims cleared.
     3 dogfood-found upstream quality fixes (the calibration numbers were honest, but the
     INPUTS were noisy and that noise degraded the live feed):
     (1) ACE topic noise → git.rs no longer emits commit-* topics + new high-precision
         is_low_quality_topic() gate (commit metadata, numeric/symbol fragments, camelCase
         code identifiers, <3 chars) applied at the interest-synthesis boundary AND auto-seed.
     (2) dep subterm false matches (winston-daily-rotate-file "matched" an AI paper via
         daily/rotate/file) → added generic words to COMMON_ENGLISH_WORDS; full name still matches.
     (3) domain mis-targeting (Rust/Tauri/Axum dev classified "web" → probe battery tested wrong
         domain) → detect_user_domain now lets explicit interests/onboarding tech dominate, weights
         auto-stacks low, caps ACE-topic breadth, recognises tauri/axum/wasm as systems.
     Tests: low_quality_topic matrix, winston sub-term exclusion, explicit-interests-beat-web-breadth.
     Affected suites green (probes 11, deps 37, git, context); full pre-push passed.
     Did NOT touch pre-existing Cargo.lock / untracked fourda-infer-proto/.gitignore. -->



<!-- opus-tab-fixes (2026-06-04): DONE — committed + PUSHED (origin/main @ ef2d57cf, rev-list 0/0).
     Commit Lock RELEASED, claims cleared. @opus-ace-quality: lock is FREE — proceed.
     5-tab doctrine audit (read-only agents) + clippy -D debt + 2 fix waves shipped:
     • clippy debt 61d50799 — removed dead DomainProfile.domains/infer_domains + u32::midpoint + sort_by_key.
     • Wave 1 (71aee94c/eaa1bf0c/b2fba069): Brief abstention-detection drift (rendered junk under a silent
       brief; now matches both Rust shapes + guard test), Preemption dismissal-count mismatch (count from
       post-dismissal visible items), Signal fabricated time-saved (removed 8s/article vanity metric).
     • Wave 2 (b71df73c/ef2d57cf): Brief EngagementPulse stopped fabricating 50%/stable on zero feedback
       (Option-based null + hide trend); Playbook honest cold-start (zero sun-runs → "stable" not "declining";
       ProgressRing hides "0%" on first run).
     Verified per wave: tsc 0, eslint 0, targeted vitest green, clippy --lib green, full pre-push gates passed.
     REMAINING (Wave 3, not started): the systemic backend-English i18n class (item titles/explanations/actions
     emitted in English from Rust across Blind Spots/Preemption/Playbook/Signal → render verbatim; proper fix =
     reason_code + frontend translation) + bounded fixes (Preemption paywall-as-error→upsell, missing
     explanation.expand/collapse locale keys, Blind Spots ScoreBar fill-vs-label, Signal ConfidenceIndicator ±%).
     Did NOT touch the 6 in-flight src-tauri/src files (opus-ace-quality's) / pre-existing Cargo.lock / .gitignore. -->
     <!-- Commit Lock RELEASED (opus-tab-fixes) -->

<!-- opus-coldstart-nudge (2026-06-04): DONE — committed + PUSHED (origin/main @ 3ef4d4c9,
     a7304418..3ef4d4c9, rev-list 0/0). Commit Lock RELEASED, claims cleared.
     Skipper-recovery cold-start fix: the first-run PersonalizeNudge (shown when a user finished
     onboarding with no interests — typically a skipper) now offers a ONE-CLICK fully-local
     "Scan my projects" instead of only bouncing to Settings. Reuses the store's runAutoDiscovery
     (ace_auto_discover — the same proven path the onboarding choice gate uses), then loadUserContext
     (nudge auto-resolves once interests populate) + startAnalysis (re-scores the feed). Settings kept
     as secondary; card stays dismissible; dismiss disabled mid-scan. Explicit click = consent (INV-004).
     NOTE: the bigger lever (in-session scan during onboarding) was ALREADY shipped by opus-provider-side
     Wave 3 (cf5dcc79/3468c4ce) — verified via an Explore agent before building, so this is the genuine
     remaining gap (skipper recovery), not a duplicate.
     Frontend-only: PersonalizeNudge.tsx + BriefingView.tsx + FreeBriefingPanel.tsx + new
     PersonalizeNudge.test.tsx (5 tests). Reused onboarding.choice.* locale keys (no new strings).
     Verified: tsc 0, 5 nudge tests, 57 briefing/first-run tests, eslint 0, clean HMR (0 warnings),
     full pre-push gate passed. Did NOT touch pre-existing Cargo.lock / fourda-infer-proto/.gitignore. -->
     <!-- Commit Lock RELEASED (opus-coldstart-nudge) -->

<!-- opus-builtin-removal-verify (2026-06-03→04): DONE — committed + PUSHED (origin/main @ a7304418,
     49d754a0..a7304418, rev-list 0/0). Commit Lock RELEASED, claims cleared.
     Post-Phase-2 verification of the built-in LLM removal:
     • LIVE (running app, Victauri :7373): detect_ghost_commands → 0 builtin ghosts; check_ipc_integrity →
       healthy, 0 errors/106 calls; test_llm_connection (anthropic) → real round-trip OK through the edited
       llm.rs; get_diagnostics → 0 warnings, 0 builtin/sidecar/llama mentions. App recompiled + restarted
       clean after the merge (PID 44332).
     • GBNF concern resolved by CODE PROOF: complete_ollama_structured uses Ollama-native format:json (never
       GBNF — that was builtin-only); my edit only swapped a match→irrefutable-let returning the same schema.
       Zero Ollama regression. Ollama confirmed available (llama3.2/qwen2.5:14b).
     • Migration UNIT-TESTED (a7304418): single-instance guard blocks a clean cold-start, so extracted
       migrate_retired_llm_provider() + 2 tests (builtin→none+model-cleared; none/ollama/anthropic/openai/
       openai-compatible untouched). Settings::validate() confirmed not to pre-empt provider.
     NOTE: immuneScanPending is for e3d557f6 + 91f53b0b (other sessions' fix commits) — NOT mine; left for
     the owning session. Did NOT touch pre-existing Cargo.lock / fourda-infer-proto/.gitignore. -->
     <!-- Commit Lock RELEASED (opus-builtin-removal-verify) -->
<!-- opus-builtin-removal-phase2 (2026-06-03): DONE — committed + PUSHED (origin/main @ 49d754a0,
     e3d557f6..49d754a0, rev-list 0/0). Commit Lock RELEASED, claims cleared.
     Phase 2 of the built-in LLM removal (backend) deferred by 25f0d945. ONE commit, built + validated
     in an isolated git worktree (dev server was live, would hot-reload mid-edit), then fast-forward merged.
     • Deleted llm_engine.rs (sidecar), model_manager.rs (GGUF catalog), settings_commands_llm/builtin.rs
       + the 7 builtin commands + lib.rs registrations + commands.ts contract.
     • Removed the `builtin` provider arm across the generation/capability stack (llm.rs, llm_stream.rs,
       llm_judgments.rs, ollama_capability.rs, ai_costs.rs, sovereign_developer_profile.rs,
       settings_commands.rs, health_checks.rs, 6 is_builtin_available guards, compute_has_llm) + the
       app_setup sidecar auto-start/shutdown. Dropped the llama-server-only StructuredOutputMode::Grammar
       variant + GBNF grammar — synthesis is JsonSchema for every provider now.
     • Migration: settings load resets persisted provider=="builtin" → "none" (manager_init.rs).
     • Frontend/locale: vestigial provider==='builtin' branches, 7 orphaned builtin-LLM locale keys ×13
       (built-in *embeddings* keys KEPT), sidecar error mapping. Removed 6 builtin victauri_dogfood tests
       + the builtin IPC-command assertions. .ai/FAILURE_MODES.md updated.
     • LANDMINES AVOIDED: blind_spots.rs/knowledge_decay.rs (Node.js builtin MODULES), calibration_*.rs/
       probes_engine.rs (fastembed embeddings, owned by opus-calibration-honesty-2), builtInSemantic* keys.
     Verified: cargo test --no-run (all compile), clippy adds 0 new warnings (4 remaining are pre-existing:
     domains/midpoint/sort_by), 3 compute_has_llm tests, tsc 0, 1260 frontend tests, validate:translations 0,
     27 script tests. Worktree + branch cleaned up. Did NOT touch pre-existing Cargo.lock / fourda-infer-proto/.gitignore. -->
     <!-- Commit Lock RELEASED (opus-builtin-removal-phase2) -->

<!-- opus-calibration-honesty-2 (2026-06-03): DONE — committed + PUSHED (origin/main @ f67db536,
     68a47f67..f67db536, rev-list 0/0). Immune-pass follow-up to the calibration honesty work.
     • HIGH fixed: signal-coverage axes fired from a DATA-EXISTENCE proxy (cached_context_count>0,
       active_topics non-empty) → replaced single-CVE audit_signal_axes with a BATTERY audit folded
       into run_probe_calibration (axis fires only when it crossed threshold on ≥1 of the 12
       real-embedding relevant probes). Removed proxy + one-probe volatility; unified two passes.
     • MEDIUM fixed: PersonaMetrics hardcoded fp:0/fn:0 → ProbeResults now carries real
       true_pos/false_pos/true_neg/false_neg; PersonaMetrics surfaces them.
     LIVE-VERIFIED (real profile, :7373): axes [context,interest,ace] → [context,interest,dependency]
     — phantom 'ace' (proxy) dropped, genuine 'dependency' surfaced; persona tp:4/fp:2/tn:4/fn:2
     (was fp:0/fn:0); disc 14, grade B/77. 10 probes_engine tests + full pre-push suite green.
     Immune pass recorded in antibody 2026-06-02-proxy-derived-state.md; c57ca5b9+8c88032e marked
     scanned. Restarted the dev server (it had exited during the pre-commit cache sweep) — UP on :7373.
     Did NOT touch pre-existing Cargo.lock / untracked fourda-infer-proto/.gitignore. -->



<!-- opus-debt-paydown (2026-06-02): DONE — committed (40205500..f1de614b, 5 commits).
     Commit Lock RELEASED, claims cleared. Paid down the documented debt from the
     vanity-gate/header + p1-false-state sessions (screenshots 2852/2853):
     • test(onboarding) 40205500 — builtin persistence path: quick-setup-utils.test.ts (21) +
       use-quick-setup.test.ts (8). Locks the false-ready guard (Built-in, no model → honest none).
     • test(doctrine) b607d6f6 — both gate scripts refactored to export their matcher (CLI/.husky
       behaviour unchanged, both still exit 0) + node:test suites (27) that pin catches AND known
       blind spots. New `pnpm run test:scripts`.
     • fix(intelligence) 4aa57ad4 — frontend sweep found 1 more proxy-state instance:
       ResultItemExpanded isLocalModel dropped builtin → fixed.
     • test(personalization) 1e44ec73 — compute_has_llm cloud + unknown-provider arms.
     • docs(failure-modes) f1de614b — proxy-derived-state class now recorded in tracked
       .ai/FAILURE_MODES.md (was gitignored-only).
     Verified: 1272 frontend tests, 27 script tests, 2 compute_has_llm Rust tests, tsc 0,
     validate:translations 0 errors, both gates exit 0. immuneScanPending CLEARED (ce67a49e +
     1f65229c added to scannedBugFixCommits — no new class, covered by the existing antibody;
     immune-pass note appended to 2026-06-02-proxy-derived-state.md).
     Did NOT touch the pre-existing Cargo.lock / untracked fourda-infer-proto/.gitignore.
     Orphaned worktree agent-a1d6dc2d1e211087e left in place — it has uncommitted changes
     (M specs/ARCHITECTURE.md); the cleanup script correctly refuses to remove it. -->
     <!-- Commit Lock RELEASED (opus-debt-paydown) -->

<!-- opus-vanity-gate-and-header (2026-06-02): DONE — committed + PUSHED (origin/main @ ce67a49e,
     1f65229c..ce67a49e, rev-list 0/0). Commit Lock RELEASED, claims cleared.
     • 33fb9bbd — vanity-metrics gate: scripts/check-vanity-metrics.cjs (wired into .husky/pre-commit)
       enforces intelligence-doctrine rule 3, flagging banned counters only when adjacent to a
       number/{{count}} (prose-safe). Tested 4 ways (clean/catch/prose-ignored/marker). Second
       doctrine rule now enforced at commit-time alongside the LLM-gate.
     • ce67a49e — onboarding AI-provider header reflects Built-in readiness (new builtinReady signal,
       updates on model-download-progress; "Built-in model ready"/"Download a model to enable" ×13).
       Live-verified: green check + "Built-in model ready" with qwen3-14b-q4km downloaded.
     All gates green; full pre-push suite passed. -->


<!-- opus-gate-and-builtin (2026-06-02): DONE — committed + PUSHED (origin/main @ 1f65229c,
     851fa416..1f65229c, rev-list 0/0). Commit Lock RELEASED, claims cleared.
     • bbed75de — antibody ENFORCEMENT: last 2 guarded sites (channel_render, settings/manager
       is_rerank_enabled) routed through compute_has_llm (tree now single-source-of-truth, fixes
       their builtin false-negative). New scripts/check-llm-gate-honesty.cjs wired into .husky/pre-commit
       (// llm-gate-ok: escape hatch) — tested clean/catch/marker.
     • 1f65229c — onboarding Built-in PERSISTENCE: builtinSelected lifted to the hook; on continue,
       saveBuiltinProvider persists provider="builtin"+downloaded model (or honest "none" if no model).
       Live-verified: Built-in → Enter 4DA → provider="builtin"/model="qwen3-14b-q4km", has_llm:true/local.
     immuneScanPending cleared (no new class — covered by antibody 2026-06-02-proxy-derived-state.md;
     the new gate now prevents recurrence). All gates green; full pre-push suite passed. -->


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

<!-- opus-onboarding-scrollbar (2026-06-01): DONE — committed + PUSHED (origin/main @ bf04e3b4).
     Dead root scrollbar behind the first-run overlays (splash + onboarding): documentElement
     (#root min-height) overflowed the viewport while the `fixed inset-0` overlay also scrolled →
     second, non-functional scrollbar. Fix landed as an isolated 1-file commit in src/App.tsx
     (lock documentElement overflow while splash/onboarding open, restore on close) — deliberately
     NOT in the claimed Onboarding.tsx; complements @opus-provider-side's in-flow overlap fix cf5dcc79.
     Mechanism live-verified via CDP (docScrollbarPx 8→0→8, overlay's own overflow-y-auto untouched);
     tsc 0, eslint 0. Commit Lock RELEASED, claim cleared. Terminal closing. -->
     <!-- Commit Lock RELEASED (opus-onboarding-scrollbar) -->
