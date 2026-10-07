// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import {
  TILE_GAP,
  layoutRows,
  stableThemeLayout,
  themeWeights,
  type LayoutTheme,
  type Rect,
  type RememberedLayout,
} from './theme-map-layout';

// ThemeMapView's frame, px (see the component's styles): the view is
// 100vh - 210 (min 500); the row has pt-2 / pb-3 and px-4; the stack column
// is 280 wide plus gap-3; the theme column stacks a 24px legend, the
// treemap host and a 36px unthemed bar with gap-2. The host has margin -4,
// so its box is the flex slot grown by TILE_GAP / 2 on every side.
const LEGEND_H = 24;
const BAR_H = 36;
const COL_GAP = 8;

function frame(vw: number, vh: number) {
  const viewH = Math.max(500, vh - 210);
  const colH = viewH - 8 - 12 - 40; // row padding + footer
  const colW = vw - 32 - 280 - 12;
  const slotH = colH - LEGEND_H - BAR_H - 2 * COL_GAP;
  const host: Rect = { x: 0, y: 0, w: colW + TILE_GAP, h: slotH + TILE_GAP };
  // In host coordinates: the slot ends TILE_GAP/2 above the host's bottom,
  // then the column gap, then the bar.
  const bar: Rect = { x: TILE_GAP / 2, y: host.h - TILE_GAP / 2 + COL_GAP, w: colW, h: BAR_H };
  return { host, bar };
}

/** The box a tile actually paints (inset by half the gap per side). */
const painted = (r: Rect): Rect => ({
  x: r.x + TILE_GAP / 2,
  y: r.y + TILE_GAP / 2,
  w: Math.max(0, r.w - TILE_GAP),
  h: Math.max(0, r.h - TILE_GAP),
});

const intersects = (a: Rect, b: Rect) =>
  a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

// 19 themes sized like the live 7-day window.
const COUNTS = [14, 12, 11, 9, 8, 8, 7, 6, 6, 5, 5, 4, 4, 3, 3, 3, 2, 2, 2];
const ids = COUNTS.map((_, i) => `cluster_${1000 + i}`);
const members = COUNTS.map((n, i) => Array.from({ length: n }, (_, k) => i * 100 + k));

function themesFor(area: number): LayoutTheme[] {
  const ws = themeWeights(COUNTS, area);
  return COUNTS.map((_, i) => ({ id: ids[i]!, members: members[i]!, weight: ws[i]! }));
}

/** A remembered layout as found live on 2026-10-07: 23 slots, 19 keys. */
const CORRUPT: RememberedLayout = {
  rows: [
    [ids[0]!, ids[1]!, ids[2]!, ids[0]!],
    [ids[3]!, ids[4]!, ids[5]!, ids[6]!, ids[1]!],
    [ids[7]!, ids[8]!, ids[9]!, ids[10]!, ids[11]!],
    [ids[12]!, ids[13]!, ids[14]!, ids[15]!, ids[2]!],
    [ids[16]!, ids[17]!, ids[18]!, ids[16]!],
  ],
  members: Object.fromEntries(ids.map((id, i) => [id, members[i]!])),
};

describe('theme map never paints over the unthemed bar', () => {
  for (const [vw, vh] of [
    [1200, 800],
    [1440, 900],
    [1700, 1184],
  ] as const) {
    for (const [name, prev] of [
      ['fresh', null],
      ['remembered', CORRUPT],
    ] as const) {
      it(`${vw}x${vh}, ${name} layout`, () => {
        const { host, bar } = frame(vw, vh);
        const themes = themesFor(host.w * host.h);
        const { rects, remembered } = stableThemeLayout(themes, host, prev);
        expect(rects).toHaveLength(themes.length);
        for (const r of rects) {
          const p = painted(r);
          expect(intersects(p, bar)).toBe(false);
          expect(p.y + p.h).toBeLessThanOrEqual(host.h + 1e-6);
          expect(r.h).toBeGreaterThan(TILE_GAP); // every tile has a body
        }
        // The layout written back never repeats a key.
        const flat = remembered.rows.flat();
        expect(new Set(flat).size).toBe(flat.length);
        expect(flat).toHaveLength(themes.length);
      });
    }
  }
});

describe('stable layout keys', () => {
  it('gives an unmatched theme a distinct key when its id is already a matched key', () => {
    const prev: RememberedLayout = { rows: [['a', 'b']], members: { a: [1, 2, 3], b: [4, 5, 6] } };
    const themes: LayoutTheme[] = [
      { id: 'x', members: [1, 2, 3], weight: 3 }, // matches remembered "a"
      { id: 'a', members: [9, 10], weight: 2 }, // new theme whose id is "a"
      { id: 'b', members: [4, 5, 6], weight: 3 },
    ];
    const { remembered } = stableThemeLayout(themes, { x: 0, y: 0, w: 900, h: 500 }, prev);
    const flat = remembered.rows.flat();
    expect(new Set(flat).size).toBe(3);
    expect(Object.keys(remembered.members)).toHaveLength(3);
  });
});

describe('layoutRows', () => {
  it('stays inside the rect even when rows repeat an index', () => {
    const rect = { x: 0, y: 0, w: 600, h: 400 };
    const out = layoutRows([[0, 1], [2, 0]], [1, 1, 1], rect);
    for (const r of out) {
      expect(r.y).toBeGreaterThanOrEqual(0);
      expect(r.y + r.h).toBeLessThanOrEqual(400 + 1e-9);
      expect(r.h).toBeGreaterThan(0);
    }
  });
});
