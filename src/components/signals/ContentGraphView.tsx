// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState, useCallback } from 'react';
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

import { cmd } from '../../lib/commands';
import type {
  ContentGraph,
  GraphNode as ContentGraphNode,
  GraphEdge as ContentGraphEdge,
  GraphCluster,
} from '../../types/graph';
import ContentGraphNodeComponent, { SOURCE_COLORS, type ContentNode } from './ContentGraphNode';
import ContentGraphEdgeComponent from './ContentGraphEdge';

function toFlowNodes(graphNodes: ContentGraphNode[], clusters: GraphCluster[]): Node[] {
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
        color: '#A0A0A0',
        fontSize: 11,
        fontWeight: 600,
        fontFamily: 'Inter, sans-serif',
        letterSpacing: '0.03em',
        textTransform: 'uppercase',
        pointerEvents: 'none',
        whiteSpace: 'nowrap',
        textShadow: '0 1px 4px rgba(0,0,0,0.8)',
        transform: 'translateX(-50%)',
      }}
    >
      {data.label}
      <span style={{ color: '#8A8A8A', fontWeight: 400, marginLeft: 4, fontSize: 10 }}>
        ({data.count})
      </span>
    </div>
  );
}

function LoadingState() {
  return (
    <div className="h-full min-h-[500px] flex items-center justify-center" style={{ backgroundColor: '#0A0A0A' }}>
      <div className="flex flex-col items-center gap-3">
        <div className="w-8 h-8 border-2 border-white/30 border-t-white rounded-full animate-spin" />
        <span style={{ color: '#A0A0A0', fontSize: 13, fontFamily: 'Inter, sans-serif' }}>
          Building content graph...
        </span>
      </div>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="h-full min-h-[500px] flex items-center justify-center" style={{ backgroundColor: '#0A0A0A' }}>
      <div className="flex flex-col items-center gap-2">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="#8A8A8A" strokeWidth="1.5">
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
        <span style={{ color: '#8A8A8A', fontSize: 14, fontFamily: 'Inter, sans-serif' }}>
          No content relationships found
        </span>
        <span style={{ color: '#6B7280', fontSize: 12, fontFamily: 'Inter, sans-serif' }}>
          Connections will appear as content is analyzed
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

export default function ContentGraphView() {
  const [loading, setLoading] = useState(true);
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);

  useEffect(() => {
    let cancelled = false;

    cmd('build_content_graph', { days: 7, max_nodes: 150 })
      .then((graph: ContentGraph) => {
        if (cancelled) return;
        setNodes(toFlowNodes(graph.nodes, graph.clusters));
        setEdges(toFlowEdges(graph.edges));
      })
      .catch((err) => {
        if (!cancelled) console.error('[ContentGraph] Failed to load:', err);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [setNodes, setEdges]);

  const onInit = useCallback((instance: { fitView: () => void }) => {
    instance.fitView();
  }, []);

  const isEmpty = !loading && nodes.length === 0;

  if (loading) return <LoadingState />;
  if (isEmpty) return <EmptyState />;

  return (
    <div className="h-full min-h-[500px]" style={{ backgroundColor: '#0A0A0A' }}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
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
        <Background color="#2A2A2A" gap={20} />
        <Controls
          showInteractive={false}
          style={{
            backgroundColor: '#141414',
            borderColor: '#2A2A2A',
            borderRadius: 8,
          }}
        />
        <MiniMap
          nodeColor={minimapNodeColor}
          maskColor="rgba(10, 10, 10, 0.85)"
          style={{
            backgroundColor: '#141414',
            borderColor: '#2A2A2A',
          }}
        />
      </ReactFlow>
    </div>
  );
}
