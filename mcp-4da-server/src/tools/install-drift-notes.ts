// SPDX-License-Identifier: Apache-2.0
/**
 * Words for install drift (node_modules holding a different version than the
 * lockfile pins), shared by vulnerability_scan, what_should_i_know,
 * get_actionable_signals, dependency_health and upgrade_planner so every
 * surface says the same thing about the same drifted package.
 *
 * The case this exists for (measured 2026-09-10): the lockfile pinned the
 * patched hono 4.13.5 while node_modules still held the vulnerable 4.13.1.
 * "Upgrade hono to 4.13.4" would be wrong advice there, because the lockfile
 * is already past it. The fix is a reinstall, and the output has to say so.
 */

import { isActionableVulnerability } from "../live/maintenance.js";
import { presentedSeverity, SEVERITY_RANK, type SeverityTier } from "../live/severity-scope.js";
import type { InstallDriftRecord, VulnerabilityEntry, VulnerabilityScanResult } from "../live/types.js";

const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();

/** A relative directory label for a sentence: the scan root reads as words, not ".". */
export function placeLabel(label: string): string {
  return label === "." ? "the project root" : label;
}

/** The directories a row names, spelled for a message. */
export function dirsLabel(dirs: string[], relative: (dir: string) => string = (d) => d.replace(/\\/g, "/")): string {
  const labels = [...new Set(dirs.map(relative))].map(placeLabel).sort();
  return labels.length > 0 ? labels.join(", ") : "the project";
}

/** Drift records for one npm lockfile instance (package at its lockfile version). */
export function driftFor(drift: InstallDriftRecord[], name: string, lockfileVersion: string | null): InstallDriftRecord[] {
  return drift.filter((r) => r.package === name && r.lockfileVersion === lockfileVersion);
}

/** The action for a scan row about an installed version the lockfile does not pin. */
export function driftAction(v: VulnerabilityEntry): string {
  const command = v.installFix ?? "a reinstall";
  return `Run \`${command}\` in ${dirsLabel(v.sourceDirs)} — node_modules has ${v.package}@${v.currentVersion}; the lockfile pins ${v.installDriftOf}`;
}

/** Whether the lockfile's own version of a drifted package carries this same advisory. */
export function lockfileAlsoAffected(v: VulnerabilityEntry, all: VulnerabilityEntry[]): boolean {
  if (!v.installDriftOf) return false;
  const ids = new Set([v.vulnId, ...v.aliases]);
  return all.some(
    (x) =>
      x.package === v.package &&
      x.ecosystem === v.ecosystem &&
      x.currentVersion === v.installDriftOf &&
      isActionableVulnerability(x) &&
      [x.vulnId, ...x.aliases].some((id) => ids.has(id)),
  );
}

const topTier = (entries: VulnerabilityEntry[]): SeverityTier =>
  entries.map(presentedSeverity).reduce<SeverityTier>(
    (a, b) => (SEVERITY_RANK[b] > SEVERITY_RANK[a] ? b : a),
    "unknown",
  );

/** Group the actionable drift rows of a scan per (package, installed, locked). */
export function groupDriftRows(vulns: VulnerabilityEntry[]): VulnerabilityEntry[][] {
  const groups = new Map<string, VulnerabilityEntry[]>();
  for (const v of vulns) {
    if (!v.installDriftOf || !isActionableVulnerability(v)) continue;
    const key = `${v.ecosystem}\0${v.package}\0${v.currentVersion}\0${v.installDriftOf}`;
    const list = groups.get(key);
    if (list) list.push(v);
    else groups.set(key, [v]);
  }
  return [...groups.values()];
}

export interface DriftAdvisory {
  title: string;
  signal_type: "security_alert";
  priority: "critical" | "high" | "medium";
  action: string;
  url: null;
}

/**
 * One briefing advisory per drifted package whose installed copy OSV lists as
 * vulnerable, graded the way the app grades its install-drift Preemption row
 * (`evidence::install_drift`): high when the reinstall clears an advisory the
 * running copy has, medium when the lockfile's pinned version is exposed too
 * (the reinstall alone is then not the fix, and each advisory's own signal
 * carries its grade). Always a security alert, so the verdict is at least
 * review_needed and a vulnerable installed copy is never reported clean.
 *
 * Measured 2026-09-10: the hono incident's three advisories are medium on
 * their own, and the installed copy stayed vulnerable for 25 days while a
 * one-line reinstall would have cleared all three.
 */
export function installDriftAdvisories(scan: VulnerabilityScanResult): DriftAdvisory[] {
  return groupDriftRows(scan.vulnerabilities).map((entries) => {
    const first = entries[0];
    const count = entries.length;
    const pinExposed = entries.some((e) => lockfileAlsoAffected(e, scan.vulnerabilities));
    const clearsAny = entries.some((e) => !lockfileAlsoAffected(e, scan.vulnerabilities));
    return {
      title: `${first.package}: the installed ${first.currentVersion} in node_modules has ${count} known vulnerabilit${count === 1 ? "y" : "ies"}; the lockfile pins ${first.installDriftOf}`,
      signal_type: "security_alert",
      priority: clearsAny ? "high" : "medium",
      action: `${driftAction(first)}${
        !pinExposed
          ? " (the lockfile version is not affected; the reinstall is the whole fix)"
          : clearsAny
            ? " (the reinstall clears some of these; the lockfile version is still affected by the rest, so upgrade it too)"
            : " (the lockfile version is also affected; upgrade it, then reinstall)"
      }`,
      url: null,
    };
  });
}

/** The `install_drift` rows and per-vulnerability annotations for one scan. */
export interface DriftView {
  rows: Array<Record<string, unknown>>;
  annotate(v: VulnerabilityEntry): Record<string, unknown>;
}

export function buildDriftView(
  result: VulnerabilityScanResult,
  drift: InstallDriftRecord[],
  includeDev: boolean,
  relative: (dir: string) => string,
): DriftView {
  const actionable = result.vulnerabilities.filter(isActionableVulnerability);
  // OSV's answer for one npm (package, version), whichever dependency asked it.
  const affected = (pkg: string, version: string) =>
    actionable.some((v) => v.ecosystem === "npm" && v.package === pkg && v.currentVersion === version);
  // Null when the version was not in the scanned set (a dev dependency without
  // include_dev) or the scan went offline with nothing to say about it.
  const answer = (record: InstallDriftRecord, version: string): boolean | null => {
    if (record.isDev && !includeDev) return null;
    if (affected(record.package, version)) return true;
    return result.offline ? null : false;
  };

  const rows = drift.map((record) => {
    const vulnerableInstalled = answer(record, record.installedVersion);
    const lockfileVulnerable = answer(record, record.lockfileVersion);
    const dir = relative(record.dir);
    const where = placeLabel(dir);
    let note: string;
    if (vulnerableInstalled === true && lockfileVulnerable === false) {
      note = `The lockfile is patched but node_modules is not: run \`${record.fix}\` in ${where}.`;
    } else if (vulnerableInstalled === true) {
      note = `The installed copy is vulnerable and so is the lockfile's version: upgrade, then run \`${record.fix}\` in ${where}.`;
    } else if (vulnerableInstalled === false) {
      note = `node_modules is out of step with the lockfile (no known advisory affects the installed version): run \`${record.fix}\` in ${where}.`;
    } else {
      const why = record.isDev && !includeDev ? "a dev dependency; pass include_dev to check it" : "the scan was offline";
      note = `node_modules is out of step with the lockfile; the installed version was not checked (${why}): run \`${record.fix}\` in ${where}.`;
    }
    return {
      package: record.package,
      dir,
      lockfile_version: record.lockfileVersion,
      installed_version: record.installedVersion,
      vulnerable_installed: vulnerableInstalled,
      lockfile_version_vulnerable: lockfileVulnerable,
      fix: record.fix,
      note,
    };
  });

  const inDirs = (dir: string, dirs: string[]) => dirs.some((d) => norm(d) === norm(dir));

  const annotate = (v: VulnerabilityEntry): Record<string, unknown> => {
    if (v.installDriftOf) {
      const command = v.installFix ?? "a reinstall";
      const where = dirsLabel(v.sourceDirs, relative);
      return {
        installed_version: v.currentVersion,
        lockfile_version: v.installDriftOf,
        install_note: lockfileAlsoAffected(v, result.vulnerabilities)
          ? `This is the copy in node_modules; the lockfile's ${v.installDriftOf} is also affected — upgrade, then run \`${command}\` in ${where}.`
          : `This is the copy in node_modules: the lockfile is patched (pins ${v.installDriftOf}) but node_modules still has ${v.currentVersion} — run \`${command}\` in ${where}.`,
      };
    }
    if (v.ecosystem !== "npm") return {};
    // A lockfile row whose package node_modules holds at another version.
    const behind = drift.filter(
      (r) => r.package === v.package && r.lockfileVersion === v.currentVersion && inDirs(r.dir, v.sourceDirs),
    );
    if (behind.length > 0) {
      const installed = [...new Set(behind.map((r) => r.installedVersion))].join(", ");
      return {
        installed_version: installed,
        install_note: `node_modules has ${installed}, not this lockfile version — run \`${behind[0].fix}\` in ${dirsLabel(behind.map((r) => r.dir), relative)}; advisories against the installed copy are listed under its own version.`,
      };
    }
    // A lockfile row that is also what a drifted workspace has installed.
    const holders = drift.filter(
      (r) => r.package === v.package && r.installedVersion === v.currentVersion && inDirs(r.dir, v.sourceDirs),
    );
    if (holders.length > 0) {
      return {
        installed_version: v.currentVersion,
        lockfile_version: [...new Set(holders.map((r) => r.lockfileVersion))].join(", "),
        install_note: `Also the copy node_modules holds in ${dirsLabel(holders.map((r) => r.dir), relative)}, where the lockfile pins a different version — run \`${holders[0].fix}\` there.`,
      };
    }
    return {};
  };

  return { rows, annotate };
}
