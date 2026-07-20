// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Pure mapping + viewport helpers for the content graph view. Extracted from
// ContentGraphView so the view owns only data flow + interactions (size gate).
import { useEffect, useRef } from 'react';
import { useStore, type Node, type Edge } from '@xyflow/react';

import type {
  GraphNode as ContentGraphNode,
  GraphEdge as ContentGraphEdge,
  GraphCluster,
} from '../../types/graph';
import { CATEGORY_COLORS, type ContentNode } from './ContentGraphNode';

const LAST_VIEW_KEY = '4da:graph:lastViewedAt';

/** Free space between a cluster's outermost member and its hull ring. */
const HULL_PADDING = 60;

/** Resting opacity for nodes that do NOT touch the user's stack — the stack
 *  is the figure, everything else is ground (founder decision 2026-07-20:
 *  stack-relevance must read at any distance; luminance is the channel that
 *  survives far zoom). Categories keep their hues, just quieted. */
export const NON_STACK_OPACITY = 0.42;

/** Writes the live viewport zoom to a CSS variable on the React Flow wrapper.
 *  Stack ring/halo widths divide by it (calc(px / var(--graph-zoom))), so the
 *  gold beacon holds a constant ON-SCREEN size at every zoom level instead of
 *  vanishing at fit view. Renders nothing; updates bypass React entirely. */
export function ZoomCssVar() {
  const zoom = useStore((s) => s.transform[2]);
  const ref = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    const host = ref.current?.closest('.react-flow') as HTMLElement | null;
    host?.style.setProperty('--graph-zoom', String(zoom));
  }, [zoom]);
  return <div ref={ref} style={{ display: 'none' }} />;
}

export function toFlowNodes(graphNodes: ContentGraphNode[], clusters: GraphCluster[]): Node[] {
  const lastViewed = localStorage.getItem(LAST_VIEW_KEY);
  const lastViewedMs = lastViewed ? new Date(lastViewed).getTime() : 0;

  const contentNodes: Node[] = graphNodes.map((n) => ({
    id: String(n.id),
    type: 'contentNode' as const,
    position: { x: n.x, y: n.y },
    data: {
      title: n.title,
      url: n.url,
      source_type: n.source_type,
      relevance_score: n.relevance_score,
      signal_type: n.signal_type,
      signal_priority: n.signal_priority,
      primary_topic: n.primary_topic,
      cluster_id: n.cluster_id,
      member_count: n.member_count,
      member_ids: n.member_ids,
      category: n.category,
      affects_you: n.affects_you,
      isNew: n.created_at ? new Date(n.created_at).getTime() > lastViewedMs : false,
    },
    // First paint matches the resting figure-ground state; the hover effect
    // owns opacity from then on (single source — see the dim effect).
    style: { opacity: n.affects_you ? 1 : NON_STACK_OPACITY },
  }));

  // Hull discs render UNDER members (array order = paint order) so theme
  // grouping is visible at fit zoom — proximity alone stops carrying it once
  // clusters shrink to 2-5 members.
  const positionOf = new Map(graphNodes.map((n) => [n.id, { x: n.x, y: n.y }]));
  const hullNodes: Node[] = clusters.map((c) => {
    let radius = 0;
    for (const id of c.node_ids) {
      const p = positionOf.get(id);
      if (!p) continue;
      const d = Math.hypot(p.x - c.centroid_x, p.y - c.centroid_y);
      if (d > radius) radius = d;
    }
    radius += HULL_PADDING;
    return {
      id: `hull-${c.id}`,
      type: 'clusterHull' as const,
      position: { x: c.centroid_x - radius, y: c.centroid_y - radius },
      data: { radius },
      selectable: false,
      draggable: false,
      connectable: false,
      focusable: false,
      // On the WRAPPER, not just the inner div: otherwise the disc swallows
      // pane clicks/drags across its whole area (deselect + panning break).
      style: { pointerEvents: 'none' as const },
    };
  });

  const clusterNodes: Node[] = clusters.map((c) => ({
    id: `cluster-${c.id}`,
    type: 'clusterLabel' as const,
    position: { x: c.centroid_x, y: c.centroid_y - 30 },
    // Show the cluster's ITEM count (node_ids), not source_count — the latter
    // is almost always 1 (clusters form from same-source neighbours), so it
    // read as a meaningless "(1)" on every label (doctrine rule 3: no vanity
    // metrics). Item count tells the user how big the cluster actually is.
    data: { label: c.label, count: c.node_ids.length },
    selectable: false,
    draggable: false,
    connectable: false,
    // On the WRAPPER (like the hulls): label wrappers otherwise intercept
    // clicks on nodes directly beneath them (live find, 2026-07-20 tour).
    style: { pointerEvents: 'none' as const },
  }));

  return [...hullNodes, ...contentNodes, ...clusterNodes];
}

export function toFlowEdges(graphEdges: ContentGraphEdge[]): Edge[] {
  return graphEdges.map((e, i) => ({
    id: `e-${e.source}-${e.target}-${i}`,
    source: String(e.source),
    target: String(e.target),
    type: 'contentEdge' as const,
    data: {
      edge_type: e.edge_type,
      weight: e.weight,
      label: e.label,
      methods: e.methods,
    },
  }));
}

// Only content nodes appear in the minimap — label/hull helper nodes drew as
// phantom gray dots (live audit 2026-07-19). Stack nodes render gold so the
// overview answers "where is my stack" before the canvas does.
export function minimapNodeColor(node: Node): string {
  if (node.type !== 'contentNode') return 'transparent';
  const data = node.data as ContentNode['data'] | undefined;
  if (data?.affects_you) return 'var(--color-accent-gold)';
  if (!data?.category) return '#6B7280';
  return CATEGORY_COLORS[data.category] ?? '#6B7280';
}

/** Stamp the last-viewed marker (drives the isNew pulse on next visit). */
export function markGraphViewed() {
  localStorage.setItem(LAST_VIEW_KEY, new Date().toISOString());
}
