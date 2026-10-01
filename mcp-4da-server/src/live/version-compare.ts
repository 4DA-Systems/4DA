// SPDX-License-Identifier: Apache-2.0
/**
 * Version ordering per ecosystem, for advisory ranges and fix selection.
 *
 * `compareSemver` reads MAJOR.MINOR.PATCH and calls anything else "equal".
 * That is wrong outside npm and crates.io, and it was wrong in practice
 * (measured 2026-10-02 against pip-audit and osv-scanner):
 * - PyPI versions are PEP 440, not semver. `5.1` and `4.2` are ordinary
 *   releases; read as "equal" to everything, django 3.2.0 was sent to the 5.1
 *   line's fix (5.1.1) instead of its own line's (3.2.25 / 4.2.16).
 * - Go versions carry a `v`, OSV's Go ranges do not, and pseudo-versions
 *   (`v0.0.0-20200622213623-75b288015ac9`) order as semver prereleases.
 *
 * `0` is OSV's "since the beginning" bound and orders below every version.
 */

import type { OsvEcosystem } from "./types.js";
import { compareSemver, parseSemver } from "./semver-utils.js";
import { compareVersionPrecedence, parseSemverPrecedence } from "./semver-precedence.js";

// ---------------------------------------------------------------------------
// PEP 440
// ---------------------------------------------------------------------------

interface Pep440 {
  epoch: number;
  release: number[];
  /** Pre-release: [rank, n] with a=0, b=1, rc=2; null for none. */
  pre: [number, number] | null;
  post: number | null;
  dev: number | null;
}

const PEP440 =
  /^v?(?:(\d+)!)?(\d+(?:\.\d+)*)(?:[-_.]?(a|alpha|b|beta|c|rc|pre|preview)[-_.]?(\d*))?(?:(?:[-_.]?(?:post|rev|r)[-_.]?(\d*))|-(\d+))?(?:[-_.]?dev[-_.]?(\d*))?(?:\+[a-z0-9]+(?:[-_.][a-z0-9]+)*)?$/i;

export function parsePep440(version: string): Pep440 | null {
  const m = PEP440.exec(version.trim());
  if (!m) return null;
  const preRank: Record<string, number> = { a: 0, alpha: 0, b: 1, beta: 1, c: 2, rc: 2, pre: 2, preview: 2 };
  return {
    epoch: m[1] ? Number(m[1]) : 0,
    release: m[2].split(".").map(Number),
    pre: m[3] ? [preRank[m[3].toLowerCase()], m[4] ? Number(m[4]) : 0] : null,
    post: m[5] !== undefined ? (m[5] ? Number(m[5]) : 0) : m[6] !== undefined ? Number(m[6]) : null,
    dev: m[7] !== undefined ? (m[7] ? Number(m[7]) : 0) : null,
  };
}

export function comparePep440(a: Pep440, b: Pep440): number {
  if (a.epoch !== b.epoch) return a.epoch > b.epoch ? 1 : -1;
  const len = Math.max(a.release.length, b.release.length);
  for (let i = 0; i < len; i++) {
    const x = a.release[i] ?? 0;
    const y = b.release[i] ?? 0;
    if (x !== y) return x > y ? 1 : -1;
  }
  // PEP 440's sort key within one release:
  //   X.devN < X.aN.devM < X.aN < X.aN.postM < X.bN < X.rcN < X < X.postN.devM < X.postN
  // pre: a/b/rc rank, a dev-only release below every pre-release, a final above;
  // then post (none lowest); then dev (none highest).
  const key = (v: Pep440): number[] => [
    v.pre ? v.pre[0] : v.post === null && v.dev !== null ? -1 : 3,
    v.pre ? v.pre[1] : 0,
    v.post === null ? -1 : v.post,
    v.dev === null ? Number.POSITIVE_INFINITY : v.dev,
  ];
  const ka = key(a);
  const kb = key(b);
  for (let i = 0; i < ka.length; i++) {
    if (ka[i] !== kb[i]) return ka[i] > kb[i] ? 1 : -1;
  }
  return 0;
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

const stripGoV = (v: string) => v.replace(/^v(?=\d)/, "");

/** Whether `version` is readable for this ecosystem's ordering. */
export function isComparable(version: string, ecosystem: OsvEcosystem | string): boolean {
  if (version === "0") return true;
  switch (ecosystem) {
    case "PyPI":
      return parsePep440(version) !== null;
    case "Go":
      return parseSemverPrecedence(stripGoV(version)) !== null;
    case "npm":
    case "crates.io":
      return parseSemverPrecedence(version) !== null;
    default:
      return parseSemver(version) !== null;
  }
}

/**
 * Order two versions of one ecosystem: 1, -1, or 0 for equal OR unreadable
 * (the same contract as compareSemver, so an unreadable bound never decides).
 */
export function compareVersions(a: string, b: string, ecosystem: OsvEcosystem | string): number {
  if (a === b) return 0;
  if (a === "0") return isComparable(b, ecosystem) ? -1 : 0;
  if (b === "0") return isComparable(a, ecosystem) ? 1 : 0;
  switch (ecosystem) {
    case "PyPI": {
      const pa = parsePep440(a);
      const pb = parsePep440(b);
      return pa && pb ? comparePep440(pa, pb) : 0;
    }
    case "Go":
      return compareVersionPrecedence(stripGoV(a), stripGoV(b)) ?? 0;
    case "npm":
    case "crates.io":
      return compareVersionPrecedence(a, b) ?? compareSemver(a, b);
    default:
      return compareSemver(a, b);
  }
}

/** Highest of `versions` in this ecosystem's order, or null when empty. */
export function maxVersion(versions: string[], ecosystem: OsvEcosystem | string): string | null {
  if (versions.length === 0) return null;
  return versions.reduce((max, v) => (compareVersions(v, max, ecosystem) > 0 ? v : max), versions[0]);
}

/** Package-name equality as the ecosystem's registry defines it. */
export function samePackageName(a: string, b: string, ecosystem: OsvEcosystem | string): boolean {
  if (a === b) return true;
  if (ecosystem === "PyPI") {
    const n = (s: string) => s.toLowerCase().replace(/[-_.]+/g, "-");
    return n(a) === n(b);
  }
  if (ecosystem === "crates.io") {
    const n = (s: string) => s.toLowerCase().replace(/_/g, "-");
    return n(a) === n(b);
  }
  return false;
}
