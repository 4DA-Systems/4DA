#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * check-pnpm-overrides.cjs — stop a pnpm upgrade silently disarming every
 * security override in the repo.
 *
 * This repo pins `packageManager: pnpm@9.x` and carries its dependency-security
 * floors as `pnpm.overrides` blocks in package.json — 69 of them across four
 * manifests at the time of writing, including the ones holding `undici`,
 * `form-data` and `tmp` above known-vulnerable ranges.
 *
 * **pnpm 11 does not read the `pnpm` field in package.json at all.** Run any
 * pnpm 11 command here and it says so out loud:
 *
 *     [WARN] The "pnpm" field in package.json is no longer read by pnpm.
 *            The following keys were ignored: "pnpm.overrides", ...
 *
 * A warning, not an error — so bumping the pin to 11 would resolve every
 * overridden package back to its vulnerable version, regenerate the lockfiles
 * happily, and leave the audit green until the next advisory sweep. The floors
 * would be gone and nothing would have failed.
 *
 * This gate makes that upgrade fail loudly instead. It is deliberately inert
 * while the pin stays on pnpm <11: it is a tripwire for a future change, not a
 * complaint about the present one.
 */

const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.resolve(__dirname, '..');

/** Manifests that may legitimately carry overrides. */
const MANIFESTS = [
  'package.json',
  'site/package.json',
  'paddle-webhook/package.json',
  'mcp-4da-server/package.json',
  'mcp-memory-server/package.json',
];

/** The pnpm major that stopped reading the `pnpm` field in package.json. */
const PNPM_FIELD_DROPPED_IN_MAJOR = 11;

function pnpmMajorFrom(packageManagerField) {
  if (!packageManagerField) return null;
  const m = /^pnpm@(\d+)\./.exec(String(packageManagerField).trim());
  return m ? Number(m[1]) : null;
}

/**
 * Pure core, so the test can drive it without a filesystem.
 *
 * @param {number|null} pnpmMajor
 * @param {Array<{file: string, count: number}>} usages
 * @returns {{ok: boolean, message: string}}
 */
function evaluate(pnpmMajor, usages) {
  const total = usages.reduce((n, u) => n + u.count, 0);

  if (pnpmMajor === null) {
    return {
      ok: false,
      message:
        'Could not read a pnpm major from `packageManager` in package.json. ' +
        'This gate needs it to know whether pnpm.overrides is still honoured.',
    };
  }

  if (pnpmMajor < PNPM_FIELD_DROPPED_IN_MAJOR) {
    return {
      ok: true,
      message:
        `OK — pinned to pnpm ${pnpmMajor}.x, which still reads the \`pnpm\` field. ` +
        `${total} override(s) across ${usages.length} manifest(s) are live.`,
    };
  }

  if (total === 0) {
    return {
      ok: true,
      message: `OK — pnpm ${pnpmMajor}.x is pinned and no manifest relies on \`pnpm.overrides\`.`,
    };
  }

  const list = usages.map((u) => `    - ${u.file} (${u.count} override(s))`).join('\n');
  return {
    ok: false,
    message:
      `pnpm ${pnpmMajor}.x DOES NOT READ the \`pnpm\` field in package.json, but ` +
      `${total} security override(s) are still declared there:\n${list}\n\n` +
      '  Every one of those floors is now inert. Packages held above a vulnerable\n' +
      '  range will resolve straight back down, the lockfiles will regenerate\n' +
      '  without complaint, and nothing else in CI will notice.\n\n' +
      '  Move them to the `overrides` key in each project\'s pnpm-workspace.yaml\n' +
      '  (pnpm 11\'s new home for this setting), regenerate every lockfile, and\n' +
      '  re-run the advisory audit before landing the pin bump.',
  };
}

function collectUsages() {
  const usages = [];
  for (const rel of MANIFESTS) {
    const full = path.join(ROOT, rel);
    if (!fs.existsSync(full)) continue;
    let json;
    try {
      json = JSON.parse(fs.readFileSync(full, 'utf8'));
    } catch {
      continue;
    }
    const count = Object.keys((json.pnpm && json.pnpm.overrides) || {}).length;
    if (count > 0) usages.push({ file: rel, count });
  }
  return usages;
}

function main() {
  const root = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'));
  const result = evaluate(pnpmMajorFrom(root.packageManager), collectUsages());
  if (result.ok) {
    console.log(`[check-pnpm-overrides] ${result.message}`);
    return 0;
  }
  console.error(`[check-pnpm-overrides] ${result.message}`);
  return 1;
}

module.exports = { evaluate, pnpmMajorFrom, PNPM_FIELD_DROPPED_IN_MAJOR };

if (require.main === module) process.exit(main());
