// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.resolve({})) }));

import type { ContentGraph, GraphCluster, GraphNode } from '../../types/graph';
import type { SourceRelevance } from '../../types';
import { graphNodeSetKey, surfacedSignature } from './use-content-graph';
import {
  TILE_HEADER_H,
  TILE_LINE_H,
  TILE_PAD,
  binaryTreemap,
  buildThemeMap,
  byImportance,
  cleanTitle,
  displayThemeLabel,
  visibleLines,
} from './theme-map-model';

function node(id: number, over: Partial<GraphNode> = {}): GraphNode {
  return {
    id,
    title: `n${id}`,
    url: null,
    source_type: 'hackernews',
    relevance_score: 0.5,
    signal_type: null,
    signal_priority: null,
    created_at: '2026-10-01T00:00:00Z',
    primary_topic: null,
    cluster_id: null,
    member_count: 1,
    member_ids: [id],
    category: 'discussion',
    affects_you: false,
    ...over,
  };
}

function cluster(id: string, node_ids: number[], label = id): GraphCluster {
  return { id, label, node_ids, source_count: 1, coherence: 0.7 };
}

function graph(nodes: GraphNode[], clusters: GraphCluster[] = []): ContentGraph {
  return {
    nodes,
    edges: [],
    clusters,
    meta: {} as ContentGraph['meta'],
  };
}

describe('graph staleness signals', () => {
  it('node-set key ignores order — only WHAT the map shows', () => {
    const a = graph([node(1), node(2, { member_ids: [2, 7] })]);
    const b = graph([node(2, { member_ids: [7, 2] }), node(1)]);
    expect(graphNodeSetKey(a)).toBe(graphNodeSetKey(b));
  });

  it('node-set key changes when a node or a story member changes', () => {
    const base = graphNodeSetKey(graph([node(1), node(2)]));
    expect(graphNodeSetKey(graph([node(1), node(3)]))).not.toBe(base);
    expect(graphNodeSetKey(graph([node(1), node(2, { member_ids: [2, 9] })]))).not.toBe(base);
  });

  it('surfaced signature is stable across a re-merge that changes nothing surfaced', () => {
    // The old pill compared the results ARRAY by identity: every background
    // merge (new array, same content) showed "Corpus updated".
    const mk = (id: number, relevant: boolean) =>
      ({ id, relevant, excluded: false } as unknown as SourceRelevance);
    const first = [mk(3, true), mk(1, true), mk(2, false)];
    const remerged = [mk(1, true), mk(2, false), mk(3, true), mk(4, false)];
    expect(surfacedSignature(remerged)).toBe(surfacedSignature(first));
    expect(surfacedSignature([...remerged, mk(5, true)])).not.toBe(surfacedSignature(first));
  });
});

describe('theme map partition', () => {
  it('puts every node in exactly one of stack / theme / unthemed', () => {
    const nodes = [
      node(1, { cluster_id: 'a' }),
      node(2, { cluster_id: 'a' }),
      node(3, { cluster_id: 'a', affects_you: true }),
      node(4, { affects_you: true }),
      node(5),
    ];
    const map = buildThemeMap(graph(nodes, [cluster('a', [1, 2, 3])]));
    expect(map.stack.map((n) => n.id).sort()).toEqual([3, 4]);
    expect(map.themes).toHaveLength(1);
    expect(map.themes[0]!.items.map((n) => n.id).sort()).toEqual([1, 2]);
    expect(map.unthemed.map((n) => n.id)).toEqual([5]);
    const all = [...map.stack, ...map.themes.flatMap((t) => t.items), ...map.unthemed].map((n) => n.id);
    expect(new Set(all).size).toBe(nodes.length);
    expect(all).toHaveLength(nodes.length);
  });

  it('drops a theme whose every member is a stack item, and keeps backend theme order', () => {
    const nodes = [node(1, { affects_you: true }), node(2), node(3), node(4), node(5)];
    const map = buildThemeMap(graph(nodes, [cluster('z', [4, 5]), cluster('stack-only', [1]), cluster('b', [2, 3])]));
    expect(map.themes.map((t) => t.id)).toEqual(['z', 'b']);
  });

  it('orders items security first, then urgency, then relevance', () => {
    const items = [
      node(1, { relevance_score: 0.9 }),
      node(2, { relevance_score: 0.2, category: 'security' }),
      node(3, { relevance_score: 0.5, signal_priority: 'critical' }),
      node(4, { relevance_score: 0.95 }),
    ].sort(byImportance);
    expect(items.map((n) => n.id)).toEqual([2, 3, 4, 1]);
  });
});

describe('treemap layout', () => {
  const area = (r: { w: number; h: number }) => r.w * r.h;

  it('tiles the whole rectangle with no overlap, areas proportional to weight', () => {
    const weights = [10, 6, 4, 3, 3, 2];
    const rects = binaryTreemap(weights, { x: 0, y: 0, w: 1200, h: 700 });
    const total = weights.reduce((a, b) => a + b, 0);
    const sum = rects.reduce((a, r) => a + area(r), 0);
    expect(sum).toBeCloseTo(1200 * 700, 3);
    rects.forEach((r, i) => expect(area(r) / (1200 * 700)).toBeCloseTo(weights[i]! / total, 6));
    for (let i = 0; i < rects.length; i++) {
      for (let j = i + 1; j < rects.length; j++) {
        const a = rects[i]!;
        const b = rects[j]!;
        const ox = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
        const oy = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
        expect(ox <= 1e-6 || oy <= 1e-6).toBe(true);
      }
    }
  });

  it('keeps tiles near-square on a wide viewport (the old map used 37% of the width)', () => {
    const weights = Array.from({ length: 15 }, (_, i) => 15 - i);
    const rects = binaryTreemap(weights, { x: 0, y: 0, w: 1360, h: 760 });
    const worst = Math.max(...rects.map((r) => Math.max(r.w / r.h, r.h / r.w)));
    expect(worst).toBeLessThan(4);
  });

  it('handles empty, single and zero-weight inputs', () => {
    expect(binaryTreemap([], { x: 0, y: 0, w: 10, h: 10 })).toEqual([]);
    expect(binaryTreemap([5], { x: 1, y: 2, w: 10, h: 10 })).toEqual([{ x: 1, y: 2, w: 10, h: 10 }]);
    const zero = binaryTreemap([0, 0], { x: 0, y: 0, w: 10, h: 10 });
    expect(zero.every((r) => Number.isFinite(r.w) && Number.isFinite(r.h))).toBe(true);
  });
});

describe('tile capacity and text', () => {
  it('shows every title when they fit, otherwise spends the last line on "+N more"', () => {
    const h = 2 * TILE_PAD + TILE_HEADER_H + 4 * TILE_LINE_H;
    expect(visibleLines(h, 3)).toEqual({ shown: 3, more: 0 });
    expect(visibleLines(h, 4)).toEqual({ shown: 4, more: 0 });
    expect(visibleLines(h, 9)).toEqual({ shown: 3, more: 6 });
    expect(visibleLines(10, 2)).toEqual({ shown: 0, more: 2 });
  });

  it('writes theme labels in sentence case without mangling package names', () => {
    expect(displayThemeLabel('rust · cache · next.js')).toBe('Rust · cache · next.js');
    expect(displayThemeLabel('JWT · decoding')).toBe('JWT · decoding');
    expect(displayThemeLabel('')).toBe('');
  });

  it('strips a known source prefix only', () => {
    expect(cleanTitle('crates.io: tokio v1.53.2')).toBe('tokio v1.53.2');
    expect(cleanTitle('npm: @ai-sdk/openai v4.0.83')).toBe('@ai-sdk/openai v4.0.83');
    expect(cleanTitle('Rust 1.80: released')).toBe('Rust 1.80: released');
  });
});
