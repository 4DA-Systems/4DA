#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Dismiss open Dependabot alerts raised against the vendored conformance
 * corpus (src-tauri/tests/conformance/**).
 *
 * Those lockfiles are pinned to known-vulnerable versions ON PURPOSE: they are
 * the dependency engine's test fixtures (#903), not dependencies of 4DA. GitHub's
 * dependency graph still alerts on them — 3,664 of 3,665 open alerts on
 * 2026-10-11, which buried the one real alert. `dependabot.yml exclude-paths`
 * stops update PRs, not alerts, and a GITHUB_TOKEN cannot write Dependabot
 * alerts, so this runs with the caller's own `gh` login.
 *
 * Dismissal is reversible (reopen the alert). New advisories against the pinned
 * fixture versions raise new alerts over time; the sentinel scan reports when
 * any are open.
 *
 * Usage: node scripts/dismiss-fixture-alerts.cjs [--execute]   (dry run without --execute)
 */

const { execFileSync } = require("child_process");

const REPO = "repos/4DA-Systems/4DA";
const FIXTURE_PREFIX = "src-tauri/tests/conformance/";
// GitHub caps dismissed_comment at 280 characters.
const COMMENT =
  "Vendored test fixture (dependency-engine conformance corpus, src-tauri/tests/conformance). " +
  "Pinned on purpose so the scanner is tested against known-vulnerable versions; not a dependency of 4DA.";

function isFixtureManifest(manifestPath) {
  return typeof manifestPath === "string" && manifestPath.startsWith(FIXTURE_PREFIX);
}

/** Split `{ number, manifest, severity, pkg }` rows into fixture and real alerts. */
function splitAlerts(rows) {
  const fixture = [];
  const real = [];
  for (const r of rows) (isFixtureManifest(r.manifest) ? fixture : real).push(r);
  return { fixture, real };
}

/** Every open alert as `{ number, manifest, severity, pkg }`. Throws if gh fails. */
function listOpenAlerts(gh = (args) => execFileSync("gh", args, { encoding: "utf8", maxBuffer: 256 * 1024 * 1024, windowsHide: true })) {
  const out = gh([
    "api",
    "--paginate",
    `${REPO}/dependabot/alerts?state=open&per_page=100`,
    "--jq",
    '.[] | [.number, .dependency.manifest_path, .security_advisory.severity, .dependency.package.name] | @tsv',
  ]);
  return out
    .split("\n")
    .filter(Boolean)
    .map((line) => {
      const [number, manifest, severity, pkg] = line.split("\t");
      return { number: Number(number), manifest, severity, pkg };
    });
}

function main() {
  const execute = process.argv.includes("--execute");
  const { fixture, real } = splitAlerts(listOpenAlerts());
  console.log(`open alerts: ${fixture.length + real.length} (fixture ${fixture.length}, real ${real.length})`);
  for (const r of real.slice(0, 10)) console.log(`  real: #${r.number} ${r.pkg} ${r.severity} ${r.manifest}`);
  if (!execute) {
    if (fixture.length) console.log("dry run: pass --execute to dismiss the fixture alerts");
    return 0;
  }
  let done = 0;
  let failed = 0;
  for (const a of fixture) {
    try {
      execFileSync(
        "gh",
        ["api", "-X", "PATCH", `${REPO}/dependabot/alerts/${a.number}`, "-f", "state=dismissed",
          "-f", "dismissed_reason=not_used", "-f", `dismissed_comment=${COMMENT}`, "--silent"],
        { windowsHide: true },
      );
      done++;
    } catch (e) {
      failed++;
      console.error(`#${a.number}: ${String(e.stderr || e.message).slice(0, 200)}`);
    }
  }
  console.log(`dismissed ${done}, failed ${failed}`);
  return failed ? 1 : 0;
}

if (require.main === module) {
  process.exit(main());
} else {
  module.exports = { FIXTURE_PREFIX, COMMENT, isFixtureManifest, splitAlerts, listOpenAlerts };
}
