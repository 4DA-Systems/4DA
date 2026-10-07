// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Short, human-readable project labels from absolute project paths.
 *
 * Shared by Preemption and Blind Spots. The Blind Spots row used to show only
 * the LAST path segment, so two repos' Tauri crates both read
 * "src-tauri, src-tauri" (audit 2026-10-07). Labels are the last two segments
 * and grow leftwards only where two paths would otherwise collide.
 */

function segments(fullPath: string): string[] {
  return fullPath.replace(/\\/g, '/').split('/').filter(Boolean);
}

/** The last two path segments ("4da/src-tauri"), or the whole short path. */
export function shortenProjectPath(fullPath: string): string {
  return segments(fullPath).slice(-2).join('/');
}

/**
 * Distinct labels for a set of project paths, in input order, deduplicated.
 * Two different paths never share a label: a colliding pair gains leading
 * segments until it differs ("bridge/src-tauri" vs "relay/src-tauri", or
 * "tools/apps/bridge/src-tauri" when "apps/bridge/src-tauri" still collides).
 */
export function formatProjectNames(paths: string[]): string[] {
  const unique = Array.from(new Set(paths.map((p) => segments(p).join('/')))).filter(Boolean);
  const depth = new Map<string, number>(unique.map((p) => [p, 2]));
  const label = (p: string) => segments(p).slice(-(depth.get(p) ?? 2)).join('/');
  for (let guard = 0; guard < 64; guard++) {
    const byLabel = new Map<string, string[]>();
    for (const p of unique) {
      const l = label(p);
      byLabel.set(l, [...(byLabel.get(l) ?? []), p]);
    }
    let grew = false;
    for (const group of byLabel.values()) {
      if (group.length < 2) continue;
      for (const p of group) {
        const d = depth.get(p) ?? 2;
        if (d < segments(p).length) {
          depth.set(p, d + 1);
          grew = true;
        }
      }
    }
    if (!grew) break;
  }
  return Array.from(new Set(unique.map(label)));
}
