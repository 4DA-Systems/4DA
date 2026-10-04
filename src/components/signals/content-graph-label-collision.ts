// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Generic label collision resolver — pure geometry on boxes, no React, no DOM.
// The content graph builds its boxes in content-graph-label-layout.ts.

/** A candidate displacement for a label, in flow units. `key` names it for
 *  the renderer (node labels flip `below` / `above` via a data attribute). */
export interface LabelOffset {
  dx: number;
  dy: number;
  key: string;
}

/** One label's box at its default placement (top-left + size, flow units). */
export interface LabelBox {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** Higher places first; a lower label never displaces a higher one. */
  priority: number;
  /** Placements to try, in preference order. The first is the default. */
  candidates: LabelOffset[];
  /** How much a higher label should avoid covering this one's default spot
   *  while it still waits to be placed (default 1). */
  weight?: number;
  /** A soft obstacle, not a label: never placed or reported, but every label
   *  pays `weight` for covering it — so labels step off it when they can. */
  obstacle?: boolean;
}

export interface LabelPlacement {
  visible: boolean;
  offset: LabelOffset;
}

export const DEFAULT_OFFSET: LabelOffset = { dx: 0, dy: 0, key: 'default' };

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export function rectsOverlap(a: Rect, b: Rect, gap = 0): boolean {
  return (
    a.x < b.x + b.w + gap &&
    b.x < a.x + a.w + gap &&
    a.y < b.y + b.h + gap &&
    b.y < a.y + a.h + gap
  );
}

/** Cost of moving one step down a label's candidate list, relative to
 *  covering one lower-priority label's default spot (cost 1). Keeps a label
 *  at its natural place unless moving actually frees a neighbour. */
const CANDIDATE_STEP_COST = 0.15;

/**
 * Greedy priority placement: labels are placed highest priority first (ties by
 * id, so the result is deterministic). Each takes, among its candidates that
 * overlap nothing already placed, the one that covers the fewest default
 * spots of the labels still waiting (by their weight), with a
 * small penalty per step away from its preferred placement — so a cluster
 * header nudges off its own stack members' labels instead of erasing them. A
 * label with no free candidate is suppressed; a suppressed node label stays
 * reachable — hovering the node opens its tooltip with the full title.
 * `gap` is the minimum free space between two labels.
 */
export function resolveLabelCollisions(
  boxes: LabelBox[],
  gap = 0,
): Map<string, LabelPlacement> {
  const obstacles = boxes.filter((b) => b.obstacle);
  const order = boxes
    .filter((b) => !b.obstacle)
    .sort((a, b) => b.priority - a.priority || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  const cands = order.map((b) => (b.candidates.length > 0 ? b.candidates : [DEFAULT_OFFSET]));
  // Waiting labels' default rects, weighted by how much they matter.
  const pending: Array<Rect & { weight: number }> = order.map((b, i) => ({
    x: b.x + cands[i]![0]!.dx,
    y: b.y + cands[i]![0]!.dy,
    w: b.w,
    h: b.h,
    weight: b.weight ?? 1,
  }));
  // Two labels can only ever touch if the areas their candidates span do:
  // a one-off neighbour list keeps each pass near-linear on sparse maps.
  const reach: Rect[] = order.map((b, i) => {
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const c of cands[i]!) {
      x0 = Math.min(x0, b.x + c.dx);
      y0 = Math.min(y0, b.y + c.dy);
      x1 = Math.max(x1, b.x + c.dx + b.w);
      y1 = Math.max(y1, b.y + c.dy + b.h);
    }
    return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
  });
  const neighbours: number[][] = order.map(() => []);
  for (let i = 0; i < order.length; i++) {
    for (let j = i + 1; j < order.length; j++) {
      if (rectsOverlap(reach[i]!, reach[j]!, gap)) {
        neighbours[i]!.push(j);
        neighbours[j]!.push(i);
      }
    }
  }
  const placed: Array<Rect | null> = order.map(() => null);
  const out = new Map<string, LabelPlacement>();
  order.forEach((box, idx) => {
    const near = neighbours[idx]!;
    let best: { c: LabelOffset; r: Rect; cost: number } | null = null;
    cands[idx]!.forEach((c, ci) => {
      const r = { x: box.x + c.dx, y: box.y + c.dy, w: box.w, h: box.h };
      let cost = ci * CANDIDATE_STEP_COST;
      if (best && cost >= best.cost) return;
      for (const j of near) {
        const p = placed[j];
        if (p && rectsOverlap(p, r, gap)) return;
      }
      for (const j of near) {
        if (j > idx && rectsOverlap(pending[j]!, r, gap)) cost += pending[j]!.weight;
      }
      for (const o of obstacles) {
        if (rectsOverlap(o, r)) cost += o.weight ?? 1;
      }
      if (!best || cost < best.cost) best = { c, r, cost };
    });
    const chosen = best as { c: LabelOffset; r: Rect; cost: number } | null;
    if (chosen) {
      placed[idx] = chosen.r;
      out.set(box.id, { visible: true, offset: chosen.c });
    } else {
      out.set(box.id, { visible: false, offset: cands[idx]![0]! });
    }
  });
  return out;
}

/** Pairs of boxes that overlap at their default placement (or at the given
 *  placements, counting visible labels only). The verification metric. */
export function countOverlaps(boxes: LabelBox[], placements?: Map<string, LabelPlacement>): number {
  const rects: Rect[] = [];
  for (const b of boxes) {
    if (b.obstacle) continue;
    const p = placements?.get(b.id);
    if (p && !p.visible) continue;
    const o = p?.offset ?? DEFAULT_OFFSET;
    rects.push({ x: b.x + o.dx, y: b.y + o.dy, w: b.w, h: b.h });
  }
  let n = 0;
  for (let i = 0; i < rects.length; i++) {
    for (let j = i + 1; j < rects.length; j++) {
      if (rectsOverlap(rects[i]!, rects[j]!)) n++;
    }
  }
  return n;
}
