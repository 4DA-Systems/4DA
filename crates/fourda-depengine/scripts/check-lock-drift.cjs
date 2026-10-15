#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// The engine runs from two lockfiles. Inside the app it is a path dependency,
// so `src-tauri/Cargo.lock` picks its serde / serde_json / semver / regex;
// the standalone build (CI tests, the wasm module) uses this crate's own
// `Cargo.lock`. Those should not run different rules: semver parses every
// version the matcher orders, regex reads the non-semver schemes, and
// serde_json writes the stored ranges and the wasm API's answers.
//
// Policy, for every package both lockfiles hold:
// - same major.minor (patch differs): a note. Patch releases of these crates
//   are fixes within the same behaviour, and the wasm == native corpus check
//   (check-wasm.cjs) still runs on the crate's own lockfile.
// - major.minor drift that THIS CHANGE introduced on the crate side (the
//   crate lockfile's version is not in the base commit's crate lockfile):
//   an error. The crate must follow the app, never lead it.
// - any other major.minor drift (the app moved first, or no base is known):
//   a ::warning, not a failure. Dependabot bumps /src-tauri and
//   /crates/fourda-depengine in separate PRs; failing the app-side PR would
//   leave neither able to merge alone. The app's PR lands with a warning;
//   the crate's PR to the same line then has no drift and passes.
//
// Usage: node check-lock-drift.cjs [--base-crate-lock FILE] [crate.lock] [app.lock]
// `--base-crate-lock` is the crate's Cargo.lock at the merge base; an empty
// file means the crate had no lockfile there (every version is new).
// Exit 1 on crate-introduced drift or an unreadable lockfile.

'use strict';

const fs = require('node:fs');
const path = require('node:path');

/** name -> Set of versions, from a Cargo.lock's [[package]] tables. */
function parseLock(text) {
  const out = new Map();
  for (const block of text.split(/\r?\n\[\[package\]\]\r?\n/).slice(1)) {
    const name = /^name = "([^"]+)"/m.exec(block)?.[1];
    const version = /^version = "([^"]+)"/m.exec(block)?.[1];
    if (!name || !version) continue;
    if (!out.has(name)) out.set(name, new Set());
    out.get(name).add(version);
  }
  return out;
}

const majorMinor = (v) => v.split(/[.+-]/).slice(0, 2).join('.');

/**
 * Compare the crate's lockfile with the app's.
 * @param {Map<string, Set<string>>} crate
 * @param {Map<string, Set<string>>} app
 * @param {Map<string, Set<string>> | null} baseCrate the crate lockfile at the
 *   merge base, or null when unknown (drift is then never blamed on the crate)
 * @returns {{shared: number, notes: string[], warnings: string[], errors: string[]}}
 */
function checkDrift(crate, app, baseCrate) {
  const result = { shared: 0, notes: [], warnings: [], errors: [] };
  for (const [name, versions] of [...crate].sort(([a], [b]) => a.localeCompare(b))) {
    if (name === 'fourda-depengine' || !app.has(name)) continue;
    result.shared++;
    const appVersions = [...app.get(name)];
    for (const v of versions) {
      if (appVersions.includes(v)) continue;
      // The app may hold several majors of a crate; compare with the one on
      // the same major.minor line if there is one.
      const sameLine = appVersions.find((a) => majorMinor(a) === majorMinor(v));
      if (sameLine) {
        result.notes.push(`${name}: crate ${v}, app ${sameLine} (patch drift, allowed)`);
        continue;
      }
      const line = `${name}: crate ${v}, app ${appVersions.join(' / ')} (major.minor drift)`;
      const crateMovedIt = baseCrate !== null && !(baseCrate.get(name)?.has(v) ?? false);
      if (crateMovedIt) {
        result.errors.push(`${line}; this change moved the crate's lockfile away from the app's`);
      } else {
        result.warnings.push(
          `${line}; the app's lockfile moved first: bump crates/fourda-depengine to the same line`,
        );
      }
    }
  }
  return result;
}

function main(argv) {
  const args = [...argv];
  let basePath = null;
  const at = args.indexOf('--base-crate-lock');
  if (at !== -1) {
    basePath = args[at + 1];
    args.splice(at, 2);
  }
  const crateLock = args[0] || path.join(__dirname, '..', 'Cargo.lock');
  const appLock = args[1] || path.join(__dirname, '..', '..', '..', 'src-tauri', 'Cargo.lock');

  let crate;
  let app;
  let base = null;
  try {
    crate = parseLock(fs.readFileSync(crateLock, 'utf8'));
    app = parseLock(fs.readFileSync(appLock, 'utf8'));
    if (basePath) base = parseLock(fs.readFileSync(basePath, 'utf8'));
  } catch (e) {
    console.error(`::error::cannot read a lockfile: ${e.message}`);
    return 1;
  }
  if (crate.size === 0 || app.size === 0) {
    console.error(`::error::no packages parsed (crate ${crate.size}, app ${app.size})`);
    return 1;
  }
  if (base === null) {
    console.log('note no base crate lockfile given: drift is reported, never blamed');
  }

  const r = checkDrift(crate, app, base);
  for (const n of r.notes) console.log(`note ${n}`);
  for (const w of r.warnings) console.log(`::warning::${w}`);
  for (const e of r.errors) console.error(`::error::${e}`);
  const failed = r.errors.length > 0;
  console.log(
    `${failed ? 'FAIL' : 'ok  '} ${r.shared} packages in both lockfiles; ` +
      `${r.errors.length} crate-introduced drift(s), ${r.warnings.length} app-led drift warning(s)`,
  );
  return failed ? 1 : 0;
}

module.exports = { parseLock, checkDrift, majorMinor };

if (require.main === module) {
  process.exit(main(process.argv.slice(2)));
}
