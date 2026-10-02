// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.resolve({})) }));

import type { ContentGraph, GraphNode } from '../../types/graph';
import type { SourceRelevance } from '../../types';
import { graphNodeSetKey, surfacedSignature } from './use-content-graph';
import { toFlowNodes } from './ContentGraphFlowHelpers';

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
    x: id * 10,
    y: id * 10,
    ...over,
  };
}

function graph(nodes: GraphNode[]): ContentGraph {
  return {
    nodes,
    edges: [],
    clusters: [],
    meta: {} as ContentGraph['meta'],
  };
}

describe('graph staleness signals', () => {
  it('node-set key ignores layout and order — only WHAT the map shows', () => {
    const a = graph([node(1), node(2, { member_ids: [2, 7] })]);
    const b = graph([node(2, { member_ids: [7, 2], x: 999 }), node(1, { y: -5 })]);
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

describe('unconnected lane header', () => {
  it('labels the lane with the count of nodes in no cluster, above the lane', () => {
    const nodes = [
      node(1, { cluster_id: 'cluster_1', y: 0 }),
      node(2, { cluster_id: 'cluster_1', y: 10 }),
      node(3, { y: 500, x: 40 }),
      node(4, { y: 595, x: 20 }),
    ];
    const flow = toFlowNodes(nodes, []);
    const lane = flow.find((n) => n.type === 'laneLabel');
    expect(lane).toBeTruthy();
    expect((lane!.data as { count: number }).count).toBe(2);
    expect(lane!.position.y).toBeLessThan(500);
    expect(lane!.position.x).toBe(20);
  });

  it('renders no lane header when every node is in a cluster', () => {
    const flow = toFlowNodes([node(1, { cluster_id: 'c' }), node(2, { cluster_id: 'c' })], []);
    expect(flow.some((n) => n.type === 'laneLabel')).toBe(false);
  });
});
