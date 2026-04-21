// SPDX-License-Identifier: FSL-1.1-Apache-2.0

import type { SemverDistance } from "./types.js";

export function parseSemver(version: string): [number, number, number] | null {
  const match = version.replace(/^v/, "").match(/^(\d+)\.(\d+)\.(\d+)/);
  if (!match) return null;
  return [parseInt(match[1]), parseInt(match[2]), parseInt(match[3])];
}

export function computeSemverDistance(current: string, latest: string): SemverDistance | null {
  const c = parseSemver(current);
  const l = parseSemver(latest);
  if (!c || !l) return null;

  const major = l[0] - c[0];
  const minor = major === 0 ? l[1] - c[1] : 0;
  const patch = major === 0 && minor === 0 ? l[2] - c[2] : 0;

  let label: SemverDistance["label"] = "up-to-date";
  if (major > 0) label = "major";
  else if (minor > 0) label = "minor";
  else if (patch > 0) label = "patch";

  return { major: Math.max(0, major), minor: Math.max(0, minor), patch: Math.max(0, patch), label };
}

export function isPreRelease(version: string): boolean {
  return /[-+]/.test(version.replace(/^v/, "").replace(/^\d+\.\d+\.\d+/, "").slice(0, 1));
}
