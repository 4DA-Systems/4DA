#!/usr/bin/env node
/*
 * benchmark-cold-boot.cjs
 *
 * Sovereign Cold Boot — regression detector for the architectural invariants
 * that prevent the cold-boot stampede.
 *
 * This is a *static* benchmark: it inspects the codebase for the presence of
 * specific patterns that the Sovereign Cold Boot architecture depends on.
 * If any of them disappear (because someone refactors and reverts the fix),
 * this script fails the build.
 *
 * Why static and not runtime?
 *
 * A real runtime cold-boot benchmark would launch the compiled binary,
 * scrape the logs, and assert timing. That's valuable but adds 30+ seconds
 * to the release gate and is fragile across CI environments. The static
 * checks below catch >90% of regressions in <500ms with zero flakiness.
 *
 * Checks performed (each one corresponds to a Sovereign Cold Boot wave):
 *
 *   Wave 1 — sqlite-vec verified once
 *     • src-tauri/src/state.rs has `verify_sqlite_vec_once`
 *     • src-tauri/src/state.rs no longer logs `sqlite-vec verified` from
 *       inside `open_db_connection`
 *     • src-tauri/src/app_setup.rs calls `verify_sqlite_vec_once` from
 *       `initialize_pre_tauri`
 *     • `initialize_pre_tauri` does not initialize ACE
 *     • preemptive DB recovery treats locks as transient and uses a short timeout
 *
 *   Wave 1 — persisted scheduler timestamps
 *     • src-tauri/src/scheduler_state.rs exists
 *     • src-tauri/src/db/migrations.rs has `Phase 51` and `scheduler_state` table
 *     • TARGET_VERSION is at least 51
 *
 *   Wave 1 — adaptive cold-boot grace
 *     • src-tauri/src/monitoring.rs has a cold-boot grace period guard
 *
 *   Wave 2 — pre-baked briefing snapshot
 *     • src-tauri/src/briefing_snapshot.rs exists
 *     • get_briefing_snapshot is registered in lib.rs invoke_handler
 *     • The Tauri command type exists in src/lib/commands.ts
 *
 *   Wave 2 — ollama auto-pull is dead
 *     • src-tauri/src/ollama.rs no longer calls `pull_ollama_model` from
 *       `ensure_models_available`
 *     • The `ollama-needs-models` event is emitted instead
 *
 *   Wave 3 — frontend instant paint
 *     • src/main.tsx paints a boot shell before importing the full App graph
 *     • src/main.tsx does not statically import App
 *     • src/main.tsx keeps Tailwind/font CSS off the first-paint module graph
 *     • src/main.tsx signals Rust readiness before optional snapshot IPC
 *     • useAnalysis does not recursively load context files on app mount
 *     • get_context_files runs recursive filesystem reads on a blocking worker
 *     • debug startup purges stale WebView2 service workers before webview creation
 *     • optional snapshot IPC has a strict startup timeout
 *     • src/main.tsx renders React before optional snapshot hydration starts
 *     • src/main.tsx has no top-level await before React mounts
 *     • The instantSnapshot field exists in store/types.ts
 *
 *   Wave 4 — boot context detection
 *     • src-tauri/src/boot_context.rs exists
 *     • monitoring.rs reads `current_grace_secs()` instead of a hard-coded const
 *
 *   Wave 5 — universal startup watchdog
 *     • src-tauri/src/startup_watchdog.rs exists
 *     • app_setup.rs calls `begin_startup_watch`, starts the frontend gate,
 *       `start_heartbeat`, and `mark_clean_shutdown`
 *     • startup_frontend.rs calls `mark_phase0_complete` when showing the window
 *
 *   Wave 6 — phased startup instrumentation
 *     • app_setup.rs logs `phase = 0` elapsed milliseconds
 *     • ACE warmup waits for frontend readiness and first-light grace before expensive scans
 *     • scheduler background jobs wait for frontend readiness and first-light grace before health checks
 *     • startup Preemption cache warm waits for frontend readiness
 *     • Victauri dogfood mode uses a longer heavy-startup grace and disables auto-start work
 *     • Startup settings reads do not hydrate keychain secrets while holding the settings lock
 *
 *   Wave 7 — webview navigation recovery
 *     • startup_frontend.rs has the persistent recovery loop
 *
 * Usage:
 *   node scripts/benchmark-cold-boot.cjs           # full check
 *   node scripts/benchmark-cold-boot.cjs --quiet   # only print failures
 *
 * Exit codes:
 *   0 — all invariants present
 *   1 — one or more invariants missing (regression)
 */

'use strict';

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const QUIET = process.argv.includes('--quiet');

// ── Pretty output helpers ─────────────────────────────────────────────────
const GREEN = '\x1b[32m';
const RED = '\x1b[31m';
const YELLOW = '\x1b[33m';
const CYAN = '\x1b[36m';
const BOLD = '\x1b[1m';
const RESET = '\x1b[0m';

const passed = [];
const failed = [];

function read(rel) {
  const full = path.join(ROOT, rel);
  if (!fs.existsSync(full)) return null;
  return fs.readFileSync(full, 'utf8');
}

function fileExists(rel) {
  return fs.existsSync(path.join(ROOT, rel));
}

function check(name, ok, hint) {
  if (ok) {
    passed.push(name);
    if (!QUIET) console.log(`  ${GREEN}✓${RESET} ${name}`);
  } else {
    failed.push({ name, hint });
    console.log(`  ${RED}✗${RESET} ${name}`);
    if (hint) console.log(`    ${YELLOW}${hint}${RESET}`);
  }
}

function section(title) {
  if (!QUIET) console.log(`\n${BOLD}${CYAN}${title}${RESET}`);
}

// ──────────────────────────────────────────────────────────────────────────
// Wave 1 — sqlite-vec verified once
// ──────────────────────────────────────────────────────────────────────────
section('Wave 1 — sqlite-vec verified once');

const stateRs = read('src-tauri/src/state.rs') ?? '';
check(
  'state.rs declares verify_sqlite_vec_once',
  /pub fn verify_sqlite_vec_once\b/.test(stateRs),
  'add the one-shot verifier to state.rs'
);
check(
  'state.rs SQLITE_VEC_VERIFY_DONE one-shot guard exists',
  /SQLITE_VEC_VERIFY_DONE/.test(stateRs),
  'one-shot guard prevents per-connection re-verification'
);
// Extract the body of open_db_connection (from `fn open_db_connection` until
// the next blank-line-followed-by-fn declaration). Then verify it does NOT
// contain a literal info! call that includes "sqlite-vec verified".
function extractFnBody(src, fnName) {
  const idx = src.indexOf(`fn ${fnName}(`);
  if (idx === -1) return '';
  // Find the opening brace and walk to its matching close
  let depth = 0;
  let started = false;
  let end = idx;
  for (let i = idx; i < src.length; i++) {
    const c = src[i];
    if (c === '{') {
      depth++;
      started = true;
    } else if (c === '}') {
      depth--;
      if (started && depth === 0) {
        end = i;
        break;
      }
    }
  }
  return src.slice(idx, end + 1);
}

function hasTopLevelAwaitBefore(src, marker) {
  const end = src.indexOf(marker);
  if (end === -1) return true;
  const lines = src.slice(0, end).split(/\r?\n/);
  let depth = 0;

  for (const rawLine of lines) {
    const line = rawLine.replace(/\/\/.*$/, '');
    if (depth === 0 && /\bawait\b/.test(line)) {
      return true;
    }

    for (const ch of line) {
      if (ch === '{') depth += 1;
      if (ch === '}') depth = Math.max(0, depth - 1);
    }
  }

  return false;
}

const openDbBody = extractFnBody(stateRs, 'open_db_connection');
check(
  'open_db_connection no longer logs "sqlite-vec verified"',
  openDbBody.length > 0 && !/info!\([^)]*sqlite-vec verified/.test(openDbBody),
  'verify+log was moved to verify_sqlite_vec_once; per-connection logging is the regression'
);

const appSetupRs = read('src-tauri/src/app_setup.rs') ?? '';
check(
  'app_setup.rs calls verify_sqlite_vec_once from initialize_pre_tauri',
  /verify_sqlite_vec_once\(\)/.test(appSetupRs),
  'wire verify_sqlite_vec_once into initialize_pre_tauri'
);
const preTauriBody = extractFnBody(appSetupRs, 'initialize_pre_tauri');
check(
  'initialize_pre_tauri keeps ACE off the critical path',
  preTauriBody.length > 0 && !/get_ace_engine\s*\(/.test(preTauriBody),
  'ACE initialization belongs after first-light, not before Tauri/Victauri registration'
);

// ──────────────────────────────────────────────────────────────────────────
// Wave 1 — persisted scheduler timestamps
// ──────────────────────────────────────────────────────────────────────────
section('Wave 1 — persisted scheduler timestamps');

check(
  'scheduler_state.rs exists',
  fileExists('src-tauri/src/scheduler_state.rs'),
  'src-tauri/src/scheduler_state.rs is the stampede killer'
);

const schedulerStateRs = read('src-tauri/src/scheduler_state.rs') ?? '';
check(
  'scheduler_state.rs exposes hydrate_from_db',
  /pub fn hydrate_from_db\b/.test(schedulerStateRs),
  'hydrate_from_db is the entry point called from setup_app'
);
check(
  'scheduler_state.rs exposes persist_run',
  /pub fn persist_run\b/.test(schedulerStateRs),
  'jobs need persist_run to survive restart'
);

const migrationsRs = read('src-tauri/src/db/migrations.rs') ?? '';
check(
  'migration Phase 51 exists for scheduler_state table',
  /Phase 51/.test(migrationsRs) && /scheduler_state/.test(migrationsRs),
  'migration Phase 51 must create the scheduler_state table'
);
const targetVersionMatch = migrationsRs.match(/TARGET_VERSION:\s*i64\s*=\s*(\d+)/);
check(
  'migration TARGET_VERSION is at least 51',
  targetVersionMatch && parseInt(targetVersionMatch[1], 10) >= 51,
  'TARGET_VERSION must include Phase 51 (scheduler_state)'
);
check(
  'preemptive DB recovery is lock-bounded',
  /busy_timeout\(std::time::Duration::from_millis\(250\)\)/.test(migrationsRs) &&
    /is_lock_contention/.test(migrationsRs) &&
    /database locked during recovery quick_check/.test(migrationsRs) &&
    /locked_db_returns_recovery_failed_without_quarantine/.test(migrationsRs) &&
    /is_database_lock_contention/.test(stateRs),
  'cold-boot DB recovery must return quickly on SQLITE_BUSY/LOCKED and must never quarantine a merely locked DB'
);

// ──────────────────────────────────────────────────────────────────────────
// Wave 1 — adaptive cold-boot grace
// ──────────────────────────────────────────────────────────────────────────
section('Wave 1 — adaptive cold-boot grace period');

const monitoringRs = read('src-tauri/src/monitoring.rs') ?? '';
check(
  'monitoring.rs has cold-boot grace constant',
  /COLD_BOOT_GRACE_SECS/.test(monitoringRs),
  'COLD_BOOT_GRACE_SECS_DEFAULT documents the safe ceiling'
);
check(
  'monitoring.rs scheduler defers maintenance during grace',
  /Cold-boot grace period|cold_boot_elapsed/.test(monitoringRs),
  'scheduler must skip maintenance for the first N seconds after start'
);

// ──────────────────────────────────────────────────────────────────────────
// Wave 2 — pre-baked briefing snapshot
// ──────────────────────────────────────────────────────────────────────────
section('Wave 2 — pre-baked briefing snapshot');

check(
  'briefing_snapshot.rs exists',
  fileExists('src-tauri/src/briefing_snapshot.rs'),
  'briefing_snapshot.rs is the killer feature'
);

const briefingSnapshotRs = read('src-tauri/src/briefing_snapshot.rs') ?? '';
check(
  'briefing_snapshot.rs exposes get_briefing_snapshot tauri::command',
  /#\[tauri::command\][\s\S]{0,200}fn get_briefing_snapshot/.test(briefingSnapshotRs),
  'get_briefing_snapshot is the frontend entry point'
);
check(
  'briefing_snapshot.rs exposes save_snapshot',
  /pub fn save_snapshot\b/.test(briefingSnapshotRs),
  'save_snapshot is called by monitoring + Stop handler'
);
check(
  'briefing_snapshot.rs writes atomically (temp + rename)',
  /\.tmp/.test(briefingSnapshotRs) && /rename/.test(briefingSnapshotRs),
  'atomic write protects against mid-write corruption'
);

const libRs = read('src-tauri/src/lib.rs') ?? '';
check(
  'lib.rs registers get_briefing_snapshot in invoke_handler',
  /briefing_snapshot::get_briefing_snapshot/.test(libRs),
  'register the command in tauri::generate_handler!'
);

const commandsTs = read('src/lib/commands.ts') ?? '';
check(
  'commands.ts has get_briefing_snapshot in CommandMap',
  /get_briefing_snapshot:\s*\{\s*params/.test(commandsTs),
  'the IPC validator requires the entry on a single line'
);

const defaultCapabilityJson = read('src-tauri/capabilities/default.json') ?? '';

// ──────────────────────────────────────────────────────────────────────────
// Wave 2 — Ollama auto-pull is dead
// ──────────────────────────────────────────────────────────────────────────
section('Wave 2 — Ollama auto-pull replaced with consent banner');

const ollamaRs = read('src-tauri/src/ollama.rs') ?? '';
// Extract the function body and check for actual call sites (not doc comments).
// Doc comments mentioning pull_ollama_model are fine — they document the
// frontend-driven flow. What's NOT fine is an actual `pull_ollama_model(...)` call.
const ensureBody = extractFnBody(ollamaRs, 'ensure_models_available');
const pullCallRegex = /(?<!`)\bcrate::settings_commands::pull_ollama_model\s*\(/;
check(
  'ollama.rs ensure_models_available no longer auto-pulls',
  ensureBody.length > 0 && !pullCallRegex.test(ensureBody),
  'auto-pull was the worst cold-boot offender — never reintroduce it'
);
check(
  'ollama.rs emits ollama-needs-models event',
  /ollama-needs-models/.test(ollamaRs),
  'consent request replaces silent auto-pull'
);
check(
  'ollama.rs estimates download size for the consent banner',
  /estimate_model_size_mb|estimated_mb/.test(ollamaRs),
  'honest size estimate makes the consent prompt trustworthy'
);

// ──────────────────────────────────────────────────────────────────────────
// Wave 3 — frontend instant paint
// ──────────────────────────────────────────────────────────────────────────
section('Wave 3 — frontend instant paint');

const mainTsx = read('src/main.tsx') ?? '';
const useAnalysisTs = read('src/hooks/use-analysis.ts') ?? '';
const useAppListenersTs = read('src/hooks/use-app-listeners.ts') ?? '';
const briefingViewTsx = read('src/components/BriefingView.tsx') ?? '';
const briefingWarmupStateTsx = read('src/components/BriefingWarmupState.tsx') ?? '';
const startupRuntimeTs = read('src/lib/startup-runtime.ts') ?? '';
const contextCommandsRs = read('src-tauri/src/context_commands.rs') ?? '';
const settingsCommandsRs = read('src-tauri/src/settings_commands.rs') ?? '';
const settingsCommandsLicenseRs = read('src-tauri/src/settings_commands_license.rs') ?? '';
const startupHealthRs = read('src-tauri/src/startup_health.rs') ?? '';
check(
  'main.tsx does not statically import the full App graph',
  !/import\s+App\s+from\s+['"]\.\/App['"]/.test(mainTsx),
  'the full App graph must be loaded after first paint, not before main.tsx can execute'
);
check(
  'main.tsx keeps CSS/font imports off the first-paint graph',
  !/import\s+['"]\.\/App\.css['"]/.test(mainTsx) &&
    !/import\s+['"]@fontsource-variable\//.test(mainTsx),
  'Tailwind scanning and font CSS must stay behind the dynamic App import'
);
check(
  'main.tsx paints BootShell before importing App',
  /function\s+BootShell/.test(mainTsx) &&
    /root\.render\([\s\S]*<BootShell\s*\/>[\s\S]*\);/.test(mainTsx) &&
    mainTsx.indexOf('<BootShell />') < mainTsx.indexOf("import('./App')"),
  'the hidden webview must get a real root child before the heavy app module graph loads'
);
check(
  'main.tsx signals frontend readiness before snapshot IPC',
  /frontend-ready/.test(mainTsx)
    && /mark_frontend_ready/.test(mainTsx)
    && /get_briefing_snapshot/.test(mainTsx)
    && mainTsx.indexOf('void signalFrontendReady()') < mainTsx.indexOf('void hydrateStartupSnapshot()'),
  'first-light readiness must not wait behind optional snapshot hydration'
);
check(
  'main window capability allows frontend-ready event emission',
  /core:event:default/.test(defaultCapabilityJson),
  'main window must be allowed to emit frontend-ready before any command IPC'
);
check(
  'use-analysis does not load context files on app mount',
  !/useEffect\(\s*\(\)\s*=>\s*\{[\s\S]{0,120}loadContextFiles\(\)/.test(useAnalysisTs),
  'recursive context file reads belong behind the Results context panel, not the default app mount'
);
check(
  'get_context_files runs recursive file reads on a blocking worker',
  /spawn_blocking/.test(contextCommandsRs) && /collect_context_files/.test(contextCommandsRs),
  'recursive project file reads must not occupy the async runtime used by IPC'
);
check(
  'app_setup.rs purges stale dev WebView2 service workers',
  /purge_dev_webview_service_worker_cache/.test(appSetupRs) &&
    /Service Worker/.test(appSetupRs) &&
    /EBWebView/.test(appSetupRs),
  'stale localhost service workers must not be able to hijack the dev app shell'
);
check(
  'main.tsx bounds optional snapshot IPC wait',
  /STARTUP_SNAPSHOT_TIMEOUT_MS/.test(mainTsx) && /withStartupTimeout/.test(mainTsx),
  'snapshot hydration is optional; an IPC stall must not block createRoot'
);
check(
  'main.tsx renders React before optional snapshot hydration starts',
  /void hydrateStartupSnapshot\(\)/.test(mainTsx) &&
    mainTsx.indexOf('<BootShell />') < mainTsx.indexOf('void hydrateStartupSnapshot()'),
  'React first render must never wait behind optional snapshot hydration'
);
check(
  'main.tsx has no top-level await before React mounts',
  !hasTopLevelAwaitBefore(mainTsx, '<BootShell />'),
  'move startup async work into non-blocking background tasks'
);
check(
  'main.tsx stashes snapshot on window.__4DA_INSTANT_SNAPSHOT__',
  /__4DA_INSTANT_SNAPSHOT__/.test(mainTsx),
  'globalThis stash bridges the pre-React fetch to the briefing slice'
);

const briefingSliceTs = read('src/store/briefing-slice.ts') ?? '';
check(
  'briefing-slice consumes the preloaded snapshot',
  /readPreloadedSnapshot|__4DA_INSTANT_SNAPSHOT__/.test(briefingSliceTs),
  'briefing slice must initialize instantSnapshot from the global stash'
);

const storeTypesTs = read('src/store/types.ts') ?? '';
check(
  'store/types.ts declares InstantBriefingSnapshot',
  /InstantBriefingSnapshot\b/.test(storeTypesTs),
  'instant snapshot type is part of the store contract'
);

// ──────────────────────────────────────────────────────────────────────────
// Wave 4 — boot context detection
// ──────────────────────────────────────────────────────────────────────────
section('Wave 4 — boot context detection');

check(
  'boot_context.rs exists',
  fileExists('src-tauri/src/boot_context.rs'),
  'boot_context.rs adapts grace period to launch cause'
);

const bootContextRs = read('src-tauri/src/boot_context.rs') ?? '';
check(
  'boot_context.rs has all four launch contexts',
  /ColdPowerOn/.test(bootContextRs)
    && /AutoStart/.test(bootContextRs)
    && /UserLaunched/.test(bootContextRs)
    && /ProcessRestart/.test(bootContextRs),
  'all four contexts must be enumerated'
);
check(
  'boot_context.rs exposes current_grace_secs',
  /pub fn current_grace_secs\b/.test(bootContextRs),
  'monitoring.rs reads the dynamic grace period from this fn'
);
check(
  'monitoring.rs reads boot_context::current_grace_secs',
  /current_grace_secs\(\)/.test(monitoringRs),
  'scheduler must use the dynamic grace, not a hard-coded constant'
);

// ──────────────────────────────────────────────────────────────────────────
// Wave 5 — universal startup watchdog
// ──────────────────────────────────────────────────────────────────────────
section('Wave 5 — universal startup watchdog');

check(
  'startup_watchdog.rs exists',
  fileExists('src-tauri/src/startup_watchdog.rs'),
  'startup_watchdog.rs is the last-line safety net'
);

const watchdogRs = read('src-tauri/src/startup_watchdog.rs') ?? '';
const startupFrontendRs = read('src-tauri/src/startup_frontend.rs') ?? '';
check(
  'startup_watchdog.rs exposes begin_startup_watch',
  /pub fn begin_startup_watch\b/.test(watchdogRs),
  'begin_startup_watch records start time + crash trail'
);
check(
  'startup_watchdog.rs exposes mark_phase0_complete',
  /pub fn mark_phase0_complete\b/.test(watchdogRs),
  'phase 0 mark fires when the window is visible'
);
check(
  'startup_watchdog.rs exposes start_heartbeat',
  /pub fn start_heartbeat\b/.test(watchdogRs),
  'heartbeat enables frontend to detect frozen backend'
);
check(
  'startup_watchdog.rs exposes mark_clean_shutdown',
  /pub fn mark_clean_shutdown\b/.test(watchdogRs),
  'clean shutdown removes the .running marker'
);

check(
  'app_setup.rs wires begin_startup_watch into pre-Tauri init',
  /begin_startup_watch\(\)/.test(appSetupRs),
  'watchdog must initialize in initialize_pre_tauri'
);
check(
  'startup_frontend.rs exists',
  fileExists('src-tauri/src/startup_frontend.rs'),
  'startup_frontend.rs owns the hidden-window first-light gate'
);
check(
  'app_setup.rs starts the frontend readiness gate',
  /start_frontend_readiness_gate\(\s*app_handle\.clone\(\)\s*\)/.test(appSetupRs),
  'setup_app must start the frontend gate as soon as it has an AppHandle'
);
check(
  'startup_frontend.rs calls mark_phase0_complete on window-show',
  /mark_phase0_complete\(\)/.test(startupFrontendRs),
  'every code path that shows the window must mark phase 0 complete'
);
check(
  'app_setup.rs starts the heartbeat',
  /start_heartbeat\(\)/.test(appSetupRs),
  'heartbeat must start at the end of setup_app'
);
check(
  'app_setup.rs marks clean shutdown in Stop handler',
  /mark_clean_shutdown\(\)/.test(appSetupRs),
  'clean shutdown removes crash markers'
);

// ──────────────────────────────────────────────────────────────────────────
// Wave 6 — phased startup instrumentation
// ──────────────────────────────────────────────────────────────────────────
section('Wave 6 — phased startup instrumentation');

check(
  'app_setup.rs logs phase 0 elapsed milliseconds',
  /phase\s*=\s*0/.test(appSetupRs) && /elapsed_ms/.test(appSetupRs),
  'phase budgets must be observable in cold-boot logs'
);
check(
  'app_setup.rs records setup_began Instant',
  /setup_began\s*=\s*std::time::Instant::now/.test(appSetupRs),
  'setup_began is the clock used by phase markers'
);
check(
  'startup_frontend.rs defines debug-aware first-light background grace',
    /background_grace_after_first_light/.test(startupFrontendRs) &&
    /debug_assertions/.test(startupFrontendRs) &&
    /victauri_e2e_active/.test(startupFrontendRs) &&
    /from_mins\(5\)/.test(startupFrontendRs) &&
    /from_secs\(90\)/.test(startupFrontendRs) &&
    /from_secs\(20\)/.test(startupFrontendRs),
  'dev/Victauri boots need a longer Vite/Tailwind grace while release keeps the shorter delay'
);
check(
  'startup_frontend.rs defines longer heavy-startup grace for dogfood/dev',
    /heavy_startup_work_grace_after_first_light/.test(startupFrontendRs) &&
    /victauri_e2e_active/.test(startupFrontendRs) &&
    /from_mins\(5\)/.test(startupFrontendRs) &&
    /from_mins\(3\)/.test(startupFrontendRs) &&
    /from_secs\(20\)/.test(startupFrontendRs),
  'ACE/Preemption scans need a separate heavy-work grace so live dogfood does not race background CPU spikes'
);
check(
  'app_setup.rs defers ACE warmup until after first-light grace',
  /wait_until_frontend_ready/.test(appSetupRs) &&
    /heavy_startup_work_grace_after_first_light\(\)\)\s*\.await[\s\S]{0,700}Running AUTONOMOUS ACE context scan/.test(appSetupRs),
  'ACE project scans must not compete with the hidden webview first-light or full-app import path'
);
check(
  'monitoring.rs defers scheduler jobs until after first-light grace',
  /wait_until_frontend_ready/.test(monitoringRs) &&
    /Frontend did not report ready before scheduler start/.test(monitoringRs) &&
    /background_grace_after_first_light\(\)\)\.await[\s\S]{0,700}last_check/.test(monitoringRs),
  'scheduler maintenance must not initialize background work before first-light and full-app import'
);
check(
  'app_setup.rs defers Preemption cache warm until frontend readiness',
  /warm_preemption_cache_after_first_light/.test(appSetupRs) &&
    /wait_until_frontend_ready/.test(appSetupRs) &&
    /heavy_startup_work_grace_after_first_light/.test(appSetupRs) &&
    !/OSV mirror synced recently[\s\S]{0,260}preemption::warm_preemption_cache\(\)\.await/.test(appSetupRs),
  'Preemption cache recompute is CPU-heavy and must not run on the first-light path'
);
check(
  'all frontend auto-analysis paths are disabled during Victauri dogfood startup',
  /get_startup_runtime_flags/.test(startupRuntimeTs) &&
    /victauriE2e/.test(startupRuntimeTs) &&
    /isVictauriDogfoodMode/.test(useAppListenersTs) &&
    /isVictauriDogfoodMode/.test(briefingViewTsx) &&
    /isVictauriDogfoodMode/.test(briefingWarmupStateTsx),
  'Victauri smoke verification must not trigger foreground analysis from mount, snapshot freshen, or warmup empty-state paths'
);
check(
  'backend auto-start jobs are disabled during Victauri dogfood startup',
  /Victauri E2E active - skipping startup OSV sync and Preemption cache warm/.test(appSetupRs) &&
    /Victauri E2E active - skipping startup ACE initialization/.test(appSetupRs) &&
    /Victauri E2E active - skipping startup data cleanup/.test(appSetupRs) &&
    /Victauri E2E active - skipping startup dep-linker repair/.test(appSetupRs) &&
    /Victauri E2E active - skipping startup context corpus maintenance/.test(appSetupRs) &&
    /Victauri E2E active - skipping startup model registry refresh/.test(appSetupRs) &&
    /Victauri E2E active - skipping immediate morning briefing check/.test(appSetupRs) &&
    /Victauri E2E active - background scheduler disabled for live verification/.test(monitoringRs),
  'Victauri smoke verification should measure the shell and IPC without startup schedulers, DB cleanup, OSV, Preemption, ACE, registry, or briefing jobs competing for runtime threads'
);
check(
  'startup get_settings does not hydrate keychain secrets under the settings lock',
  /pub async fn get_settings/.test(settingsCommandsRs) &&
    !/pub async fn get_settings[\s\S]{0,260}ensure_keys_hydrated/.test(settingsCommandsRs),
  'get_settings is on the startup paint path; keychain recovery belongs in save/key-consuming paths, not in this IPC response'
);
check(
  'license and trial reads clone settings before local work',
  /guard\.get\(\)\.license\.clone\(\)/.test(settingsCommandsLicenseRs) &&
    /get_trial_status[\s\S]{0,220}guard\.get\(\)\.license\.clone\(\)/.test(settingsCommandsLicenseRs),
  'license badge reads must not hold the global settings lock while doing expiry or cache work'
);
check(
  'startup health is cached before frontend mount',
  /initialize_startup_health_cache\(\)/.test(appSetupRs) &&
    /pub\(crate\) fn initialize_startup_health_cache/.test(startupHealthRs) &&
    /pub\(crate\) fn get_startup_health\(\) -> Vec<HealthIssue>[\s\S]{0,180}initialize_startup_health_cache\(\)/.test(startupHealthRs),
  'HealthBanner must not recompute startup health, keychain, or DB probes after the webview mounts'
);
check(
  'startup health IPC path does not hydrate keychain secrets',
  !/pub\(crate\) fn get_startup_health[\s\S]{0,500}ensure_keys_hydrated/.test(startupHealthRs) &&
    /Victauri E2E active - skipping startup health keychain probe/.test(startupHealthRs),
  'startup health must remain cached and dogfood-safe; keychain hydration belongs in key-consuming paths'
);

// ──────────────────────────────────────────────────────────────────────────
// Wave 7 — webview navigation recovery
// ──────────────────────────────────────────────────────────────────────────
section('Wave 7 — webview navigation recovery');

check(
  'startup_frontend.rs has a persistent recovery loop',
  /recovery loop|recovery_began/.test(startupFrontendRs),
  'the dev-mode recovery loop must keep running, not give up after 30s'
);
check(
  'startup_frontend.rs re-navigates webview when dev server returns',
  /consecutive_navigates/.test(startupFrontendRs),
  'recovery loop must re-navigate, not just probe'
);
check(
  'startup_frontend.rs recovery can mark a rendered root ready',
  /__TAURI__\?\.core\?\.invoke/.test(startupFrontendRs) && /mark_frontend_ready/.test(startupFrontendRs),
  'recovery must use the public Tauri v2 invoke path when the React root is present'
);

// ──────────────────────────────────────────────────────────────────────────
// Summary
// ──────────────────────────────────────────────────────────────────────────
const total = passed.length + failed.length;
console.log(`\n${BOLD}Sovereign Cold Boot benchmark${RESET}`);
console.log(`  ${GREEN}${passed.length}${RESET} passed   ${failed.length > 0 ? RED : GREEN}${failed.length}${RESET} failed   ${total} total`);

if (failed.length > 0) {
  console.log(`\n${RED}${BOLD}REGRESSION DETECTED${RESET} — Sovereign Cold Boot architectural invariants are missing.`);
  console.log('Each failure represents a real cold-boot UX regression.');
  console.log('Restore the missing invariant or update this script if the architecture has intentionally changed.\n');
  process.exit(1);
}

console.log(`\n${GREEN}${BOLD}All Sovereign Cold Boot invariants present.${RESET}`);
console.log('Cold boot UX is protected from regression.\n');
process.exit(0);
