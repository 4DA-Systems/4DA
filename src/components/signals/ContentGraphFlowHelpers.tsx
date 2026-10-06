// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Pure mapping + viewport helpers for the Graph view: the client-side layout
// (graph-layout.ts) becomes React Flow nodes and edges.
import { useEffect, useRef } from 'react';
import { useStore, type Node, type Edge } from '@xyflow/react';

import type { ContentGraph } from '../../types/graph';
import type { GraphLayout } from './graph-layout';
import { nodeMarkSize } from './content-graph-label-layout';
import { displayThemeLabel } from './theme-map-model';

/** Below this zoom only the labels that carry the map's meaning render —
 *  theme headers, the shelf header, stack and security items. At fit zoom
 *  every-node labels at a readable size would pile into a smear (audit
 *  2026-10-02). Hover always reveals a title. */
export const LABEL_DETAIL_ZOOM = 0.75;

/** Writes the live viewport zoom to the React Flow wrapper: `--graph-zoom`
 *  (ring/halo widths and label sizes divide by it, holding a constant
 *  on-screen size) and `data-graph-lod` (`far` below LABEL_DETAIL_ZOOM —
 *  content-graph.css hides the non-essential labels there). */
export function ZoomCssVar() {
  const zoom = useStore((s) => s.transform[2]);
  const ref = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    const host = ref.current?.closest('.react-flow') as HTMLElement | null;
    host?.style.setProperty('--graph-zoom', String(zoom));
    host?.setAttribute('data-graph-lod', zoom < LABEL_DETAIL_ZOOM ? 'far' : 'near');
  }, [zoom]);
  return <div ref={ref} style={{ display: 'none' }} />;
}

const STATIC = { selectable: false, draggable: false, connectable: false, focusable: false } as const;

export function toFlowNodes(graph: ContentGraph, layout: GraphLayout, lastViewedMs: number): Node[] {
  const contentNodes: Node[] = [];
  for (const n of graph.nodes) {
    const p = layout.positions.get(n.id);
    if (!p) continue;
    const size = nodeMarkSize(n.member_count, n.relevance_score);
    contentNodes.push({
      id: String(n.id),
      type: 'contentNode' as const,
      // Layout positions are mark CENTRES; React Flow positions are top-left.
      position: { x: p.x - size / 2, y: p.y - size / 2 },
      data: { ...n, isNew: n.created_at ? new Date(n.created_at).getTime() > lastViewedMs : false },
    });
  }

  // Hulls paint UNDER members (array order = paint order). Pointer events
  // off on the WRAPPER, or the disc swallows pane drags and node clicks.
  const hullNodes: Node[] = layout.groups.map((g) => ({
    id: `hull-${g.key}`,
    type: 'clusterHull' as const,
    position: { x: g.cx - g.radius, y: g.cy - g.radius },
    data: { radius: g.radius, stack: g.kind === 'stack' },
    ...STATIC,
    style: { pointerEvents: 'none' as const },
  }));

  const headerNodes: Node[] = layout.groups.map((g) => ({
    id: `cluster-${g.key}`,
    type: 'clusterLabel' as const,
    // Anchor in the hull's name band (graph-layout.ts HEADER_BAND), clear of
    // the members.
    position: { x: g.cx, y: g.headerY },
    // radius: how far LabelCollisionLayer may nudge the header in its hull.
    data: {
      label: g.kind === 'stack' ? g.label : displayThemeLabel(g.label),
      count: g.nodeIds.length,
      radius: g.radius,
      stack: g.kind === 'stack',
    },
    ...STATIC,
    style: { pointerEvents: 'none' as const },
  }));

  // The shelf: items related to no theme, said plainly by its header.
  const laneHeader: Node[] = layout.shelf
    ? [
        {
          id: 'lane-unconnected',
          type: 'laneLabel' as const,
          position: { x: layout.shelf.x, y: layout.shelf.y },
          data: { count: layout.shelf.nodeIds.length },
          ...STATIC,
          style: { pointerEvents: 'none' as const },
        },
      ]
    : [];

  return [...hullNodes, ...contentNodes, ...headerNodes, ...laneHeader];
}

export function toFlowEdges(graph: ContentGraph, layout: GraphLayout): Edge[] {
  return graph.edges
    .filter((e) => layout.positions.has(e.source) && layout.positions.has(e.target))
    .map((e, i) => ({
      id: `e-${e.source}-${e.target}-${i}`,
      source: String(e.source),
      target: String(e.target),
      type: 'contentEdge' as const,
      data: { weight: e.weight, label: e.label },
    }));
}
