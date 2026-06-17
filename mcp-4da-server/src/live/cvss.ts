// SPDX-License-Identifier: Apache-2.0
/**
 * CVSS v3.0 / v3.1 base-score calculator.
 *
 * OSV advisories frequently expose severity only as a CVSS vector string
 * (e.g. "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:L") rather than a numeric
 * base score. Without computing the score we cannot bucket the advisory into
 * critical/high/medium/low — so this implements the published base-score
 * formula. CVSS v4.0 vectors are not yet handled (returns null).
 */

const AV: Record<string, number> = { N: 0.85, A: 0.62, L: 0.55, P: 0.2 };
const AC: Record<string, number> = { L: 0.77, H: 0.44 };
const UI: Record<string, number> = { N: 0.85, R: 0.62 };
const CIA: Record<string, number> = { N: 0, L: 0.22, H: 0.56 };
// Privileges Required is scope-dependent.
const PR_UNCHANGED: Record<string, number> = { N: 0.85, L: 0.62, H: 0.27 };
const PR_CHANGED: Record<string, number> = { N: 0.85, L: 0.68, H: 0.5 };

/** CVSS v3.1 "Roundup": smallest one-decimal value >= input (with float-safe integer math). */
function roundUp(input: number): number {
  const intInput = Math.round(input * 100000);
  if (intInput % 10000 === 0) return intInput / 100000;
  return (Math.floor(intInput / 10000) + 1) / 10;
}

/**
 * Parse a CVSS vector string and return its base score (0.0–10.0), or null if
 * the vector is not a recognizable v3.x base vector (missing required metrics,
 * v2/v4, or malformed).
 */
export function cvssBaseScore(vector: string): number | null {
  if (!vector || !/CVSS:3\.[01]/i.test(vector) && !vector.includes("AV:")) return null;

  const metrics: Record<string, string> = {};
  for (const part of vector.split("/")) {
    const [k, v] = part.split(":");
    if (k && v) metrics[k.trim().toUpperCase()] = v.trim().toUpperCase();
  }

  const scope = metrics.S; // U (unchanged) | C (changed)
  const prTable = scope === "C" ? PR_CHANGED : PR_UNCHANGED;

  const av = AV[metrics.AV];
  const ac = AC[metrics.AC];
  const pr = prTable[metrics.PR];
  const ui = UI[metrics.UI];
  const c = CIA[metrics.C];
  const i = CIA[metrics.I];
  const a = CIA[metrics.A];

  // All eight base metrics are mandatory; bail if any is unrecognized.
  if ([av, ac, pr, ui, c, i, a].some((x) => x === undefined) || (scope !== "U" && scope !== "C")) {
    return null;
  }

  const iscBase = 1 - (1 - c) * (1 - i) * (1 - a);
  const impact =
    scope === "C"
      ? 7.52 * (iscBase - 0.029) - 3.25 * Math.pow(iscBase - 0.02, 15)
      : 6.42 * iscBase;

  if (impact <= 0) return 0;

  const exploitability = 8.22 * av * ac * pr * ui;
  const raw = scope === "C" ? 1.08 * (impact + exploitability) : impact + exploitability;
  return roundUp(Math.min(raw, 10));
}
