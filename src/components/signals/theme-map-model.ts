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

/**
 * Order-preserving binary treemap: split the ordered list where the weight
 * halves balance, cut the rectangle across its longer side in proportion,
 * recurse. Neighbours in the list stay neighbours on screen (themes arrive
 * similarity-ordered), and cutting the long side keeps tiles near-square at
 * any viewport aspect — the map always fills the space it is given.
 */
export function binaryTreemap(weights: number[], rect: Rect): Rect[] {
  const n = weights.length;
  const out: Rect[] = new Array(n);
  if (n === 0) return out;
  const prefix = [0];
  for (const w of weights) prefix.push(prefix[prefix.length - 1]! + Math.max(0, w));

  const split = (i0: number, i1: number, r: Rect) => {
    if (i1 - i0 === 1) {
      out[i0] = r;
      return;
    }
    const base = prefix[i0]!;
    const total = prefix[i1]! - base;
    let k = i0 + 1;
    if (total > 0) {
      let best = Infinity;
      for (let j = i0 + 1; j < i1; j++) {
        const d = Math.abs(prefix[j]! - base - total / 2);
        if (d < best) {
          best = d;
          k = j;
        }
      }
    } else {
      k = i0 + Math.floor((i1 - i0) / 2);
    }
    const f = total > 0 ? (prefix[k]! - base) / total : (k - i0) / (i1 - i0);
    if (r.w >= r.h) {
      const w0 = r.w * f;
      split(i0, k, { x: r.x, y: r.y, w: w0, h: r.h });
      split(k, i1, { x: r.x + w0, y: r.y, w: r.w - w0, h: r.h });
    } else {
      const h0 = r.h * f;
      split(i0, k, { x: r.x, y: r.y, w: r.w, h: h0 });
      split(k, i1, { x: r.x, y: r.y + h0, w: r.w, h: r.h - h0 });
    }
  };
  split(0, n, rect);
  return out;
}

/** Tile area weight: member count plus one, so a 2-item theme still gets
 *  room for its header and both titles (the count itself is printed). */
export function themeWeight(theme: Theme): number {
  return theme.items.length + 1;
}

/** Tile typography, px — shared by the tile and its capacity estimate. */
export const TILE_PAD = 10;
export const TILE_HEADER_H = 22;
export const TILE_LINE_H = 20;

/** How many item lines fit in a tile of height `h`; when not every item
 *  fits, the last line is spent on "+N more". */
export function visibleLines(h: number, itemCount: number): { shown: number; more: number } {
  const lines = Math.max(0, Math.floor((h - 2 * TILE_PAD - TILE_HEADER_H) / TILE_LINE_H));
  if (itemCount <= lines) return { shown: itemCount, more: 0 };
  const shown = Math.max(0, lines - 1);
  return { shown, more: itemCount - shown };
}
