#!/usr/bin/env node
/*
 * flush-tray-ghosts.cjs — clear orphaned 4DA icons from the Windows system tray.
 *
 * WHY THIS EXISTS
 *   Each running `fourda.exe` registers exactly one system-tray icon at startup
 *   (`monitoring::setup_tray`, called once per process). Windows only removes that
 *   icon when the process exits *cleanly* — Tauri drops the `TrayIcon` on
 *   `RunEvent::Exit`, which tells the shell to remove it. When the process is
 *   FORCE-KILLED or crashes, that cleanup never runs and the icon becomes a
 *   "ghost": it lingers in the tray-overflow flyout until something pings the
 *   dead window.
 *
 *   In this dev environment the process is killed constantly — cargo-watch
 *   TerminateProcess on every hot-reload, the documented `taskkill /F /IM
 *   fourda.exe` workflow, and the multi-agent build/test fleet (100+ process
 *   exits/hour). Each abnormal exit leaves a ghost, and they pile up into a grid
 *   of hundreds of identical "4" icons when you open the tray-overflow flyout.
 *
 *   This is a DEV artifact of force-killing, not a shipped-app bug: a normal
 *   install launches once, runs tray-resident, and quits cleanly (one icon,
 *   removed on exit). There is no per-icon removal API on Windows without a
 *   shell restart, so the reliable fix is to restart explorer.exe — it rebuilds
 *   the notification area from scratch and only *live* processes re-register
 *   their icons, so every ghost vanishes.
 *
 * WHAT IT DOES
 *   - Windows only (tray ghosts are a Win32 shell artifact). No-op elsewhere.
 *   - Reports how many fourda.exe processes are currently running (live icons
 *     come back automatically; only orphans are flushed).
 *   - Restarts explorer.exe (stop + relaunch), then confirms it came back.
 *
 * USAGE
 *   node scripts/flush-tray-ghosts.cjs            # flush now
 *   node scripts/flush-tray-ghosts.cjs --dry-run  # report only, do not restart
 *   pnpm run flush-tray-ghosts                     # via package.json alias
 *
 * Exit 0 on success or clean no-op (non-Windows). Exit 1 only if the shell
 * restart was attempted and explorer did not come back.
 */
'use strict';

const { spawnSync } = require('child_process');

const DRY_RUN = process.argv.includes('--dry-run');

function log(msg) {
  process.stdout.write(`${msg}\n`);
}

if (process.platform !== 'win32') {
  log('flush-tray-ghosts: Windows-only (tray ghosts are a Win32 shell artifact). Nothing to do.');
  process.exit(0);
}

// Run a PowerShell snippet and return trimmed stdout (empty string on failure).
function ps(script) {
  const res = spawnSync(
    'powershell.exe',
    ['-NoProfile', '-NonInteractive', '-Command', script],
    { encoding: 'utf8', windowsHide: true }
  );
  return (res.stdout || '').trim();
}

const fourdaCount = ps(
  "(Get-Process -Name fourda -ErrorAction SilentlyContinue | Measure-Object).Count"
);
log(`flush-tray-ghosts: fourda.exe running = ${fourdaCount || '0'}`);
if ((fourdaCount || '0') !== '0') {
  log('  note: 4DA is running — its live tray icon will re-register after the shell restarts.');
}

if (DRY_RUN) {
  log('flush-tray-ghosts: --dry-run — would restart explorer.exe to rebuild the tray. No action taken.');
  process.exit(0);
}

log('flush-tray-ghosts: restarting explorer.exe to rebuild the notification area...');
ps('Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue');

// Windows auto-restarts explorer in most configs; relaunch explicitly if not.
const restart = [
  'Start-Sleep -Milliseconds 1200',
  'if (-not (Get-Process -Name explorer -ErrorAction SilentlyContinue)) { Start-Process explorer.exe }',
  'Start-Sleep -Milliseconds 800',
  '(Get-Process -Name explorer -ErrorAction SilentlyContinue | Measure-Object).Count',
].join('; ');
const explorerCount = ps(restart);

if ((explorerCount || '0') === '0') {
  log('flush-tray-ghosts: explorer.exe did NOT come back. Run "explorer.exe" manually to restore the taskbar.');
  process.exit(1);
}

log(`flush-tray-ghosts: done — explorer.exe restarted (${explorerCount} running), orphaned 4DA tray icons cleared.`);
process.exit(0);
