// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, expect, it } from 'vitest';

import type { ContentGraph, GraphNode } from '../../types/graph';
import { HULL_PADDING, MEMBER_SPACING, buildGroups, discRadius, layoutGraph } from './graph-layout';

function node(id: number, over: Partial<GraphNode> = {}): GraphNode {
  return {
    id,
    title: `item ${id}`,
    url: null,
    source_type: 'hackernews',
    relevance_score: 0.5,
    signal_type: null,
    signal_priority: null,
    created_at: '2026-10-06T00:00:00Z',
    primary_topic: null,
    cluster_id: null,
    member_count: 1,
    member_ids: [id],
    category: 'discussion',
    affects_you: false,
    ...over,
  };
}

/** Themes of the given sizes (ids from 1), `stackInTheme` stack items folded
 *  into the first theme, `looseStack` unclaimed stack items, `shelf` unthemed. */
function graphOf(sizes: number[], opts: { stackInTheme?: number; looseStack?: number; shelf?: number } = {}): ContentGraph {
  const nodes: GraphNode[] = [];
  const clusters: ContentGraph['clusters'] = [];
  let id = 1;
  sizes.forEach((size, c) => {
    const ids: number[] = [];
    for (let k = 0; k < size; k++) {
      nodes.push(node(id));
      ids.push(id++);
    }
    const stackIds: number[] = [];
    if (c === 0) {
      for (let k = 0; k < (opts.stackInTheme ?? 0); k++) {
        nodes.push(node(id, { affects_you: true }));
        stackIds.push(id++);
      }
    }
    clusters.push({ id: `t${c}`, label: `theme ${c}`, node_ids: ids, source_count: 1, coherence: 0.5, stack_node_ids: stackIds });
  });
  for (let k = 0; k < (opts.looseStack ?? 0); k++) nodes.push(node(id++, { affects_you: true }));
  for (let k = 0; k < (opts.shelf ?? 0); k++) nodes.push(node(id++, { relevance_score: k / 100 }));
  return {
    nodes,
    edges: [],
    clusters,
    meta: {
      total_items: nodes.length,
      total_edges: 0,
      cluster_count: clusters.length,
      story_count: 0,
      collapsed_items: 0,
      hidden_items: 0,
      window_candidates: nodes.length,
      time_window_days: 7,
      edge_threshold: 'mutual-knn',
      mean_cluster_coherence: 0.5,
      curated_items: 0,
      windows_differ: false,
    },
  };
}

const LIVE_SIZES = [14, 11, 9, 8, 7, 6, 6, 5, 5, 4, 4, 3, 3, 3];

describe('buildGroups', () => {
  it('places every node exactly once', () => {
    const g = graphOf(LIVE_SIZES, { stackInTheme: 3, looseStack: 4, shelf: 20 });
    const { groups, shelfIds } = buildGroups(g, 'Your stack');
    const all = [...groups.flatMap((x) => x.nodeIds), ...shelfIds];
    expect(all).toHaveLength(g.nodes.length);
    expect(new Set(all).size).toBe(g.nodes.length);
  });

  it('keeps stack items inside their theme and puts unclaimed ones in a leading stack group', () => {
    const g = graphOf([5, 4], { stackInTheme: 2, looseStack: 3 });
    const { groups } = buildGroups(g, 'Your stack');
    expect(groups[0]).toMatchObject({ key: 'stack', kind: 'stack', label: 'Your stack' });
    expect(groups[0]!.nodeIds).toHaveLength(3);
    const first = groups.find((x) => x.key === 't0')!;
    expect(first.nodeIds).toHaveLength(7);
  });

  it('has no stack group when every stack item is claimed', () => {
    const { groups } = buildGroups(graphOf([5], { stackInTheme: 2 }), 'Your stack');
    expect(groups.some((x) => x.kind === 'stack')).toBe(false);
  });

  it('sorts the shelf by relevance, highest first', () => {
    const g = graphOf([3], { shelf: 5 });
    const { shelfIds } = buildGroups(g, 's');
    const scores = shelfIds.map((id) => g.nodes.find((n) => n.id === id)!.relevance_score);
    expect(scores).toEqual([...scores].sort((a, b) => b - a));
  });
});

describe('layoutGraph', () => {
  it('positions every node, with no two marks closer than a label slot', () => {
    const g = graphOf(LIVE_SIZES, { stackInTheme: 3, looseStack: 4, shelf: 30 });
    const l = layoutGraph(g, 'Your stack', 1.6);
    expect(l.positions.size).toBe(g.nodes.length);
    const pts = [...l.positions.values()];
    let min = Infinity;
    for (let i = 0; i < pts.length; i++)
      for (let j = i + 1; j < pts.length; j++) min = Math.min(min, Math.hypot(pts[i]!.x - pts[j]!.x, pts[i]!.y - pts[j]!.y));
    expect(min).toBeGreaterThan(MEMBER_SPACING * 0.5);
  });

  it('never overlaps two hulls', () => {
    const l = layoutGraph(graphOf(LIVE_SIZES, { looseStack: 4, shelf: 30 }), 's', 1.6);
    for (let i = 0; i < l.groups.length; i++)
      for (let j = i + 1; j < l.groups.length; j++) {
        const a = l.groups[i]!;
        const b = l.groups[j]!;
        expect(Math.hypot(a.cx - b.cx, a.cy - b.cy)).toBeGreaterThanOrEqual(a.radius + b.radius - 1e-6);
      }
  });

  it('keeps every member inside its hull and below the name band', () => {
    const l = layoutGraph(graphOf(LIVE_SIZES), 's', 1.6);
    for (const g of l.groups) {
      // The name's line box at the lowest fit zoom seen live (~0.25): 18 px.
      const nameBottom = g.headerY + 18 / 0.25;
      for (const id of g.nodeIds) {
        const p = l.positions.get(id)!;
        expect(Math.hypot(p.x - g.cx, p.y - g.cy)).toBeLessThanOrEqual(g.radius - HULL_PADDING + 1e-6);
        expect(p.y).toBeGreaterThan(nameBottom);
      }
      expect(g.headerY).toBeGreaterThan(g.cy - g.radius);
      expect(discRadius(g.nodeIds.length)).toBeLessThan(g.radius);
    }
  });

  it('fills a wide canvas wide and a tall canvas tall', () => {
    const g = graphOf(LIVE_SIZES, { shelf: 30 });
    const shape = (aspect: number) => {
      const l = layoutGraph(g, 's', aspect);
      const xs = [...l.positions.values()].map((p) => p.x);
      const ys = [...l.positions.values()].map((p) => p.y);
      return (Math.max(...xs) - Math.min(...xs)) / (Math.max(...ys) - Math.min(...ys));
    };
    expect(shape(1.6)).toBeGreaterThan(1.1);
    expect(shape(0.6)).toBeLessThan(1);
  });

  it('is deterministic', () => {
    const g = graphOf(LIVE_SIZES, { looseStack: 2, shelf: 10 });
    const a = layoutGraph(g, 's', 1.4);
    const b = layoutGraph(g, 's', 1.4);
    expect([...a.positions]).toEqual([...b.positions]);
    expect(a.order).toEqual(b.order);
  });

  it('keeps the remembered group order when the backend reorders themes', () => {
    const g = graphOf([6, 5, 4, 3]);
    const first = layoutGraph(g, 's', 1.6);
    const reordered: ContentGraph = { ...g, clusters: [...g.clusters].reverse().map((c, i) => ({ ...c, id: `new${i}` })) };
    const second = layoutGraph(reordered, 's', 1.6, { order: first.order, members: first.members });
    const firstLabels = first.order.map((k) => g.clusters.find((c) => c.id === k)!.label);
    const secondLabels = second.groups.map((x) => x.label);
    // Placed order follows the remembered keys, so the labels read the same.
    expect(second.order).toEqual(first.order);
    expect([...secondLabels].sort()).toEqual([...firstLabels].sort());
  });

  it('handles a map with only unthemed items', () => {
    const l = layoutGraph(graphOf([], { shelf: 7 }), 's', 1.6);
    expect(l.groups).toHaveLength(0);
    expect(l.shelf?.nodeIds).toHaveLength(7);
    expect(l.positions.size).toBe(7);
  });
});
