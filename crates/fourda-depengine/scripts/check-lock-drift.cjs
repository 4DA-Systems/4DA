#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// The engine runs from two lockfiles. Inside the app it is a path dependency,
// so `src-tauri/Cargo.lock` picks its serde / serde_json / semver / regex;
// the standalone build (CI tests, the wasm module) uses this crate's own
// `Cargo.lock`. Those must not run different rules: semver parses every
// version the matcher orders, regex reads the non-semver schemes, and
// serde_json writes the stored ranges and the wasm API's answers.
//
// Policy: every package in BOTH lockfiles must share major.minor (minor
// releases of these crates can change parsing; a lockfile on 1.13 and the
// other on 1.14 is a real fork of the engine). A patch-level difference is
// reported, not failed: Dependabot bumps the two lockfiles on its own
// schedule, patch releases of these crates are fixes within the same
// behaviour, and the wasm == native corpus check (check-wasm.cjs) still has
// to pass on the crate's own lockfile.
//
// Usage: node check-lock-drift.cjs [crate Cargo.lock] [app Cargo.lock]
// Exit 1 on a major.minor drift or an unreadable lockfile.

'use strict';

const fs = require('node:fs');
const path = require('node:path');

const crateLock = process.argv[2] || path.join(__dirname, '..', 'Cargo.lock');
const appLock =
  process.argv[3] || path.join(__dirname, '..', '..', '..', 'src-tauri', 'Cargo.lock');

/** name -> Set of versions, from a Cargo.lock's [[package]] tables. */
function packages(file) {
  const text = fs.readFileSync(file, 'utf8');
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

let crate;
let app;
try {
  crate = packages(crateLock);
  app = packages(appLock);
} catch (e) {
  console.error(`::error::cannot read a lockfile: ${e.message}`);
  process.exit(1);
}
if (crate.size === 0 || app.size === 0) {
  console.error(`::error::no packages parsed (crate ${crate.size}, app ${app.size})`);
  process.exit(1);
}

let failed = false;
let shared = 0;
for (const [name, versions] of [...crate].sort(([a], [b]) => a.localeCompare(b))) {
  if (name === 'fourda-depengine' || !app.has(name)) continue;
  shared++;
  const appVersions = [...app.get(name)];
  for (const v of versions) {
    if (appVersions.includes(v)) continue;
    // The app may hold several majors of a crate; compare with the one on
    // the same major.minor line if there is one.
    const sameLine = appVersions.find((a) => majorMinor(a) === majorMinor(v));
    if (sameLine) {
      console.log(`note ${name}: crate ${v}, app ${sameLine} (patch drift, allowed)`);
    } else {
      console.error(
        `::error::${name}: crate ${v}, app ${appVersions.join(' / ')} (major.minor drift)`,
      );
      failed = true;
    }
  }
}
console.log(
  `${failed ? 'FAIL' : 'ok  '} ${shared} packages in both lockfiles; ` +
    `${failed ? 'major.minor drift found' : 'no major.minor drift'}`,
);
process.exit(failed ? 1 : 0);
