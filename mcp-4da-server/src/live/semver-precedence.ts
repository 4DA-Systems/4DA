// SPDX-License-Identifier: Apache-2.0
/**
 * Semantic-version PRECEDENCE (semver.org section 11) for advisory range checks.
 *
 * `compareSemver` compares MAJOR.MINOR.PATCH only. That is the right tool for
 * picking the highest of several fix versions and the wrong one for deciding
 * whether an install sits inside an advisory range: it reads `5.0.0-rc.1` and
 * `5.0.0-rc.2` as EQUAL, so an install at 5.0.0-rc.1 was judged outside
 * `[5.0.0-beta.1, 5.0.0-rc.2)` — a range GHSA-82fw-gwwq-j7x9 really stores for
 * vitest. Range checks use this comparator; `compareSemver` keeps its
 * behaviour for its other callers.
 *
 * Parsing follows the Rust reference (`src-tauri/src/osv/matching.rs`
 * `parse_version`, built on the `semver` crate): a leading `v` is dropped,
 * `MAJOR.MINOR` reads as `MAJOR.MINOR.0`, and anything else that is not
 * strict semver is unreadable (null) rather than guessed at.
 */

/** A parsed version, ordered by semver precedence. */
export interface SemverPrecedence {
  major: number;
  minor: number;
  patch: number;
  /** Dot-separated prerelease identifiers; empty for a release. */
  prerelease: string[];
}

const IDENT = "[0-9A-Za-z-]+";
const STRICT = new RegExp(
  `^(\\d+)\\.(\\d+)\\.(\\d+)(?:-(${IDENT}(?:\\.${IDENT})*))?(?:\\+${IDENT}(?:\\.${IDENT})*)?$`,
);
const TWO_PART = /^(\d+)\.(\d+)$/;

/** Parse a version for precedence comparison, or null when it is not readable semver. */
export function parseSemverPrecedence(version: string): SemverPrecedence | null {
  const v = version.trim().replace(/^v+/, "");
  const strict = STRICT.exec(v);
  if (strict) {
    return {
      major: Number(strict[1]),
      minor: Number(strict[2]),
      patch: Number(strict[3]),
      prerelease: strict[4] ? strict[4].split(".") : [],
    };
  }
  const two = TWO_PART.exec(v);
  return two ? { major: Number(two[1]), minor: Number(two[2]), patch: 0, prerelease: [] } : null;
}

/**
 * Prerelease identifiers: numeric ones compare numerically and rank below
 * alphanumeric ones; alphanumeric ones compare in ASCII order.
 */
function compareIdentifier(a: string, b: string): number {
  const aNumeric = /^\d+$/.test(a);
  const bNumeric = /^\d+$/.test(b);
  if (aNumeric && bNumeric) {
    // Compared as digit strings: an identifier may exceed Number's safe range.
    const x = a.replace(/^0+(?=\d)/, "");
    const y = b.replace(/^0+(?=\d)/, "");
    if (x.length !== y.length) return x.length > y.length ? 1 : -1;
    return x === y ? 0 : x > y ? 1 : -1;
  }
  if (aNumeric) return -1;
  if (bNumeric) return 1;
  return a === b ? 0 : a > b ? 1 : -1;
}

/** Semver precedence: 1 if a > b, -1 if a < b, 0 if equal. Build metadata is ignored. */
export function comparePrecedence(a: SemverPrecedence, b: SemverPrecedence): number {
  if (a.major !== b.major) return a.major > b.major ? 1 : -1;
  if (a.minor !== b.minor) return a.minor > b.minor ? 1 : -1;
  if (a.patch !== b.patch) return a.patch > b.patch ? 1 : -1;
  // A release outranks every prerelease of the same MAJOR.MINOR.PATCH.
  if (a.prerelease.length === 0 || b.prerelease.length === 0) {
    return Math.sign(b.prerelease.length - a.prerelease.length);
  }
  const shared = Math.min(a.prerelease.length, b.prerelease.length);
  for (let i = 0; i < shared; i++) {
    const cmp = compareIdentifier(a.prerelease[i], b.prerelease[i]);
    if (cmp !== 0) return cmp;
  }
  // A longer identifier list outranks its own prefix (1.0.0-alpha < 1.0.0-alpha.1).
  return Math.sign(a.prerelease.length - b.prerelease.length);
}

/** Precedence of two version strings, or null when either is unreadable. */
export function compareVersionPrecedence(a: string, b: string): number | null {
  const pa = parseSemverPrecedence(a);
  const pb = parseSemverPrecedence(b);
  return pa && pb ? comparePrecedence(pa, pb) : null;
}
