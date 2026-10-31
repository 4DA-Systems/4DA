#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * check-tauri-alignment.cjs — catch a Tauri crate and its npm counterpart
 * drifting to different major.minor versions BEFORE a release build finds it.
 *
 * `tauri build` compares every Tauri crate it knows (`tauri`, and each official
 * `tauri-plugin-<name>`) against its npm package (`@tauri-apps/api`,
 * `@tauri-apps/plugin-<name>`) and refuses to build when major.minor differ:
 *
 *     Found version mismatched Tauri packages. Make sure the NPM package and
 *     Rust crate versions are on the same major/minor releases:
 *     tauri-plugin-updater (v2.14.0) : @tauri-apps/plugin-updater (v2.13.0)
 *
 * release.yml builds through tauri-action without `--ignore-version-mismatches`,
 * so a drift that lands on main does not fail the PR that caused it. It fails
 * the NEXT app release, after the tag is cut. That has happened twice: fixed in
 * #966 (2026-10-16), drifted again within 15 days (crate tauri-plugin-updater
 * 2.14 vs @tauri-apps/plugin-updater 2.13) and fixed in #1062 (2026-10-31).
 * The cause is structural: Dependabot opens the cargo and the npm bumps as
 * separate PRs, and each one merges alone.
 *
 * This check mirrors tauri-cli's own rule (crates/tauri-cli/src/info/plugins.rs):
 * a pair is compared only when BOTH sides exist. A crate whose npm package is not
 * a dependency of the root package.json (deep-link, autostart, notification and
 * single-instance today) is ignored, as tauri ignores it. Versions come from the
 * lockfiles, because that is what the build resolves: src-tauri/Cargo.lock and
 * the root importer of pnpm-lock.yaml. `@tauri-apps/cli` is reported against the
 * `tauri` crate as a note only; tauri itself does not compare it.
 *
 * Exit codes: 0 aligned, 1 at least one mismatch, 2 inconclusive (a lockfile is
 * missing, or a pair could not be resolved). `--json` prints the result as JSON.
 * `--annotate` adds GitHub `::warning::` annotations (used by the non-blocking
 * CI step in validate.yml; a hard gate would fail every one-sided Dependabot PR).
 */

const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.resolve(__dirname, '..');

const CARGO_LOCK = 'src-tauri/Cargo.lock';
const PNPM_LOCK = 'pnpm-lock.yaml';
const PACKAGE_JSON = 'package.json';
const CLI_PACKAGE = '@tauri-apps/cli';

/** The npm package tauri-cli pairs with a crate, or null for any other crate. */
function npmCounterpart(crateName) {
  if (crateName === 'tauri') return '@tauri-apps/api';
  const m = /^tauri-plugin-(.+)$/.exec(crateName);
  return m ? `@tauri-apps/plugin-${m[1]}` : null;
}

/** "2.14.1" -> "2.14"; null when the string does not start with a version. */
function majorMinor(version) {
  const m = /^(\d+)\.(\d+)(?:\.|$)/.exec(String(version || '').trim());
  return m ? `${Number(m[1])}.${Number(m[2])}` : null;
}

/** Compare "a.b" strings numerically: <0, 0, >0. */
function compareMajorMinor(a, b) {
  const [aMaj, aMin] = a.split('.').map(Number);
  const [bMaj, bMin] = b.split('.').map(Number);
  return aMaj - bMaj || aMin - bMin;
}

/**
 * Every package in a Cargo.lock: name -> list of versions (a list because a lock
 * can hold several versions of one crate).
 */
function parseCargoLock(text) {
  const crates = {};
  let name = null;
  for (const raw of String(text).split(/\r?\n/)) {
    const line = raw.trim();
    if (line === '[[package]]') {
      name = null;
      continue;
    }
    const nameM = /^name\s*=\s*"([^"]+)"$/.exec(line);
    if (nameM) {
      name = nameM[1];
      continue;
    }
    const verM = /^version\s*=\s*"([^"]+)"$/.exec(line);
    if (verM && name) {
      (crates[name] = crates[name] || []).push(verM[1]);
      name = null;
    }
  }
  return crates;
}

const unquote = (s) => s.trim().replace(/^'(.*)'$/, '$1').replace(/^"(.*)"$/, '$1');

/**
 * Resolved versions of the ROOT project's direct dependencies, from
 * pnpm-lock.yaml, by line parsing (no YAML dependency). Handles the lockfile v9
 * shape (`importers: / .: / dependencies: / name: / version: x`), and a value
 * written inline on the name line. A peer suffix `1.2.3(react@19.0.0)` is cut.
 */
function parsePnpmLockRoot(text) {
  const lines = String(text).split(/\r?\n/);
  const indentOf = (l) => l.length - l.trimStart().length;

  // Locate the root importer; lockfiles without `importers:` keep the root's
  // sections at the top level.
  let start = 0;
  let base = -2;
  const importersAt = lines.findIndex((l) => /^importers:\s*$/.test(l));
  if (importersAt !== -1) {
    const rootAt = lines.findIndex((l, i) => i > importersAt && /^ {2}(?:\.|'\.'|"\.")\s*:\s*$/.test(l));
    if (rootAt === -1) return {};
    start = rootAt + 1;
    base = 2;
  }

  const resolved = {};
  let section = null;
  let pkg = null;
  for (let i = start; i < lines.length; i++) {
    const line = lines[i];
    if (!line.trim() || line.trimStart().startsWith('#')) continue;
    const indent = indentOf(line);
    if (indent <= base) break; // next importer or next top-level key
    if (indent === base + 2) {
      const m = /^\s*([A-Za-z]+):\s*$/.exec(line);
      section = m && /^(dependencies|devDependencies|optionalDependencies)$/.test(m[1]) ? m[1] : null;
      pkg = null;
      continue;
    }
    if (!section) continue;
    if (indent === base + 4) {
      const m = /^\s*('[^']+'|"[^"]+"|[^\s:]+):\s*(.*)$/.exec(line);
      if (!m) continue;
      pkg = unquote(m[1]);
      if (m[2].trim()) resolved[pkg] = unquote(m[2]).split('(')[0];
      continue;
    }
    if (indent === base + 6 && pkg) {
      const m = /^\s*version:\s*(.+)$/.exec(line);
      if (m) resolved[pkg] = unquote(m[1]).split('(')[0];
    }
  }
  return resolved;
}

/** Names in the root package.json `dependencies` and `devDependencies`. */
function declaredFromPackageJson(json) {
  return [...Object.keys(json.dependencies || {}), ...Object.keys(json.devDependencies || {})];
}

/**
 * Pure core, so the tests can drive it without a filesystem.
 *
 * @param {Record<string, string|string[]>} crates  Cargo.lock: crate -> version(s)
 * @param {Record<string, string>} npmResolved      pnpm-lock.yaml root importer: package -> version
 * @param {Iterable<string>} declaredNpm            root package.json dependencies + devDependencies
 */
function evaluate(crates, npmResolved, declaredNpm) {
  const declared = new Set(declaredNpm || []);
  const resolved = npmResolved || {};
  const versionsOf = (name) => {
    const v = (crates || {})[name];
    return v == null ? [] : [...new Set(Array.isArray(v) ? v : [v])];
  };

  const pairs = [];
  const ignored = [];
  const notes = [];
  const pairedNpm = new Set();

  const crateNames = Object.keys(crates || {})
    .filter((c) => npmCounterpart(c))
    .sort((a, b) => (a === 'tauri' ? -1 : b === 'tauri' ? 1 : a.localeCompare(b)));

  for (const crate of crateNames) {
    const npm = npmCounterpart(crate);
    const versions = versionsOf(crate);
    if (!declared.has(npm)) {
      ignored.push({ crate, crateVersion: versions.join(', '), npm, reason: `${npm} is not a dependency in package.json` });
      continue;
    }
    pairedNpm.add(npm);
    const npmVersion = resolved[npm] || null;
    const pair = { crate, crateVersion: versions.join(', '), npm, npmVersion, status: 'aligned', reason: '' };
    if (versions.length !== 1) {
      // tauri-cli itself falls back to the Cargo.toml requirement here; this
      // check cannot tell which copy the build will compare, so it says so.
      pair.status = 'unknown';
      pair.reason = `${versions.length} versions of ${crate} in ${CARGO_LOCK}`;
    } else if (!npmVersion || !majorMinor(npmVersion)) {
      pair.status = 'unknown';
      pair.reason = `${npm} has no resolved version in the root importer of ${PNPM_LOCK} (run pnpm install)`;
    } else if (!majorMinor(versions[0])) {
      pair.status = 'unknown';
      pair.reason = `${crate} ${versions[0]} is not a version`;
    } else if (majorMinor(versions[0]) !== majorMinor(npmVersion)) {
      pair.status = 'mismatch';
      pair.reason = compareMajorMinor(majorMinor(versions[0]), majorMinor(npmVersion)) > 0 ? 'crate-ahead' : 'npm-ahead';
    }
    pairs.push(pair);
  }

  // A Tauri npm package with no crate behind it is not something `tauri build`
  // compares; it is reported so a removed plugin's leftover JS shows up.
  for (const npm of [...declared].sort()) {
    if (pairedNpm.has(npm)) continue;
    if (npm !== '@tauri-apps/api' && !npm.startsWith('@tauri-apps/plugin-')) continue;
    const crate = npm === '@tauri-apps/api' ? 'tauri' : `tauri-plugin-${npm.slice('@tauri-apps/plugin-'.length)}`;
    notes.push({ level: 'note', message: `${npm} is in package.json but ${crate} is not in ${CARGO_LOCK}; not compared` });
  }

  // The CLI is not part of tauri's own check: informational only, never a failure.
  const tauriVersions = versionsOf('tauri');
  const cliVersion = declared.has(CLI_PACKAGE) ? resolved[CLI_PACKAGE] || null : null;
  if (cliVersion && tauriVersions.length === 1 && majorMinor(cliVersion) && majorMinor(tauriVersions[0])) {
    const aligned = majorMinor(cliVersion) === majorMinor(tauriVersions[0]);
    notes.push({
      level: aligned ? 'note' : 'warning',
      message: aligned
        ? `${CLI_PACKAGE} ${cliVersion} and tauri ${tauriVersions[0]} share major.minor (informational)`
        : `${CLI_PACKAGE} ${cliVersion} vs tauri ${tauriVersions[0]}: different major.minor. tauri build does not refuse this, but keep them together`,
    });
  }

  const mismatches = pairs.filter((p) => p.status === 'mismatch');
  const unknown = pairs.filter((p) => p.status === 'unknown');
  return { ok: mismatches.length === 0, inconclusive: unknown.length > 0, pairs, mismatches, unknown, ignored, notes };
}

/** How to fix one mismatched pair, in the direction the drift went. */
function fixFor(pair) {
  const crateMM = majorMinor(pair.crateVersion);
  const npmMM = majorMinor(pair.npmVersion);
  if (pair.reason === 'npm-ahead') {
    return [
      `${pair.npm} (${npmMM}) moved ahead of the ${pair.crate} crate (${crateMM}). In the SAME PR, either:`,
      `  - bump the crate to ${npmMM}: check the version first (cargo search ${pair.crate}), then`,
      `    cd src-tauri && cargo update -p ${pair.crate}   (raise the Cargo.toml requirement if it caps it), or`,
      `  - hold ${pair.npm} at ${crateMM} in package.json, then pnpm install.`,
    ].join('\n');
  }
  return [
    `The ${pair.crate} crate (${crateMM}) moved ahead of ${pair.npm} (${npmMM}). In the SAME PR:`,
    `  1. Check the version first: npm view ${pair.npm}@~${crateMM}.0 version`,
    `     (it must be published; take the newest ${crateMM}.x it lists)`,
    `  2. Set "${pair.npm}": "^${crateMM}.<that patch>" in package.json`,
    `  3. pnpm install   (rewrites pnpm-lock.yaml; commit both files)`,
  ].join('\n');
}

/** The sentence a sentinel or annotation leads with. */
function summarize(result) {
  if (result.mismatches.length === 0) return '';
  const list = result.mismatches
    .map((p) => `${p.crate} ${majorMinor(p.crateVersion)} vs ${p.npm} ${majorMinor(p.npmVersion)}`)
    .join('; ');
  return `Tauri crate/npm major.minor mismatch (${list}): the next release build will fail`;
}

/** Load the three inputs through `readFile(relPath) -> string|null`. */
function loadInputs(readFile) {
  const missing = [];
  const read = (rel) => {
    const text = readFile(rel);
    if (text == null) missing.push(rel);
    return text;
  };
  const cargo = read(CARGO_LOCK);
  const pnpm = read(PNPM_LOCK);
  const pkgText = read(PACKAGE_JSON);
  let pkg = null;
  if (pkgText != null) {
    try {
      pkg = JSON.parse(pkgText);
    } catch {
      missing.push(`${PACKAGE_JSON} (invalid JSON)`);
    }
  }
  if (missing.length > 0) return { missing };
  return {
    missing,
    crates: parseCargoLock(cargo),
    npmResolved: parsePnpmLockRoot(pnpm),
    declaredNpm: declaredFromPackageJson(pkg),
  };
}

/** A reader over a directory on disk; null for a file that is not there. */
function fsReader(root = ROOT) {
  return (rel) => {
    try {
      return fs.readFileSync(path.join(root, rel), 'utf8');
    } catch {
      return null;
    }
  };
}

/** Run the whole check against one source of files. */
function check(readFile = fsReader()) {
  const inputs = loadInputs(readFile);
  if (inputs.missing.length > 0) {
    return { ok: true, inconclusive: true, missing: inputs.missing, pairs: [], mismatches: [], unknown: [], ignored: [], notes: [] };
  }
  return { missing: [], ...evaluate(inputs.crates, inputs.npmResolved, inputs.declaredNpm) };
}

function exitCodeFor(result) {
  if (!result.ok) return 1;
  return result.inconclusive ? 2 : 0;
}

// GitHub workflow-command escaping (data and property values).
const escData = (s) => String(s).replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A');
const escProp = (s) => escData(s).replace(/:/g, '%3A').replace(/,/g, '%2C');

function lineOf(text, needle) {
  const idx = String(text || '').split(/\r?\n/).findIndex((l) => l.includes(needle));
  return idx === -1 ? null : idx + 1;
}

function annotations(result, packageJsonText) {
  const out = [];
  for (const p of result.mismatches) {
    const line = lineOf(packageJsonText, `"${p.npm}"`);
    const where = `file=${PACKAGE_JSON}${line ? `,line=${line}` : ''}`;
    out.push(
      `::warning ${where},title=${escProp('Tauri version mismatch (next release build will fail)')}::` +
        escData(`${p.crate} ${p.crateVersion} vs ${p.npm} ${p.npmVersion}. tauri build refuses a major.minor mismatch.\n${fixFor(p)}`),
    );
  }
  if (result.missing.length > 0) {
    out.push(`::warning title=${escProp('Tauri alignment check inconclusive')}::${escData(`could not read ${result.missing.join(', ')}`)}`);
  }
  for (const p of result.unknown) {
    out.push(`::warning title=${escProp('Tauri alignment check inconclusive')}::${escData(`${p.crate} vs ${p.npm}: ${p.reason}`)}`);
  }
  for (const n of result.notes.filter((x) => x.level === 'warning')) {
    out.push(`::warning title=${escProp('Tauri CLI version')}::${escData(n.message)}`);
  }
  return out;
}

function render(result) {
  const tag = '[check-tauri-alignment]';
  const lines = [`${tag} Tauri crate <-> npm package, major.minor (tauri build refuses a mismatch)`];
  if (result.missing.length > 0) {
    lines.push(`  INCONCLUSIVE: could not read ${result.missing.join(', ')}`);
    return lines.join('\n');
  }
  const w = Math.max(0, ...result.pairs.map((p) => `${p.crate} ${p.crateVersion}`.length));
  for (const p of result.pairs) {
    const label = p.status === 'aligned' ? 'OK      ' : p.status === 'mismatch' ? 'MISMATCH' : 'UNKNOWN ';
    const left = `${p.crate} ${p.crateVersion}`.padEnd(w);
    const right = `${p.npm} ${p.npmVersion || '(unresolved)'}`;
    lines.push(`  ${label}  ${left}  <->  ${right}${p.status === 'unknown' ? `   (${p.reason})` : ''}`);
  }
  for (const i of result.ignored) lines.push(`  ignored   ${i.crate} ${i.crateVersion}  (${i.reason})`);
  for (const n of result.notes) lines.push(`  ${n.level.padEnd(8)}  ${n.message}`);

  if (result.mismatches.length > 0) {
    lines.push('');
    lines.push(`  ${summarize(result)}.`);
    lines.push('  release.yml builds through tauri-action without --ignore-version-mismatches,');
    lines.push('  so this fails the next app release from main, not the PR that caused it.');
    for (const p of result.mismatches) {
      lines.push('');
      lines.push(fixFor(p).replace(/^/gm, '  '));
    }
  } else if (result.inconclusive) {
    lines.push('');
    lines.push('  INCONCLUSIVE: at least one pair could not be compared (see UNKNOWN above).');
  } else {
    lines.push(`  aligned: ${result.pairs.length} pair(s) share major.minor`);
  }
  return lines.join('\n');
}

function main(argv = process.argv.slice(2)) {
  const result = check();
  const code = exitCodeFor(result);
  if (argv.includes('--json')) {
    console.log(JSON.stringify({ ...result, summary: summarize(result), exitCode: code }, null, 2));
  } else {
    (code === 0 ? console.log : console.error)(render(result));
  }
  if (argv.includes('--annotate')) {
    for (const a of annotations(result, fsReader()(PACKAGE_JSON))) console.log(a);
  }
  return code;
}

module.exports = {
  evaluate,
  check,
  loadInputs,
  fsReader,
  parseCargoLock,
  parsePnpmLockRoot,
  declaredFromPackageJson,
  npmCounterpart,
  majorMinor,
  summarize,
  fixFor,
  annotations,
  render,
  exitCodeFor,
  CARGO_LOCK,
  PNPM_LOCK,
  PACKAGE_JSON,
};

if (require.main === module) process.exit(main());
