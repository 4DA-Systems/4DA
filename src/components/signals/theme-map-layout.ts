// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Theme map layout — pure geometry, no React, no DOM: tile weights, the
// ordered strip treemap, and the remembered layout that keeps tiles where the
// user last saw them.

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Tile typography, px — shared by the tile and its capacity estimate. */
export const TILE_PAD = 10;
export const TILE_HEADER_H = 22;
export const TILE_LINE_H = 20;
/** Space between tiles, px: each tile draws inset by half of it per side,
 *  so a layout box is TILE_GAP larger than the tile painted in it. */
export const TILE_GAP = 8;
/** Shortest tile worth drawing: its header and two titles. */
export const TILE_MIN_HEIGHT = 2 * TILE_PAD + TILE_HEADER_H + 2 * TILE_LINE_H;
/** Width ÷ height the strip layout aims for: tiles hold lines of text, so
 *  wider than square reads best. */
export const TILE_TARGET_ASPECT = 1.8;
/** Narrowest tile a title reads in, px; the layout avoids anything narrower. */
export const TILE_MIN_WIDTH = 150;
/** Layout-box area the smallest theme is guaranteed: the minimum tile box
 *  (gap included) plus 20% slack. 50% slack (#855) tipped 20 themes at
 *  1200×800 into the equal-weights fallback, so a 12-item theme drew the
 *  same 199×89 tile as a 2-item one (audit 2026-10-06). */
export const MIN_TILE_AREA = 1.2 * (TILE_MIN_WIDTH + TILE_GAP) * (TILE_MIN_HEIGHT + TILE_GAP + 1);

/**
 * Tile weights: member count plus one shared offset `c`, chosen so the
 * SMALLEST theme still gets `minArea` of the `area` available. Pure count
 * weights gave a 2-item theme a 65px-wide sliver at 1194×800. A common
 * offset keeps the order of sizes — bigger themes still get bigger tiles —
 * and the exact count is printed on every tile. Only when the area cannot
 * hold every theme at the minimum do all tiles get equal weight.
 */
export function themeWeights(counts: number[], area: number, minArea = MIN_TILE_AREA): number[] {
  const k = counts.length;
  if (k === 0) return [];
  const sum = counts.reduce((a, b) => a + b, 0);
  const smallest = Math.min(...counts);
  const r = area > 0 ? minArea / area : 1;
  if (r * k >= 1) return counts.map(() => 1);
  // (smallest + c) / (sum + k·c) >= r  ⇔  c >= (r·sum − smallest) / (1 − r·k)
  const c = Math.max(1, (r * sum - smallest) / (1 - r * k));
  return counts.map((n) => n + c);
}

interface Shape {
  targetAspect: number;
  minWidth: number;
  minHeight: number;
}

const DEFAULT_SHAPE: Shape = {
  targetAspect: TILE_TARGET_ASPECT,
  minWidth: TILE_MIN_WIDTH + TILE_GAP,
  minHeight: TILE_MIN_HEIGHT + TILE_GAP + 1,
};

/** Cost of one row holding tiles of these weights: each tile's squared
 *  distance from the target aspect, plus heavy penalties for a tile too
 *  narrow for a title or a row too short for two titles. */
function rowCost(rowWeights: number[], rect: Rect, scale: number, shape: Shape): number {
  const sum = rowWeights.reduce((a, b) => a + b, 0);
  if (sum <= 0) return Infinity;
  const h = (sum * scale) / rect.w;
  let acc = h < shape.minHeight ? 100 * rowWeights.length * (shape.minHeight / Math.max(h, 1)) ** 2 : 0;
  for (const wt of rowWeights) {
    const w = (wt * scale) / h;
    const aspect = w / h;
    const off = aspect > shape.targetAspect ? aspect / shape.targetAspect : shape.targetAspect / Math.max(aspect, 1e-9);
    acc += off * off;
    if (w < shape.minWidth) acc += 100 * (shape.minWidth / Math.max(w, 1)) ** 2;
  }
  return acc;
}

const scaleOf = (ws: number[], rect: Rect): number => (rect.w * rect.h) / ws.reduce((a, b) => a + b, 0);

/** Optimal row breaks for items in list order (dynamic programming, as in
 *  justified text — a greedy pass left the last row as a stretched sliver). */
export function optimalRows(ws: number[], rect: Rect, shape: Shape = DEFAULT_SHAPE): { rows: number[][]; cost: number } {
  const n = ws.length;
  if (n === 0) return { rows: [], cost: 0 };
  const scale = scaleOf(ws, rect);
  const best: number[] = [0];
  const from: number[] = [0];
  for (let j = 1; j <= n; j++) {
    best[j] = Infinity;
    for (let i = 0; i < j; i++) {
      const c = best[i]! + rowCost(ws.slice(i, j), rect, scale, shape);
      if (c < best[j]!) {
        best[j] = c;
        from[j] = i;
      }
    }
  }
  const rows: number[][] = [];
  for (let j = n; j > 0; j = from[j]!) {
    const i = from[j]!;
    rows.unshift(Array.from({ length: j - i }, (_, k) => i + k));
  }
  return { rows, cost: best[n]! };
}

/** Total cost of a given row structure (rows of item indexes). */
export function rowsCost(rows: number[][], ws: number[], rect: Rect, shape: Shape = DEFAULT_SHAPE): number {
  const scale = scaleOf(ws, rect);
  return rows.reduce((acc, r) => acc + rowCost(r.map((i) => ws[i]!), rect, scale, shape), 0);
}

/** Boxes for a row structure: rows fill the width, heights follow weight,
 *  and the last row / last tile take the remainder so no gap is left. */
export function layoutRows(rows: number[][], ws: number[], rect: Rect): Rect[] {
  const out: Rect[] = new Array(ws.length);
  // Heights share out the weight of the rows actually laid out — never more
  // than rect.h, even if a caller's rows repeat an index (audit 2026-10-07:
  // duplicated rows overflowed the map onto the unthemed bar).
  const total = rows.reduce((acc, r) => acc + r.reduce((a, i) => a + ws[i]!, 0), 0);
  let y = rect.y;
  rows.forEach((r, ri) => {
    const sum = r.reduce((a, i) => a + ws[i]!, 0);
    const h = ri === rows.length - 1 ? rect.y + rect.h - y : total > 0 ? (rect.h * sum) / total : rect.h / rows.length;
    let x = rect.x;
    r.forEach((i, k) => {
      const w = k === r.length - 1 ? rect.x + rect.w - x : sum > 0 ? (rect.w * ws[i]!) / sum : rect.w / r.length;
      out[i] = { x, y, w, h };
      x += w;
    });
    y += h;
  });
  return out;
}

/**
 * Ordered strip treemap: tiles fill rows left to right, top to bottom, in
 * list order, with optimal row breaks. List neighbours stay screen
 * neighbours (themes arrive similarity-ordered) and the rows always fill
 * `rect` exactly. (An order-preserving binary split was tried first: at
 * 1194×800 it cut 65×228 slivers no title fits in.)
 */
export function stripTreemap(weights: number[], rect: Rect): Rect[] {
  const n = weights.length;
  if (n === 0) return [];
  const ws = weights.map((w) => Math.max(0, w));
  if (ws.reduce((a, b) => a + b, 0) <= 0 || rect.w <= 0 || rect.h <= 0) {
    return ws.map((_, i) => ({ x: rect.x + (rect.w * i) / n, y: rect.y, w: rect.w / n, h: rect.h }));
  }
  return layoutRows(optimalRows(ws, rect).rows, ws, rect);
}

// ---------------------------------------------------------------------------
// Remembered layout

/** What the map remembers between builds: rows of theme keys and each key's
 *  member item ids (theme ids change when membership does, so identity is
 *  matched by overlap, not by id). */
export interface RememberedLayout {
  rows: string[][];
  members: Record<string, number[]>;
}

export interface LayoutTheme {
  id: string;
  /** Every item id the theme stands for (story members included). */
  members: number[];
  weight: number;
}

/** Minimum member overlap (Jaccard) for a theme to inherit a remembered slot. */
const MATCH_MIN_JACCARD = 0.3;
/** A remembered layout is kept while its cost stays within this factor of
 *  the optimal one. Replayed on three real consecutive builds (2026-10-06):
 *  2x kept 95% of tiles in place at 1700x1184 through a heavy re-score,
 *  1.5x kept 58%; 3x also held at 1200x800 but only by squeezing a tile
 *  below the readable minimum (150x78), so familiarity stops at 2x. */
const KEEP_COST_FACTOR = 2;

function jaccard(a: number[], b: Set<number>): number {
  let inter = 0;
  for (const x of a) if (b.has(x)) inter++;
  const union = a.length + b.size - inter;
  return union === 0 ? 0 : inter / union;
}

/** Match current items (by member ids) to remembered keys, one-to-one, best
 *  overlap first. Shared by the Themes and Graph views' layout memory. */
export function matchByMembers(memberIds: number[][], prev: Record<string, number[]>): Map<number, string> {
  const pairs: { t: number; key: string; j: number }[] = [];
  for (const [key, ids] of Object.entries(prev)) {
    const set = new Set(ids);
    memberIds.forEach((members, t) => {
      const j = jaccard(members, set);
      if (j >= MATCH_MIN_JACCARD) pairs.push({ t, key, j });
    });
  }
  pairs.sort((a, b) => b.j - a.j || a.t - b.t || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
  const out = new Map<number, string>();
  const used = new Set<string>();
  for (const p of pairs) {
    if (out.has(p.t) || used.has(p.key)) continue;
    out.set(p.t, p.key);
    used.add(p.key);
  }
  return out;
}

/**
 * One distinct key per theme: its remembered match, else its own id. An
 * unmatched theme whose id is already some other theme's matched key gets a
 * suffixed key — sharing one key wrote that key twice into the remembered
 * rows, and the next build laid that theme out twice (live 2026-10-07:
 * 23 slots for 19 themes, the last row pushed onto the unthemed bar).
 */
function uniqueKeys(themes: LayoutTheme[], match: Map<number, string>): string[] {
  const used = new Set(match.values());
  return themes.map((t, i) => {
    const matched = match.get(i);
    if (matched !== undefined) return matched;
    let key = t.id;
    for (let n = 2; used.has(key); n++) key = `${t.id}~${n}`;
    used.add(key);
    return key;
  });
}

/**
 * Lay out themes so the map stays recognisable between visits. A plain
 * re-layout moved 81% of tiles by more than a tenth of the screen when one
 * theme disappeared (audit simulation on the live corpus, 2026-10-06);
 * reusing the remembered rows cut that to 5%. Themes that match a
 * remembered one (member overlap) keep their row and order; new themes join
 * the row of their nearest placed neighbour in `themes` order (similarity
 * order). The remembered rows are used while their cost stays within
 * KEEP_COST_FACTOR of the optimal layout; otherwise the remembered ORDER is
 * kept with fresh row breaks. With nothing remembered, the optimal layout.
 */
export function stableThemeLayout(
  themes: LayoutTheme[],
  rect: Rect,
  prev: RememberedLayout | null,
): { rects: Rect[]; remembered: RememberedLayout } {
  const ws = themes.map((t) => Math.max(0, t.weight));
  const optimal = optimalRows(ws, rect);
  const match = prev ? matchByMembers(themes.map((t) => t.members), prev.members) : new Map<number, string>();
  const keys = uniqueKeys(themes, match);
  const keyOf = (i: number) => keys[i]!;

  let rows = optimal.rows;
  if (prev && match.size > 0) {
    const indexOfKey = new Map<string, number>();
    themes.forEach((_, i) => indexOfKey.set(keyOf(i), i));
    // Each theme lands in exactly one slot, even if a remembered layout
    // (written before keys were unique) lists a key twice.
    const placed = new Set<number>();
    const candidate: number[][] = prev.rows.map((r) =>
      r.flatMap((k) => {
        const i = indexOfKey.get(k);
        if (i === undefined || placed.has(i)) return [];
        placed.add(i);
        return [i];
      }),
    );
    themes.forEach((_, i) => {
      if (placed.has(i)) return;
      // Join the row of the nearest already-placed theme before it in
      // similarity order, right after it; with none, start the first row.
      let anchor = -1;
      for (let j = i - 1; j >= 0; j--) {
        if (placed.has(j)) {
          anchor = j;
          break;
        }
      }
      if (anchor < 0) {
        if (candidate.length === 0) candidate.push([]);
        candidate[0]!.unshift(i);
      } else {
        const row = candidate.find((r) => r.includes(anchor))!;
        row.splice(row.indexOf(anchor) + 1, 0, i);
      }
      placed.add(i);
    });
    const kept = candidate.filter((r) => r.length > 0);
    if (rowsCost(kept, ws, rect) <= optimal.cost * KEEP_COST_FACTOR + 1e-9) {
      rows = kept;
    } else {
      // The remembered rows no longer fit (themes came, went or changed
      // size): keep the remembered ORDER and only re-break the rows. On
      // real consecutive builds a fresh layout here moved 89-95% of tiles
      // far; keeping the order halves that.
      const order = kept.flat();
      const reordered = optimalRows(order.map((i) => ws[i]!), rect);
      rows = reordered.rows.map((r) => r.map((k) => order[k]!));
    }
  }

  const rects = ws.reduce((a, b) => a + b, 0) > 0 && rect.w > 0 && rect.h > 0 ? layoutRows(rows, ws, rect) : stripTreemap(ws, rect);
  const remembered: RememberedLayout = {
    rows: rows.map((r) => r.map(keyOf)),
    members: Object.fromEntries(themes.map((t, i) => [keyOf(i), t.members])),
  };
  return { rects, remembered };
}
