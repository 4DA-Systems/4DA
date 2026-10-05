// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// The theme map's data model and layout — pure functions, no React, no DOM.
//
// The Signal map used to be a 150-node force canvas. Measured live
// 2026-10-05: zero edges ran BETWEEN clusters (so where a cluster sat meant
// nothing), 87% of nodes shared one category colour, and fitting the whole
// map took zoom 0.275 — 136 of 150 titles hidden, 37% of the width used. The
// information is a partition (your stack / themes / unthemed), so it renders
// as one: a stack column, an order-preserving treemap of themes that fills
// the viewport with readable titles, and an unthemed list.

import type { ContentGraph, GraphNode } from '../../types/graph';

export interface Theme {
  id: string;
  label: string;
  /** Members, most important first (see `byImportance`). */
  items: GraphNode[];
}

export interface ThemeMap {
  /** Items touching the user's stack — the most actionable set, never buried in a theme. */
  stack: GraphNode[];
  /** Themes in backend order (related themes adjacent — content_graph/themes.rs). */
  themes: Theme[];
  /** Items related to no theme in this window. */
  unthemed: GraphNode[];
}

const PRIORITY_RANK: Record<string, number> = { critical: 0, alert: 1 };

/** Security and urgent items first, then relevance, then id (deterministic). */
export function byImportance(a: GraphNode, b: GraphNode): number {
  const sa = a.category === 'security' ? 0 : 1;
  const sb = b.category === 'security' ? 0 : 1;
  if (sa !== sb) return sa - sb;
  const pa = PRIORITY_RANK[a.signal_priority ?? ''] ?? 2;
  const pb = PRIORITY_RANK[b.signal_priority ?? ''] ?? 2;
  if (pa !== pb) return pa - pb;
  if (b.relevance_score !== a.relevance_score) return b.relevance_score - a.relevance_score;
  return a.id - b.id;
}

/** Split the graph into the three disjoint groups the map shows. Every node
 *  lands in exactly one group: stack membership wins over a theme (the
 *  backend already partitions this way; the guard keeps an older payload
 *  from showing an item twice). */
export function buildThemeMap(graph: ContentGraph): ThemeMap {
  const byId = new Map(graph.nodes.map((n) => [n.id, n]));
  const stack = graph.nodes.filter((n) => n.affects_you).sort(byImportance);
  const placed = new Set(stack.map((n) => n.id));

  const themes: Theme[] = [];
  for (const c of graph.clusters) {
    const items = c.node_ids
      .map((id) => byId.get(id))
      .filter((n): n is GraphNode => n !== undefined && !placed.has(n.id))
      .sort(byImportance);
    if (items.length === 0) continue;
    for (const n of items) placed.add(n.id);
    themes.push({ id: c.id, label: c.label, items });
  }

  const unthemed = graph.nodes.filter((n) => !placed.has(n.id)).sort(byImportance);
  return { stack, themes, unthemed };
}

// The mark and the column already say where an item came from, so a leading
// "crates.io: " / "npm: " just eats width. Strip a KNOWN source prefix only —
// never a generic "word:", so "Rust 1.80: released" keeps its colon.
const SOURCE_PREFIX =
  /^(crates\.io|npm|pypi|pep|github|gh|hn|reddit|arxiv|dev\.to|lobsters|lobste\.rs|stack ?overflow|so|product ?hunt|hugging ?face|hf|go modules?|youtube|yt|bluesky|mastodon|cve|osv|rss)\s*[:\-–]\s+/i;

export function cleanTitle(raw: string): string {
  return raw.replace(SOURCE_PREFIX, '').trim() || raw;
}

/** "rust · cache · next.js" →"Rust · cache · next.js" — sentence case reads
 *  faster than the old letter-spaced capitals and keeps package spellings. */
export function displayThemeLabel(label: string): string {
  if (!label) return label;
  const first = label.charAt(0);
  return /[a-z]/.test(first) ? first.toUpperCase() + label.slice(1) : label;
}

// ---------------------------------------------------------------------------
// Treemap layout

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
 *  (gap included) plus 50% slack, since rows of fixed order cannot give
 *  every tile its ideal shape at once. */
export const MIN_TILE_AREA = 1.5 * (TILE_MIN_WIDTH + TILE_GAP) * (TILE_MIN_HEIGHT + TILE_GAP + 1);

/**
 * Ordered strip treemap: tiles fill rows left to right, top to bottom, in
 * list order, with the row breaks chosen so every tile sits as close to the
 * target aspect as the space allows. List neighbours stay
 * screen neighbours (themes arrive similarity-ordered) and the map reads like
 * text. The rows always fill `rect` exactly — no zoom, no wasted width.
 * (An order-preserving binary split was tried first: at 1194×800 it cut
 * 65×228 slivers no title fits in.)
 */
export function stripTreemap(
  weights: number[],
  rect: Rect,
  targetAspect = TILE_TARGET_ASPECT,
  minWidth = TILE_MIN_WIDTH + TILE_GAP,
  minHeight = TILE_MIN_HEIGHT + TILE_GAP + 1,
): Rect[] {
  const n = weights.length;
  const out: Rect[] = new Array(n);
  if (n === 0) return out;
  const ws = weights.map((w) => Math.max(0, w));
  const total = ws.reduce((a, b) => a + b, 0);
  if (total <= 0 || rect.w <= 0 || rect.h <= 0) {
    // Degenerate: equal slices so every tile still gets a finite box.
    ws.forEach((_, i) => {
      out[i] = { x: rect.x + (rect.w * i) / n, y: rect.y, w: rect.w / n, h: rect.h };
    });
    return out;
  }
  const scale = (rect.w * rect.h) / total;
  const prefix = [0];
  for (const w of ws) prefix.push(prefix[prefix.length - 1]! + w);
  // Cost of tiles i..j-1 as one row: each tile's squared distance from the
  // target aspect (1 = perfect), so one badly stretched tile costs more
  // than several slightly-off ones.
  const rowCost = (i: number, j: number): number => {
    const sum = prefix[j]! - prefix[i]!;
    if (sum <= 0) return Infinity;
    const h = (sum * scale) / rect.w;
    // A row too short for two titles costs every one of its tiles.
    let acc = h < minHeight ? 100 * (j - i) * (minHeight / Math.max(h, 1)) ** 2 : 0;
    for (let t = i; t < j; t++) {
      const w = (ws[t]! * scale) / h;
      const aspect = w / h;
      const off = aspect > targetAspect ? aspect / targetAspect : targetAspect / Math.max(aspect, 1e-9);
      acc += off * off;
      // A tile too narrow for a title costs far more than an off aspect.
      if (w < minWidth) acc += 100 * (minWidth / Math.max(w, 1)) ** 2;
    }
    return acc;
  };
  // Optimal row breaks, order preserved (dynamic programming, as in
  // justified text): a greedy pass left the last row as one stretched sliver.
  const best: number[] = [0];
  const from: number[] = [0];
  for (let j = 1; j <= n; j++) {
    best[j] = Infinity;
    for (let i = 0; i < j; i++) {
      const c = best[i]! + rowCost(i, j);
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

  let y = rect.y;
  rows.forEach((r, ri) => {
    const sum = r.reduce((a, i) => a + ws[i]!, 0);
    // The last row takes the remainder so rounding never leaves a gap.
    const h = ri === rows.length - 1 ? rect.y + rect.h - y : (sum * scale) / rect.w;
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
 * Tile weights: member count plus one shared offset `c`, chosen so the
 * SMALLEST theme still gets `minArea` of the `area` available. Pure count
 * weights gave a 2-item theme a 65px-wide sliver at 1194×800 (live
 * 2026-10-05: "Re…", "Sin…"). A common offset keeps the order of sizes —
 * bigger themes still get bigger tiles — and the exact count is printed on
 * every tile. When the area cannot fit every theme at the minimum, all
 * tiles get equal weight (the best that space allows).
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


/** Narrowest column of titles inside a tile, and the space between columns. */
export const TILE_COLUMN_MIN_W = 300;
export const TILE_COLUMN_GAP = 16;
const MAX_TILE_COLUMNS = 3;

/** Title columns a tile of width `w` holds. A row's large theme can come out
 *  1080px wide and two lines tall (live, 1700×1184): one column there showed
 *  2 of 13 titles beside 800px of empty space. */
export function tileColumns(w: number): number {
  const inner = w - 2 * TILE_PAD + TILE_COLUMN_GAP;
  return Math.min(MAX_TILE_COLUMNS, Math.max(1, Math.floor(inner / (TILE_COLUMN_MIN_W + TILE_COLUMN_GAP))));
}

/** How many titles fit in a tile of height `h` with `cols` columns. When not
 *  every item fits and there are 3+ slots, the last slot is spent on "+N
 *  more"; with fewer slots every slot is a title instead (a one-line tile
 *  reading only "+2 more" showed nothing, live 2026-10-05) — the header
 *  still prints the count and opens the whole theme. */
export function visibleLines(h: number, itemCount: number, cols = 1): { shown: number; more: number } {
  const lines = Math.max(0, Math.floor((h - 2 * TILE_PAD - TILE_HEADER_H) / TILE_LINE_H));
  const slots = lines * Math.max(1, cols);
  if (itemCount <= slots) return { shown: itemCount, more: 0 };
  if (slots < 3) return { shown: slots, more: 0 };
  const shown = slots - 1;
  return { shown, more: itemCount - shown };
}
