// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';

import {
  buildLabelBoxes,
  countOverlaps,
  estimateTextWidth,
  layoutGraphLabels,
  rectsOverlap,
  resolveLabelCollisions,
  DEFAULT_OFFSET,
  HEADER_RIM_INSET,
  type LabelBox,
  type LabelLayoutInput,
  type NodeLabelInput,
} from './content-graph-label-layout';

function box(id: string, x: number, y: number, priority: number, over: Partial<LabelBox> = {}): LabelBox {
  return { id, x, y, w: 100, h: 10, priority, candidates: [DEFAULT_OFFSET], ...over };
}

describe('resolveLabelCollisions', () => {
  it('keeps every label when nothing overlaps', () => {
    const boxes = [box('a', 0, 0, 1), box('b', 0, 50, 1), box('c', 200, 0, 1)];
    const out = resolveLabelCollisions(boxes);
    expect([...out.values()].every((p) => p.visible)).toBe(true);
    expect(countOverlaps(boxes, out)).toBe(0);
  });

  it('suppresses the LOWER-priority label of an overlapping pair', () => {
    const out = resolveLabelCollisions([box('low', 10, 2, 1), box('high', 0, 0, 5)]);
    expect(out.get('high')!.visible).toBe(true);
    expect(out.get('low')!.visible).toBe(false);
  });

  it('moves a label to a free candidate instead of suppressing it', () => {
    const moved = box('low', 10, 2, 1, {
      candidates: [DEFAULT_OFFSET, { dx: 0, dy: 30, key: 'below' }],
    });
    const out = resolveLabelCollisions([box('high', 0, 0, 5), moved]);
    expect(out.get('low')).toEqual({ visible: true, offset: { dx: 0, dy: 30, key: 'below' } });
  });

  it('respects the minimum gap between labels', () => {
    const boxes = [box('a', 0, 0, 2), box('b', 0, 12, 1)]; // 2 units apart
    expect(resolveLabelCollisions(boxes, 0).get('b')!.visible).toBe(true);
    expect(resolveLabelCollisions(boxes, 4).get('b')!.visible).toBe(false);
  });

  it('a higher label prefers a placement that leaves room for those below it', () => {
    // 'high' may sit at 0 (covering 'low') or move to 100 (covering nothing).
    const high = box('high', 0, 0, 5, {
      candidates: [DEFAULT_OFFSET, { dx: 0, dy: 100, key: 'down' }],
      w: 50,
    });
    const low = box('low', 0, 0, 1, { w: 50, weight: 2 });
    const out = resolveLabelCollisions([high, low]);
    expect(out.get('high')!.offset.key).toBe('down');
    expect(out.get('low')!.visible).toBe(true);
  });

  it('steps off a soft obstacle (a stack mark) when a free placement exists', () => {
    const mark = box('mark:1', 0, 0, 0, { w: 20, h: 20, obstacle: true, weight: 3 });
    const label = box('lab', 0, 5, 1, { candidates: [DEFAULT_OFFSET, { dx: 0, dy: 40, key: 'below' }] });
    const out = resolveLabelCollisions([mark, label]);
    expect(out.has('mark:1')).toBe(false); // obstacles are never reported
    expect(out.get('lab')!.offset.key).toBe('below');
    // …but never suppresses a label that has nowhere else to go.
    const stuck = resolveLabelCollisions([mark, box('lab', 0, 5, 1)]);
    expect(stuck.get('lab')!.visible).toBe(true);
    expect(countOverlaps([mark, box('lab', 0, 5, 1)])).toBe(0); // obstacles are not labels
  });

  it('never lets a label cover a HARD obstacle (a story badge)', () => {
    const badge = box('badge:1', 0, 0, 0, { w: 20, h: 16, obstacle: true, hard: true });
    const out = resolveLabelCollisions([badge, box('lab', 0, 5, 1)]);
    expect(out.get('lab')!.visible).toBe(false);
    const moved = resolveLabelCollisions([
      badge,
      box('lab', 0, 5, 1, { candidates: [DEFAULT_OFFSET, { dx: 0, dy: 40, key: 'below' }] }),
    ]);
    expect(moved.get('lab')!.offset.key).toBe('below');
  });

  it('is deterministic and order-independent', () => {
    const boxes = [box('b', 5, 0, 1), box('a', 0, 0, 1), box('c', 300, 0, 1)];
    const one = resolveLabelCollisions(boxes);
    const two = resolveLabelCollisions([...boxes].reverse());
    expect([...one.entries()].sort()).toEqual([...two.entries()].sort());
    // Equal priority: the id tie-break keeps 'a'.
    expect(one.get('a')!.visible).toBe(true);
    expect(one.get('b')!.visible).toBe(false);
  });

  it('touching edges are not an overlap', () => {
    expect(rectsOverlap({ x: 0, y: 0, w: 10, h: 10 }, { x: 10, y: 0, w: 10, h: 10 })).toBe(false);
    expect(rectsOverlap({ x: 0, y: 0, w: 10, h: 10 }, { x: 9, y: 9, w: 10, h: 10 })).toBe(true);
  });
});

/** A dense 150-node map shaped like the live one: tight clusters of long
 *  release titles (the tauri-plugin pile), headers, and a lane. */
function denseInput(zoom: number): LabelLayoutInput {
  const nodes: NodeLabelInput[] = [];
  const headers = [];
  for (let c = 0; c < 15; c++) {
    const cx = (c % 5) * 400;
    const cy = Math.floor(c / 5) * 400;
    headers.push({ id: `cluster-${c}`, x: cx, y: cy - 30, label: `topic ${c} · rust · tauri`, count: 8, radius: 220 });
    for (let k = 0; k < 8; k++) {
      nodes.push({
        id: `${c}-${k}`,
        x: cx + (k % 3) * 90 - 90,
        y: cy + Math.floor(k / 3) * 90 - 90,
        title: `crates.io: tauri-plugin-notification-${c}-${k} v2.5.1`,
        memberCount: 1,
        relevance: 0.5 + k / 20,
        stack: k % 2 === 0,
        security: k === 7,
      });
    }
  }
  for (let i = 0; i < 30; i++) {
    nodes.push({ id: `lane-${i}`, x: (i % 12) * 95, y: 1400 + Math.floor(i / 12) * 95, title: `Unconnected item ${i}`, memberCount: 1, relevance: 0.6, stack: i < 3, security: false });
  }
  return {
    zoom,
    includeNonStack: zoom >= 0.75,
    nodes,
    headers,
    lane: { x: 0, y: 1400, text: 'Not connected to any theme · 30' },
    measure: estimateTextWidth,
  };
}

describe('graph label layout', () => {
  it.each([0.17, 0.3, 0.5, 0.8, 1, 1.6])('leaves zero overlapping visible labels at zoom %s', (zoom) => {
    const input = denseInput(zoom);
    const boxes = buildLabelBoxes(input);
    const placements = layoutGraphLabels(input);
    expect(countOverlaps(boxes)).toBeGreaterThan(0); // the fixture IS dense
    expect(countOverlaps(boxes, placements)).toBe(0);
  });

  it('never suppresses the lane header, and ranks headers over stack over plain labels', () => {
    const placements = layoutGraphLabels(denseInput(0.17));
    expect(placements.get('lane')!.visible).toBe(true);
    const shown = (prefix: string, pred: (id: string) => boolean = () => true) => {
      const all = [...placements.entries()].filter(([id]) => id.startsWith(prefix) && pred(id));
      return all.filter(([, p]) => p.visible).length / Math.max(all.length, 1);
    };
    expect(shown('cluster:')).toBeGreaterThanOrEqual(shown('node:'));
  });

  it('models every "+N" story badge as a hard obstacle at the badge position', () => {
    const input = denseInput(0.4);
    input.nodes[0] = { ...input.nodes[0]!, memberCount: 3 };
    const badges = buildLabelBoxes(input).filter((b) => b.id.startsWith('badge:'));
    expect(badges).toHaveLength(1);
    expect(badges[0]!.hard && badges[0]!.obstacle).toBe(true);
    const placements = layoutGraphLabels(input);
    const labels = buildLabelBoxes(input).filter((b) => !b.obstacle);
    for (const l of labels) {
      const p = placements.get(l.id)!;
      if (!p.visible) continue;
      const r = { x: l.x + p.offset.dx, y: l.y + p.offset.dy, w: l.w, h: l.h };
      expect(rectsOverlap(r, badges[0]!)).toBe(false);
    }
  });

  it('a header may step just outside its hull when the inside is taken', () => {
    const header = buildLabelBoxes(denseInput(0.17)).find((b) => b.id === 'cluster:cluster-0')!;
    const dys = header.candidates.map((c) => c.dy);
    // The anchor sits HEADER_RIM_INSET under the rim (radius 220): one
    // fallback clears the rim above, one the rim below.
    expect(Math.min(...dys) + header.h).toBeLessThanOrEqual(-HEADER_RIM_INSET);
    expect(Math.max(...dys)).toBeGreaterThan(2 * 220 - HEADER_RIM_INSET);
    // Every in-hull step goes DOWN from the name band, never up out of it.
    const inHull = header.candidates.slice(0, -2).map((c) => c.dy);
    expect(Math.min(...inHull)).toBeGreaterThanOrEqual(0);
  });

  it('the lane header hangs ABOVE its first row', () => {
    const lane = buildLabelBoxes(denseInput(0.17)).find((b) => b.id === 'lane')!;
    expect(lane.y + lane.h).toBeLessThan(1400);
  });

  it('far zoom only places the labels level-of-detail shows (headers, lane, stack)', () => {
    const ids = buildLabelBoxes(denseInput(0.3)).map((b) => b.id);
    expect(ids.some((id) => /^node:\d+-1$/.test(id))).toBe(false); // cluster member k=1 is non-stack
    expect(ids).not.toContain('node:lane-5'); // non-stack lane item
    expect(ids).toContain('node:0-0');
  });

  it('places a 150-node map well inside one frame', () => {
    const input = denseInput(1);
    const t0 = performance.now();
    for (let i = 0; i < 20; i++) layoutGraphLabels(input);
    const perPass = (performance.now() - t0) / 20;
    expect(perPass).toBeLessThan(16);
  });
});

describe('labels stay on the visible canvas', () => {
  // 1200x800 window: the graph canvas is ~1150 px wide; at zoom 1 flow
  // units are screen px. Live 2026-10-07: "tauri-plugin-autostart …" and
  // "A Function-level Datase…" were cut at the left/right edge.
  const bounds = { x: 0, y: 0, w: 1150, h: 520 };
  const node = (id: string, x: number, y: number, title: string): NodeLabelInput => ({
    id, x, y, title, memberCount: 1, relevance: 0.9, stack: true, security: false,
  });
  const input = (nodes: NodeLabelInput[]): LabelLayoutInput => ({
    zoom: 1, includeNonStack: true, nodes, headers: [], lane: null, measure: estimateTextWidth, bounds,
  });

  it('flips a label off the left, right and bottom edges onto the canvas', () => {
    const nodes = [
      node('left', 2, 200, 'tauri-plugin-autostart v2.5.1'),
      node('right', 1150 - 40, 200, 'A Function-level Dataset for code'),
      node('bottom', 500, 520 - 50, 'tauri-plugin-notification v2'),
    ];
    const boxes = buildLabelBoxes(input(nodes)).filter((b) => !b.obstacle);
    const placements = layoutGraphLabels(input(nodes));
    for (const b of boxes) {
      const p = placements.get(b.id)!;
      expect(p.visible).toBe(true);
      const r = { x: b.x + p.offset.dx, y: b.y + p.offset.dy, w: b.w, h: b.h };
      expect(r.x).toBeGreaterThanOrEqual(bounds.x);
      expect(r.y).toBeGreaterThanOrEqual(bounds.y);
      expect(r.x + r.w).toBeLessThanOrEqual(bounds.x + bounds.w);
      expect(r.y + r.h).toBeLessThanOrEqual(bounds.y + bounds.h);
    }
    expect(placements.get('node:left')!.offset.key).toBe('right');
    expect(placements.get('node:right')!.offset.key).toBe('left');
    expect(placements.get('node:bottom')!.offset.key).toBe('above');
  });

  it('keeps the default placement when it already fits', () => {
    const placements = layoutGraphLabels(input([node('mid', 500, 200, 'serde v1')]));
    expect(placements.get('node:mid')!.offset.key).toBe('below');
  });

  it('without bounds behaves as before', () => {
    const nodes = [node('left', 2, 200, 'tauri-plugin-autostart v2.5.1')];
    const placements = layoutGraphLabels({ ...input(nodes), bounds: undefined });
    expect(placements.get('node:left')!.offset.key).toBe('below');
  });
});
