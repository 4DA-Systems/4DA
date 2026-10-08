// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Edge-aware placement for NODE labels — pure geometry. The label layer
// offers each node label four sides (below, above, right, left); at the
// canvas edge this module keeps only the ones that stay fully on the visible
// canvas, sliding a label along its side when a flip alone does not fit, and
// suppresses the label of a mark that is itself mostly off-canvas (live
// 2026-10-07 at 1200x800 after zooming in: "openai v7.30.0", "rmcp v3.5.1",
// "tauri v2.12.1" printed half off the left edge beside marks at x < 24).

import type { LabelOffset } from './content-graph-label-collision';

export interface FitRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** How far (screen px) a slid label may sit past its mark's edge and still
 *  read as that mark's label. */
export const SLIDE_SLACK_PX = 8;

function inside(r: FitRect, b: FitRect): boolean {
  return r.x >= b.x && r.y >= b.y && r.x + r.w <= b.x + b.w && r.y + r.h <= b.y + b.h;
}

/** The shift that brings [start, start+len] inside [lo, hi], or null if it cannot fit. */
function clampShift(start: number, len: number, lo: number, hi: number): number | null {
  if (len > hi - lo) return null;
  if (start < lo) return lo - start;
  if (start + len > hi) return hi - (start + len);
  return 0;
}

/**
 * Node-label candidates that keep the label on the visible canvas.
 *
 * - A side that fits as-is is kept unchanged; these come first, so a flip
 *   is preferred to a slide.
 * - Below/above may SLIDE horizontally, right/left vertically, as long as the
 *   label still overlaps the mark's span (within SLIDE_SLACK_PX) — so it still
 *   reads as this node's label. The slide is carried as `sx`/`sy`, which the
 *   renderer applies on top of the side's CSS placement.
 * - Nothing fits and the mark's centre is off-canvas: [] — the resolver
 *   suppresses the label (the node's tooltip still has the title).
 * - Nothing fits but the mark is on-canvas (a label wider than the canvas):
 *   the original list — clipping is then unavoidable, and hiding a visible
 *   node's label would be worse.
 */
export function fitNodeLabelCandidates(
  box: FitRect,
  mark: FitRect,
  candidates: LabelOffset[],
  bounds: FitRect | undefined,
  scale: number,
): LabelOffset[] {
  if (!bounds) return candidates;
  const slack = SLIDE_SLACK_PX * scale;
  const out: LabelOffset[] = [];
  const slidOut: LabelOffset[] = [];
  for (const c of candidates) {
    const r = { x: box.x + c.dx, y: box.y + c.dy, w: box.w, h: box.h };
    if (inside(r, bounds)) {
      out.push(c);
      continue;
    }
    const horizontal = c.key === 'below' || c.key === 'above';
    const sx = horizontal ? clampShift(r.x, r.w, bounds.x, bounds.x + bounds.w) : 0;
    const sy = horizontal ? 0 : clampShift(r.y, r.h, bounds.y, bounds.y + bounds.h);
    if (sx === null || sy === null) continue;
    const slid = { ...r, x: r.x + sx, y: r.y + sy };
    if (!inside(slid, bounds)) continue;
    const nearMark = horizontal
      ? slid.x <= mark.x + mark.w + slack && slid.x + slid.w >= mark.x - slack
      : slid.y <= mark.y + mark.h + slack && slid.y + slid.h >= mark.y - slack;
    if (!nearMark) continue;
    slidOut.push({ dx: c.dx + sx, dy: c.dy + sy, key: c.key, sx, sy });
  }
  out.push(...slidOut);
  if (out.length > 0) return out;
  const cx = mark.x + mark.w / 2;
  const cy = mark.y + mark.h / 2;
  const markOnCanvas =
    cx >= bounds.x && cx <= bounds.x + bounds.w && cy >= bounds.y && cy <= bounds.y + bounds.h;
  return markOnCanvas ? candidates : [];
}
