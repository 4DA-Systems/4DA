// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Graph view layout — pure geometry, no React, no DOM. Computed client-side
// from the same payload the Themes view uses, so the graph adds no backend
// work (the old server-side layout + anchors + chain edges cost ~250 ms per
// build, measured 2026-10-06).
//
// The audit of the old graph (2026-10-06) found: a portrait map in a wide
// canvas (37-40% of the width used, fit zoom 0.30), clusters placed WORSE
// than random (z -2.1: a size-ordered spiral put unrelated themes side by
// side), and the user's stack scattered or parked in "not connected". So:
// - groups are themes WITH their community's stack items inside them, plus one
//   "Your stack" group for stack items no theme claimed, placed first;
// - groups pack into rows sized to the canvas aspect, in the backend's
//   similarity order (adjacent themes are related, z +5.7 on the live corpus);
// - the unthemed items form a shelf block in the same flow, not a strip that
//   makes the map taller;
// - the group ORDER is remembered between builds (matched by shared members).

import type { ContentGraph } from '../../types/graph';
import { matchByMembers } from './theme-map-layout';

/** Target spacing between neighbouring members inside a group disc (flow
 *  units), sized for the label under each mark — the old layout's value. */
export const MEMBER_SPACING = 95;
/** Free space between a group's outermost member and its hull ring. */
export const HULL_PADDING = 60;
/** Extra band at the top of each hull for the group's name, so the name never
 *  prints across the members (live 2026-10-07: centred names covered hub
 *  marks and their titles). The member disc sits that much lower. */
export const HEADER_BAND = 40;
/** The name's top edge sits this far below the hull's rim (flow units; the
 *  label layout's HEADER_RIM_INSET). */
export const HEADER_INSET = 20;
/** Space between two hulls in a row, and between rows. */
export const GROUP_GAP = 50;
/** Golden angle: successive spiral points never align. */
const GOLDEN_ANGLE = 2.399963;
/** Height reserved above the shelf grid for its header. */
const SHELF_HEADER = 60;

export interface GraphGroup {
  key: string;
  kind: 'theme' | 'stack';
  label: string;
  nodeIds: number[];
}

export interface PlacedGroup extends GraphGroup {
  cx: number;
  cy: number;
  /** Hull radius (members + padding + name band). */
  radius: number;
  /** Top edge of the group's name (flow units). */
  headerY: number;
}

export interface GraphLayout {
  /** Mark CENTRE per node id (flow units). */
  positions: Map<number, { x: number; y: number }>;
  groups: PlacedGroup[];
  shelf: { x: number; y: number; nodeIds: number[] } | null;
  width: number;
  height: number;
  /** Group keys in display order — what the next build should remember. */
  order: string[];
  /** Member item ids per group key (identity for the next build's match). */
  members: Record<string, number[]>;
}

/** Remembered between builds: group order, and each group's member item ids. */
export interface RememberedOrder {
  order: string[];
  members: Record<string, number[]>;
}

/** Partition the payload into display groups and the unthemed shelf. Every
 *  node lands exactly once. Stack items stay inside the theme their community
 *  belongs to (`stack_node_ids`) — the context the Themes column cannot show;
 *  stack items no theme claimed form the "Your stack" group. */
export function buildGroups(graph: ContentGraph, stackLabel: string): { groups: GraphGroup[]; shelfIds: number[] } {
  const byId = new Map(graph.nodes.map((n) => [n.id, n]));
  const placed = new Set<number>();
  const themes: GraphGroup[] = [];
  for (const c of graph.clusters) {
    const ids = [...c.node_ids, ...(c.stack_node_ids ?? [])].filter((id) => byId.has(id) && !placed.has(id));
    if (ids.length === 0) continue;
    ids.forEach((id) => placed.add(id));
    themes.push({ key: c.id, kind: 'theme', label: c.label, nodeIds: ids });
  }
  const loose = graph.nodes.filter((n) => n.affects_you && !placed.has(n.id)).map((n) => n.id);
  loose.forEach((id) => placed.add(id));
  const groups: GraphGroup[] = loose.length > 0 ? [{ key: 'stack', kind: 'stack', label: stackLabel, nodeIds: loose }, ...themes] : themes;
  const shelfIds = graph.nodes
    .filter((n) => !placed.has(n.id))
    .sort((a, b) => b.relevance_score - a.relevance_score || a.id - b.id)
    .map((n) => n.id);
  return { groups, shelfIds };
}

/** Disc radius that gives `n` sunflower points ~MEMBER_SPACING apart. */
export function discRadius(n: number): number {
  return MEMBER_SPACING * 0.62 * Math.sqrt(Math.max(1, n)) + 30;
}

const hullRadius = (n: number) => discRadius(n) + HULL_PADDING + HEADER_BAND / 2;

/** Reorder groups so the ones matching a remembered group keep their
 *  remembered relative order; new groups join right after their nearest
 *  placed predecessor in the incoming (similarity) order. */
export function rememberedOrder(groups: GraphGroup[], memberIds: number[][], prev: RememberedOrder | null): { ordered: number[]; keys: string[] } {
  const identity = groups.map((_, i) => i);
  if (!prev || prev.order.length === 0) return { ordered: identity, keys: groups.map((g) => g.key) };
  const match = matchByMembers(memberIds, prev.members);
  const keyOf = (i: number) => (groups[i]!.kind === 'stack' ? 'stack' : match.get(i) ?? groups[i]!.key);
  const rank = new Map(prev.order.map((k, i) => [k, i]));
  const kept = identity.filter((i) => rank.has(keyOf(i))).sort((a, b) => rank.get(keyOf(a))! - rank.get(keyOf(b))!);
  const out = [...kept];
  for (const i of identity) {
    if (rank.has(keyOf(i))) continue;
    let anchor = -1;
    for (let j = i - 1; j >= 0; j--) {
      if (out.includes(j)) {
        anchor = j;
        break;
      }
    }
    out.splice(anchor < 0 ? 0 : out.indexOf(anchor) + 1, 0, i);
  }
  return { ordered: out, keys: out.map(keyOf) };
}

interface Block {
  w: number;
  h: number;
  /** Index into the group list, or -1 for the shelf. */
  ref: number;
}

/** Greedy rows of blocks within `width`; returns each block's centre and the
 *  total height. Rows are centred horizontally, blocks vertically in a row. */
function packRows(blocks: Block[], width: number): { centres: { x: number; y: number }[]; height: number; usedWidth: number } {
  const rows: Block[][] = [];
  let row: Block[] = [];
  let rowW = 0;
  for (const b of blocks) {
    const add = (row.length > 0 ? GROUP_GAP : 0) + b.w;
    if (row.length > 0 && rowW + add > width) {
      rows.push(row);
      row = [];
      rowW = 0;
    }
    rowW += (row.length > 0 ? GROUP_GAP : 0) + b.w;
    row.push(b);
  }
  if (row.length > 0) rows.push(row);
  const centres = new Array<{ x: number; y: number }>(blocks.length);
  let y = 0;
  let usedWidth = 0;
  for (const r of rows) {
    const rh = Math.max(...r.map((b) => b.h));
    const rw = r.reduce((a, b) => a + b.w, 0) + GROUP_GAP * (r.length - 1);
    usedWidth = Math.max(usedWidth, rw);
    let x = (width - rw) / 2;
    for (const b of r) {
      centres[blocks.indexOf(b)] = { x: x + b.w / 2, y: y + rh / 2 };
      x += b.w + GROUP_GAP;
    }
    y += rh + GROUP_GAP;
  }
  return { centres, height: Math.max(0, y - GROUP_GAP), usedWidth };
}

/**
 * Lay out the graph for a canvas of aspect `aspect` (width / height).
 * Searches the row width whose packed result matches the canvas shape, so a
 * fit-to-view fills the canvas instead of a portrait column in its middle.
 */
export function layoutGraph(
  graph: ContentGraph,
  stackLabel: string,
  aspect: number,
  prev: RememberedOrder | null = null,
): GraphLayout {
  const { groups, shelfIds } = buildGroups(graph, stackLabel);
  const byId = new Map(graph.nodes.map((n) => [n.id, n] as const));
  const memberIds = groups.map((g) => g.nodeIds.flatMap((id) => byId.get(id)?.member_ids ?? [id]));
  const { ordered, keys } = rememberedOrder(groups, memberIds, prev);

  const blocks: Block[] = ordered.map((gi) => {
    const d = 2 * hullRadius(groups[gi]!.nodeIds.length);
    return { w: d, h: d, ref: gi };
  });
  let shelfCols = 0;
  if (shelfIds.length > 0) {
    shelfCols = Math.max(1, Math.ceil(Math.sqrt(shelfIds.length * 1.6)));
    const shelfRows = Math.ceil(shelfIds.length / shelfCols);
    blocks.push({ w: shelfCols * MEMBER_SPACING, h: shelfRows * MEMBER_SPACING + SHELF_HEADER, ref: -1 });
  }

  // Width search: the packed shape's aspect falls as the row width shrinks.
  const widest = blocks.reduce((a, b) => a + b.w + GROUP_GAP, 0);
  const narrowest = Math.max(...blocks.map((b) => b.w), 1);
  let lo = narrowest;
  let hi = Math.max(widest, narrowest);
  let best = packRows(blocks, hi);
  let bestW = hi;
  for (let i = 0; i < 40; i++) {
    const mid = (lo + hi) / 2;
    const p = packRows(blocks, mid);
    if (mid / Math.max(p.height, 1) > aspect) hi = mid;
    else lo = mid;
    // Keep whichever candidate fits the canvas at the highest zoom.
    const fit = (w: number, h: number) => Math.min(aspect / w, 1 / h);
    if (fit(Math.max(p.usedWidth, 1), Math.max(p.height, 1)) > fit(Math.max(best.usedWidth, 1), Math.max(best.height, 1))) {
      best = p;
      bestW = mid;
    }
  }
  const { centres } = best;

  const degree = new Map<number, number>();
  for (const e of graph.edges) {
    degree.set(e.source, (degree.get(e.source) ?? 0) + 1);
    degree.set(e.target, (degree.get(e.target) ?? 0) + 1);
  }

  const positions = new Map<number, { x: number; y: number }>();
  const placed: PlacedGroup[] = [];
  blocks.forEach((b, bi) => {
    const c = centres[bi]!;
    if (b.ref < 0) return;
    const g = groups[b.ref]!;
    const n = g.nodeIds.length;
    const r = discRadius(n);
    const R = hullRadius(n);
    // Member disc centre: lowered by half the name band, so the band at the
    // top is HULL_PADDING + HEADER_BAND and the bottom stays HULL_PADDING.
    const my = c.y + HEADER_BAND / 2;
    // Hubs (most related members) central; ties by id — deterministic.
    const members = [...g.nodeIds].sort((a, z) => (degree.get(z) ?? 0) - (degree.get(a) ?? 0) || a - z);
    members.forEach((id, slot) => {
      const rr = n === 1 ? 0 : r * Math.sqrt((slot + 0.5) / n);
      const theta = slot * GOLDEN_ANGLE + bi * 0.7;
      positions.set(id, { x: c.x + rr * Math.cos(theta), y: my + rr * Math.sin(theta) });
    });
    placed.push({ ...g, cx: c.x, cy: c.y, radius: R, headerY: c.y - R + HEADER_INSET });
  });

  let shelf: GraphLayout['shelf'] = null;
  const shelfBlock = blocks.findIndex((b) => b.ref < 0);
  if (shelfBlock >= 0) {
    const b = blocks[shelfBlock]!;
    const c = centres[shelfBlock]!;
    const left = c.x - b.w / 2 + MEMBER_SPACING / 2;
    const top = c.y - b.h / 2 + SHELF_HEADER + MEMBER_SPACING / 2;
    shelfIds.forEach((id, i) => {
      positions.set(id, { x: left + (i % shelfCols) * MEMBER_SPACING, y: top + Math.floor(i / shelfCols) * MEMBER_SPACING });
    });
    shelf = { x: c.x - b.w / 2, y: c.y - b.h / 2 + SHELF_HEADER, nodeIds: shelfIds };
  }

  const members: Record<string, number[]> = {};
  ordered.forEach((gi, k) => {
    members[keys[k]!] = memberIds[gi]!;
  });
  return { positions, groups: placed, shelf, width: bestW, height: best.height, order: keys, members };
}
