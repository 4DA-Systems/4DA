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
 *     • src/main.tsx fetches `get_briefing_snapshot` before React mounts
 *     • The instantSnapshot field exists in store/types.ts
 *
 *   Wave 4 — boot context detection
 *     • src-tauri/src/boot_context.rs exists
 *     • monitoring.rs reads `current_grace_secs()` instead of a hard-coded const
 *
 *   Wave 5 — universal startup watchdog
 *     • src-tauri/src/startup_watchdog.rs exists
 *     • app_setup.rs calls `begin_startup_watch`, `mark_phase0_complete`,
 *       `start_heartbeat`, and `mark_clean_shutdown`
 *
 *   Wave 6 — phased startup instrumentation
 *     • app_setup.rs logs `phase = 0` elapsed milliseconds
 *
 *   Wave 7 — webview navigation recovery
 *     • app_setup.rs has the persistent recovery loop ("Phase B")
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
check(
  'main.tsx fetches get_briefing_snapshot before React mounts',
  /get_briefing_snapshot/.test(mainTsx) && mainTsx.indexOf('get_briefing_snapshot') < mainTsx.indexOf('createRoot'),
  'snapshot must be loaded BEFORE React mounts for sub-200ms first paint'
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
  'app_setup.rs calls mark_phase0_complete on window-show',
  /mark_phase0_complete\(\)/.test(appSetupRs),
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

// ──────────────────────────────────────────────────────────────────────────
// Wave 7 — webview navigation recovery
// ──────────────────────────────────────────────────────────────────────────
section('Wave 7 — webview navigation recovery');

check(
  'app_setup.rs has a persistent recovery loop',
  /Phase B|recovery loop|recovery_began/.test(appSetupRs),
  'the dev-mode recovery loop must keep running, not give up after 30s'
);
check(
  'app_setup.rs re-navigates webview when dev server returns',
  /consecutive_navigates/.test(appSetupRs),
  'recovery loop must re-navigate, not just probe'
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
