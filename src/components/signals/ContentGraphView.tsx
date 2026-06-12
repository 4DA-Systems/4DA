// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState, useCallback, useMemo } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  type Node,
  type Edge,
  useNodesState,
  useEdgesState,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import { useTheme } from '../../lib/theme';
import type {
  ContentGraph,
  GraphNode as ContentGraphNode,
  GraphEdge as ContentGraphEdge,
  GraphCluster,
} from '../../types/graph';
import ContentGraphNodeComponent, { SOURCE_COLORS, type ContentNode } from './ContentGraphNode';
import ContentGraphEdgeComponent from './ContentGraphEdge';

const LAST_VIEW_KEY = '4da:graph:lastViewedAt';

function toFlowNodes(graphNodes: ContentGraphNode[], clusters: GraphCluster[]): Node[] {
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
      isNew: n.created_at ? new Date(n.created_at).getTime() > lastViewedMs : false,
    },
  }));

  const clusterNodes: Node[] = clusters.map((c) => ({
    id: `cluster-${c.id}`,
    type: 'clusterLabel' as const,
    position: { x: c.centroid_x, y: c.centroid_y - 30 },
    data: { label: c.label, count: c.source_count },
    selectable: false,
    draggable: false,
    connectable: false,
  }));

  return [...contentNodes, ...clusterNodes];
}

function toFlowEdges(graphEdges: ContentGraphEdge[]): Edge[] {
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

function ClusterLabelNode({ data }: { data: { label: string; count: number } }) {
  return (
    <div
      style={{
        color: 'var(--color-text-secondary)',
        fontSize: 11,
        fontWeight: 600,
        fontFamily: 'Inter, sans-serif',
        letterSpacing: '0.03em',
        textTransform: 'uppercase',
        pointerEvents: 'none',
        whiteSpace: 'nowrap',
        // Halo in the page color lifts the label off edge lines in both themes
        textShadow: '0 1px 4px var(--color-bg-primary)',
        transform: 'translateX(-50%)',
      }}
    >
      {data.label}
      <span style={{ color: 'var(--color-text-muted)', fontWeight: 400, marginLeft: 4, fontSize: 10 }}>
        ({data.count})
      </span>
    </div>
  );
}

function LoadingState() {
  const { t } = useTranslation();
  return (
    <div className="h-full min-h-[500px] flex items-center justify-center" style={{ backgroundColor: 'var(--color-bg-primary)' }}>
      <div className="flex flex-col items-center gap-3">
        <div className="w-8 h-8 border-2 border-text-primary/30 border-t-text-primary rounded-full animate-spin" />
        <span style={{ color: 'var(--color-text-secondary)', fontSize: 13, fontFamily: 'Inter, sans-serif' }}>
          {t('action.loading')}
        </span>
      </div>
    </div>
  );
}

function EmptyState() {
  const { t } = useTranslation();
  return (
    <div className="h-full min-h-[500px] flex items-center justify-center" style={{ backgroundColor: 'var(--color-bg-primary)' }}>
      <div className="flex flex-col items-center gap-2">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" className="stroke-text-muted" strokeWidth="1.5">
          <circle cx="12" cy="12" r="3" />
          <circle cx="4" cy="8" r="2" />
          <circle cx="20" cy="8" r="2" />
          <circle cx="4" cy="16" r="2" />
          <circle cx="20" cy="16" r="2" />
          <line x1="9.5" y1="10.5" x2="5.5" y2="8.5" />
          <line x1="14.5" y1="10.5" x2="18.5" y2="8.5" />
          <line x1="9.5" y1="13.5" x2="5.5" y2="15.5" />
          <line x1="14.5" y1="13.5" x2="18.5" y2="15.5" />
        </svg>
        <span style={{ color: 'var(--color-text-muted)', fontSize: 14, fontFamily: 'Inter, sans-serif' }}>
          {t('signals.graphEmpty')}
        </span>
        <span style={{ color: 'var(--color-text-muted)', fontSize: 12, fontFamily: 'Inter, sans-serif' }}>
          {t('signals.graphEmptySub')}
        </span>
      </div>
    </div>
  );
}

const nodeTypes = { contentNode: ContentGraphNodeComponent, clusterLabel: ClusterLabelNode };
const edgeTypes = { contentEdge: ContentGraphEdgeComponent };

function minimapNodeColor(node: Node): string {
  const data = node.data as ContentNode['data'] | undefined;
  if (!data?.source_type) return '#6B7280';
  return SOURCE_COLORS[data.source_type] ?? '#6B7280';
}

function openExternal(url: string) {
  import('@tauri-apps/plugin-opener')
    .then(({ openUrl }) => openUrl(url))
    .catch(() => window.open(url, '_blank', 'noopener,noreferrer'));
}

const TIME_WINDOWS = [7, 14, 30] as const;

export default function ContentGraphView() {
  const { t } = useTranslation();
  const { isLight } = useTheme();
  const [days, setDays] = useState(7);
  const [loading, setLoading] = useState(true);
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const [meta, setMeta] = useState<ContentGraph['meta'] | null>(null);
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [baseEdges, setBaseEdges] = useState<Edge[]>([]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    cmd('build_content_graph', { days, maxNodes: 150 })
      .then((graph: ContentGraph) => {
        if (cancelled) return;
        setNodes(toFlowNodes(graph.nodes, graph.clusters));
        const flowEdges = toFlowEdges(graph.edges);
        setEdges(flowEdges);
        setBaseEdges(flowEdges);
        setMeta(graph.meta);
        localStorage.setItem(LAST_VIEW_KEY, new Date().toISOString());
      })
      .catch((err) => {
        if (!cancelled) console.error('[ContentGraph] Failed to load:', err);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [days, setNodes, setEdges]);

  const connectedNodeIds = useMemo(() => {
    if (!hoveredNodeId) return new Set<string>();
    const ids = new Set<string>();
    for (const e of baseEdges) {
      if (e.source === hoveredNodeId) ids.add(e.target);
      if (e.target === hoveredNodeId) ids.add(e.source);
    }
    return ids;
  }, [hoveredNodeId, baseEdges]);

  useEffect(() => {
    if (!hoveredNodeId) {
      setEdges(baseEdges);
      return;
    }
    setEdges(baseEdges.map((e) => {
      const connected = e.source === hoveredNodeId || e.target === hoveredNodeId;
      return { ...e, animated: connected, style: connected ? { ...((e.style as Record<string, unknown>) ?? {}), opacity: 1 } : undefined };
    }));
  }, [hoveredNodeId, baseEdges, setEdges]);

  useEffect(() => {
    if (!hoveredNodeId) return;
    setNodes((nds) => nds.map((n) => {
      if (n.type === 'clusterLabel') return n;
      const dimmed = n.id !== hoveredNodeId && !connectedNodeIds.has(n.id);
      return { ...n, style: dimmed ? { opacity: 0.25, transition: 'opacity 200ms ease' } : { opacity: 1, transition: 'opacity 200ms ease' } };
    }));
  }, [hoveredNodeId, connectedNodeIds, setNodes]);

  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    if (node.type === 'clusterLabel') return;
    const data = node.data as ContentNode['data'];
    const itemId = Number(node.id);
    if (!Number.isNaN(itemId)) {
      cmd('record_interaction', { sourceItemId: itemId, action: 'click' }).catch(() => {});
    }
    if (data?.url) openExternal(data.url);
  }, []);

  const onNodeMouseEnter = useCallback((_: React.MouseEvent, node: Node) => {
    if (node.type !== 'clusterLabel') setHoveredNodeId(node.id);
  }, []);

  const onNodeMouseLeave = useCallback(() => {
    setHoveredNodeId(null);
  }, []);

  const onInit = useCallback((instance: { fitView: () => void }) => {
    instance.fitView();
  }, []);

  const isEmpty = !loading && nodes.length === 0;

  if (loading) return <LoadingState />;
  if (isEmpty) return <EmptyState />;

  return (
    <div className="h-full min-h-[500px]" style={{ backgroundColor: 'var(--color-bg-primary)' }}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onNodeClick={onNodeClick}
        onNodeMouseEnter={onNodeMouseEnter}
        onNodeMouseLeave={onNodeMouseLeave}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        onInit={onInit}
        proOptions={{ hideAttribution: true }}
        minZoom={0.1}
        maxZoom={2}
        fitView
        nodesDraggable
        nodesConnectable={false}
        elementsSelectable
      >
        {/* React Flow paints these via SVG presentation attributes, which
            cannot resolve var() — resolve concrete values per theme here */}
        <Background color={isLight ? '#DDDAD2' : '#2A2A2A'} gap={20} />
        <Controls
          showInteractive={false}
          style={{
            backgroundColor: 'var(--color-bg-secondary)',
            borderColor: 'var(--color-border)',
            borderRadius: 8,
          }}
        />
        <MiniMap
          nodeColor={minimapNodeColor}
          maskColor={isLight ? 'rgba(246, 245, 242, 0.85)' : 'rgba(10, 10, 10, 0.85)'}
          style={{
            backgroundColor: 'var(--color-bg-secondary)',
            borderColor: 'var(--color-border)',
          }}
        />
      </ReactFlow>
      <div
        className="flex items-center justify-between px-4 py-2 border-t"
        style={{ backgroundColor: 'var(--color-bg-secondary)', borderColor: 'var(--color-border)' }}
      >
        <div className="flex gap-4 text-[11px]" style={{ color: 'var(--color-text-muted)', fontFamily: 'JetBrains Mono, monospace' }}>
          {meta && (
            <>
              <span>{meta.total_items} {t('signals.graphNodes', 'nodes')}</span>
              <span>{meta.total_edges} {t('signals.graphEdges', 'edges')}</span>
              <span>{meta.cluster_count} {t('signals.graphClusters', 'clusters')}</span>
            </>
          )}
        </div>
        <div className="flex items-center gap-1">
          {TIME_WINDOWS.map((w) => (
            <button
              key={w}
              onClick={() => setDays(w)}
              className={`px-2 py-0.5 text-[10px] rounded transition-colors ${
                days === w
                  ? 'bg-bg-tertiary text-text-primary'
                  : 'text-text-muted hover:text-text-secondary'
              }`}
              style={{ fontFamily: 'JetBrains Mono, monospace' }}
            >
              {/* eslint-disable-next-line i18next/no-literal-string */}
              {w}d
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
