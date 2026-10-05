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
import { TILE_HEADER_H, TILE_LINE_H, TILE_PAD } from './theme-map-layout';

export interface Theme {
  id: string;
  label: string;
  /** Members, most important first (see `byImportance`). */
  items: GraphNode[];
  /** Stack items from this theme's community — shown in the stack column,
   *  named on the tile so the link between news and packages survives. */
  stack: GraphNode[];
}

export interface ThemeMap {
  /** Items touching the user's stack — the most actionable set, never buried in a theme. */
  stack: GraphNode[];
  /** Themes in backend order (related themes adjacent — content_graph/themes.rs). */
  themes: Theme[];
  /** Items related to no theme in this window. */
  unthemed: GraphNode[];
  /** Stack item id → label of the theme its community belongs to. */
  stackTheme: Map<number, string>;
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
    const stackHere = (c.stack_node_ids ?? [])
      .map((id) => byId.get(id))
      .filter((n): n is GraphNode => n !== undefined && n.affects_you)
      .sort(byImportance);
    themes.push({ id: c.id, label: c.label, items, stack: stackHere });
  }

  const stackTheme = new Map<number, string>();
  for (const t of themes) for (const n of t.stack) if (!stackTheme.has(n.id)) stackTheme.set(n.id, t.label);
  const unthemed = graph.nodes.filter((n) => !placed.has(n.id)).sort(byImportance);
  return { stack, themes, unthemed, stackTheme };
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
export function visibleLines(
  h: number,
  itemCount: number,
  cols = 1,
  reservedLines = 0,
): { shown: number; more: number } {
  const lines = Math.max(0, Math.floor((h - 2 * TILE_PAD - TILE_HEADER_H) / TILE_LINE_H) - reservedLines);
  const slots = lines * Math.max(1, cols);
  if (itemCount <= slots) return { shown: itemCount, more: 0 };
  if (slots < 3) return { shown: slots, more: 0 };
  const shown = slots - 1;
  return { shown, more: itemCount - shown };
}
