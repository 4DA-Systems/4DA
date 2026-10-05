// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.resolve({})) }));

import type { ContentGraph, GraphCluster, GraphNode } from '../../types/graph';
import type { SourceRelevance } from '../../types';
import { graphNodeSetKey, surfacedSignature } from './use-content-graph';
import {
  TILE_GAP,
  TILE_HEADER_H,
  TILE_LINE_H,
  TILE_PAD,
  stripTreemap,
  buildThemeMap,
  byImportance,
  cleanTitle,
  MIN_TILE_AREA,
  displayThemeLabel,
  themeWeights,
  tileColumns,
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
    const rects = stripTreemap(weights, { x: 0, y: 0, w: 1200, h: 700 });
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

  it('keeps tiles text-shaped at both window sizes measured live', () => {
    // 18 themes as seen 2026-10-05, after the minimum-area offset.
    const counts = [2, 10, 3, 4, 5, 2, 2, 12, 3, 2, 18, 2, 13, 2, 6, 8, 2, 2];
    for (const [w, h] of [[820, 480], [1360, 860]] as const) {
      const rects = stripTreemap(themeWeights(counts, w * h), { x: 0, y: 0, w, h });
      for (const r of rects) {
        // Readable: wide enough for a title, tall enough for header + two
        // lines, never a tall sliver. (Wide is fine — titles are lines.)
        // The painted tile is TILE_GAP smaller than its layout box.
        expect(r.w - TILE_GAP).toBeGreaterThan(120);
        expect(visibleLines(r.h - TILE_GAP, 99).shown).toBeGreaterThanOrEqual(2);
        expect(r.h / r.w).toBeLessThan(1.5);
      }
    }
  });

  it('reads in list order: row by row, left to right', () => {
    const rects = stripTreemap([3, 3, 3, 3, 3, 3], { x: 0, y: 0, w: 900, h: 300 });
    for (let i = 1; i < rects.length; i++) {
      const a = rects[i - 1]!;
      const b = rects[i]!;
      expect(b.y > a.y + 1e-6 || (Math.abs(b.y - a.y) < 1e-6 && b.x > a.x)).toBe(true);
    }
  });

  it('gives the smallest theme at least the minimum readable area, sizes still ordered', () => {
    const counts = [14, 13, 8, 5, 3, 2, 2, 2];
    const area = 820 * 500;
    const weights = themeWeights(counts, area);
    const rects = stripTreemap(weights, { x: 0, y: 0, w: 820, h: 500 });
    for (const r of rects) expect(r.w * r.h).toBeGreaterThanOrEqual(MIN_TILE_AREA - 1e-6);
    for (let i = 1; i < counts.length; i++) {
      if (counts[i]! < counts[i - 1]!) expect(weights[i]!).toBeLessThan(weights[i - 1]!);
    }
  });

  it('falls back to equal tiles when the space cannot hold every theme at the minimum', () => {
    expect(themeWeights([10, 2, 2], 3 * MIN_TILE_AREA - 1)).toEqual([1, 1, 1]);
    expect(themeWeights([], 1000)).toEqual([]);
  });

  it('handles empty, single and zero-weight inputs', () => {
    expect(stripTreemap([], { x: 0, y: 0, w: 10, h: 10 })).toEqual([]);
    expect(stripTreemap([5], { x: 1, y: 2, w: 10, h: 10 })).toEqual([{ x: 1, y: 2, w: 10, h: 10 }]);
    const zero = stripTreemap([0, 0], { x: 0, y: 0, w: 10, h: 10 });
    expect(zero.every((r) => Number.isFinite(r.w) && Number.isFinite(r.h))).toBe(true);
  });
});

describe('tile capacity and text', () => {
  it('shows every title when they fit, otherwise spends the last line on "+N more"', () => {
    const h = 2 * TILE_PAD + TILE_HEADER_H + 4 * TILE_LINE_H;
    expect(visibleLines(h, 3)).toEqual({ shown: 3, more: 0 });
    expect(visibleLines(h, 4)).toEqual({ shown: 4, more: 0 });
    expect(visibleLines(h, 9)).toEqual({ shown: 3, more: 6 });
    // Under 3 lines every line is a title; the header carries the count.
    const twoLines = 2 * TILE_PAD + TILE_HEADER_H + 2 * TILE_LINE_H;
    expect(visibleLines(twoLines, 5)).toEqual({ shown: 2, more: 0 });
    expect(visibleLines(10, 2)).toEqual({ shown: 0, more: 0 });
  });

  it('spreads a wide tile over title columns and counts slots across them', () => {
    expect(tileColumns(250)).toBe(1);
    expect(tileColumns(700)).toBe(2);
    expect(tileColumns(1080)).toBe(3);
    expect(tileColumns(4000)).toBe(3);
    // 1080×115 live: 2 lines × 3 columns = 6 slots → 5 titles + "+8 more".
    const h = 2 * TILE_PAD + TILE_HEADER_H + 2 * TILE_LINE_H + 10;
    expect(visibleLines(h, 13, 3)).toEqual({ shown: 5, more: 8 });
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
