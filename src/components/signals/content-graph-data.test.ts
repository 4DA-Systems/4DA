// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.resolve({})) }));

import type { ContentGraph, GraphNode } from '../../types/graph';
import type { SourceRelevance } from '../../types';
import { graphNodeSetKey, surfacedSignature } from './use-content-graph';
import { toFlowNodes } from './ContentGraphFlowHelpers';
import { applyLabelPlacements } from './ContentGraphLabelLayer';

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
  it('labels the lane with the count of nodes in no cluster, anchored at the first row', () => {
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
    // The node sits at the first row's top; LaneLabelNode hangs the header
    // above it (translateY -100%) so zoom-invariant text grows upward only.
    expect(lane!.position.y).toBe(500);
    expect(lane!.position.x).toBe(20);
  });

  it('hands each cluster header its hull radius (the room it may be nudged in)', () => {
    const nodes = [
      node(1, { cluster_id: 'c', x: 100, y: 0 }),
      node(2, { cluster_id: 'c', x: 0, y: 0 }),
    ];
    const clusters = [{ id: 'c', label: 'rust', node_ids: [1, 2], source_count: 1, coherence: 0, centroid_x: 0, centroid_y: 0 }];
    const header = toFlowNodes(nodes, clusters).find((n) => n.type === 'clusterLabel')!;
    expect((header.data as { radius: number }).radius).toBe(160); // 100 + HULL_PADDING
  });

  it('renders no lane header when every node is in a cluster', () => {
    const flow = toFlowNodes([node(1, { cluster_id: 'c' }), node(2, { cluster_id: 'c' })], []);
    expect(flow.some((n) => n.type === 'laneLabel')).toBe(false);
  });
});

describe('label placements reach the DOM', () => {
  it('writes side + suppression onto node labels and the nudge onto headers', () => {
    const host = document.createElement('div');
    host.innerHTML =
      '<span data-cg-label-id="node:1"></span><span data-cg-label-id="node:2"></span>' +
      '<span data-cg-label-id="node:3"></span><div data-cg-label-id="cluster:cluster-c"></div>';
    applyLabelPlacements(
      host,
      new Map([
        ['node:1', { visible: true, offset: { dx: 0, dy: -40, key: 'above' } }],
        ['node:2', { visible: false, offset: { dx: 0, dy: 0, key: 'below' } }],
        ['cluster:cluster-c', { visible: true, offset: { dx: 12, dy: -30, key: 'nudge' } }],
      ]),
    );
    const el = (id: string) => host.querySelector<HTMLElement>(`[data-cg-label-id="${id}"]`)!;
    expect(el('node:1').dataset.cgPlace).toBe('above');
    expect(el('node:1').dataset.cgSuppressed).toBe('false');
    expect(el('node:2').dataset.cgSuppressed).toBe('true');
    // Not considered (LOD-hidden): default placement, never suppressed.
    expect(el('node:3').dataset.cgPlace).toBe('below');
    expect(el('node:3').dataset.cgSuppressed).toBe('false');
    expect(el('cluster:cluster-c').style.getPropertyValue('--cg-dx')).toBe('12px');
    expect(el('cluster:cluster-c').style.getPropertyValue('--cg-dy')).toBe('-30px');
  });
});
