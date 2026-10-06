// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Signal → Graph: the relationship view. Every item as a mark inside its
// theme (stack items inside the theme their community belongs to), lines
// between related items, laid out client-side to the canvas shape
// (graph-layout.ts). The Themes view is for reading; this one is for seeing
// how items and themes relate.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  type FitViewOptions,
  type Node,
  type Edge,
  type NodeChange,
  useNodesState,
  useEdgesState,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useTranslation } from 'react-i18next';

import type { GraphNode } from '../../types/graph';
import { useTheme } from '../../lib/theme';
import ContentGraphNodeComponent from './ContentGraphNode';
import ContentGraphEdgeComponent from './ContentGraphEdge';
import GraphDetailPanel from './GraphDetailPanel';
import { ClusterHullNode, ClusterLabelNode, LaneLabelNode } from './GraphCanvasChrome';
import { EmptyState, ErrorState, GraphLegend, LoadingState } from './ContentGraphChrome';
import ContentGraphFooter from './ContentGraphFooter';
import { useContentGraph } from './use-content-graph';
import { ZoomCssVar, toFlowEdges, toFlowNodes } from './ContentGraphFlowHelpers';
import { LabelCollisionLayer } from './ContentGraphLabelLayer';
import { GRAPH_CATEGORIES } from './graph-marks';
import { layoutGraph, type RememberedOrder } from './graph-layout';
import { markViewed, readLastViewed, readMemory, writeMemory } from './graph-visit';

const nodeTypes = {
  contentNode: ContentGraphNodeComponent,
  clusterLabel: ClusterLabelNode,
  clusterHull: ClusterHullNode,
  laneLabel: LaneLabelNode,
};
const edgeTypes = { contentEdge: ContentGraphEdgeComponent };

/** Where each window's group order is remembered (per viewer, per device). */
const orderKey = (days: number) => `4da:graph:graph-order:${days}d`;
const isRememberedOrder = (v: unknown): v is RememberedOrder =>
  !!v && Array.isArray((v as RememberedOrder).order) && typeof (v as RememberedOrder).members === 'object';
/** Fit margins in SCREEN px: titles are zoom-invariant and 150 px wide,
 *  centred on their mark, so a flow-unit padding clipped the outer titles at
 *  small windows (live 2026-10-07, 1200x800). */
const FIT_VIEW: FitViewOptions = { padding: { x: '80px', top: '24px', bottom: '20px' } };
/** Canvas aspect, rounded: re-layout only when the canvas SHAPE changes, not
 *  on every pixel of a window drag. */
const aspectBucket = (w: number, h: number) => (w > 0 && h > 0 ? Math.round((w / h) * 10) / 10 : 0);

export default function ContentGraphView() {
  const { t } = useTranslation();
  const { isLight } = useTheme();
  const [days, setDays] = useState(7);
  const { graph, loading, loadError, stale, reload, applyFresh } = useContentGraph(days);
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<GraphNode | null>(null);
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [baseEdges, setBaseEdges] = useState<Edge[]>([]);
  const [aspect, setAspect] = useState(0);
  const [hostEl, setHostEl] = useState<HTMLDivElement | null>(null);
  const flowRef = useRef<{ fitView: (opts?: FitViewOptions) => void } | null>(null);
  const needsFitRef = useRef(false);
  const lastViewedRef = useRef(0);

  // Canvas shape → layout aspect. A resize within the same aspect bucket
  // keeps the layout but re-fits it, so the map never sits half off-canvas.
  useEffect(() => {
    if (!hostEl) return;
    let raf = 0;
    let current = 0;
    const update = () => {
      const next = aspectBucket(hostEl.clientWidth, hostEl.clientHeight);
      if (next !== current) {
        current = next;
        setAspect(next);
      } else if (!needsFitRef.current) {
        cancelAnimationFrame(raf);
        raf = requestAnimationFrame(() => flowRef.current?.fitView(FIT_VIEW));
      }
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(hostEl);
    return () => {
      ro.disconnect();
      cancelAnimationFrame(raf);
    };
  }, [hostEl]);

  // "New" = arrived since the previous visit (shared stamp with Themes).
  useEffect(() => {
    if (!graph) return;
    lastViewedRef.current = readLastViewed();
    markViewed();
    setSelectedNode(null);
  }, [graph]);

  // The order remembered from the previous build, read once per loaded map.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const previousOrder = useMemo(() => readMemory(orderKey(days), isRememberedOrder), [days, graph]);

  const layout = useMemo(
    () => (graph && aspect > 0 ? layoutGraph(graph, t('signals.laneStack'), aspect, previousOrder) : null),
    [graph, aspect, previousOrder, t],
  );

  useEffect(() => {
    if (!graph || !layout) return;
    needsFitRef.current = true;
    setNodes(toFlowNodes(graph, layout, lastViewedRef.current));
    const flowEdges = toFlowEdges(graph, layout);
    setEdges(flowEdges);
    setBaseEdges(flowEdges);
    writeMemory(orderKey(days), { order: layout.order, members: layout.members });
  }, [graph, layout, days, setNodes, setEdges]);

  const connectedNodeIds = useMemo(() => {
    const ids = new Set<string>();
    if (!hoveredNodeId) return ids;
    for (const e of baseEdges) {
      if (e.source === hoveredNodeId) ids.add(e.target);
      if (e.target === hoveredNodeId) ids.add(e.source);
    }
    return ids;
  }, [hoveredNodeId, baseEdges]);

  // Hover focus: the item and its related items full strength, the rest
  // dimmed; the unhover branch resets every node (an early return once left
  // 128 of 129 nodes stuck at 25% — live 2026-07-19).
  useEffect(() => {
    setEdges(
      hoveredNodeId
        ? baseEdges.map((e) => (e.source === hoveredNodeId || e.target === hoveredNodeId ? { ...e, animated: true, style: { opacity: 1 } } : e))
        : baseEdges,
    );
    setNodes((nds) =>
      nds.map((n) => {
        if (n.type !== 'contentNode') return n;
        const opacity = hoveredNodeId === null || n.id === hoveredNodeId || connectedNodeIds.has(n.id) ? 1 : 0.25;
        return { ...n, style: { opacity, transition: 'opacity 200ms ease' } };
      }),
    );
  }, [hoveredNodeId, connectedNodeIds, baseEdges, setEdges, setNodes]);

  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    if (node.type === 'contentNode') setSelectedNode(node.data as unknown as GraphNode);
  }, []);
  const closePanel = useCallback(() => {
    setSelectedNode(null);
    setNodes((nds) => (nds.some((n) => n.selected) ? nds.map((n) => (n.selected ? { ...n, selected: false } : n)) : nds));
  }, [setNodes]);
  const onPaneClick = useCallback(() => setSelectedNode(null), []);
  const onNodeMouseEnter = useCallback((_: React.MouseEvent, node: Node) => {
    if (node.type === 'contentNode') setHoveredNodeId(node.id);
  }, []);
  const onNodeMouseLeave = useCallback(() => setHoveredNodeId(null), []);
  const onInit = useCallback((instance: { fitView: (opts?: FitViewOptions) => void }) => {
    flowRef.current = instance;
  }, []);

  // Fit on the first batch of MEASURED dimensions after a layout: React Flow
  // computes bounds from measured node sizes, which land asynchronously.
  const handleNodesChange = useCallback(
    (changes: NodeChange[]) => {
      onNodesChange(changes);
      if (needsFitRef.current && changes.some((c) => c.type === 'dimensions')) {
        needsFitRef.current = false;
        requestAnimationFrame(() => flowRef.current?.fitView(FIT_VIEW));
      }
    },
    [onNodesChange],
  );

  const categories = useMemo(() => {
    const seen = new Set(graph?.nodes.map((n) => n.category) ?? []);
    return GRAPH_CATEGORIES.filter((c) => seen.has(c));
  }, [graph]);

  if (loading) return <LoadingState />;
  if (loadError) return <ErrorState onRetry={reload} />;
  if (!graph || graph.nodes.length === 0) return <EmptyState />;

  return (
    // Flex column with a DEFINITE height: React Flow's root is height:100%,
    // which resolves to 0 under a min-height-only parent (error #004).
    <div className="flex flex-col" style={{ height: 'calc(100vh - 210px)', minHeight: 500, backgroundColor: 'var(--color-bg-primary)' }}>
      {/* Legend above the canvas, not a Panel on it: a floating panel covered
          the top row's theme names at 1200x800 (live 2026-10-07). */}
      <div className="flex items-center justify-between gap-3 px-4 pb-2">
        <GraphLegend categories={categories} hasStack={graph.nodes.some((n) => n.affects_you)} showEdges={baseEdges.length > 0} />
        {stale && (
          <button
            onClick={applyFresh}
            className="px-2.5 py-1 text-[11px] rounded border transition-colors hover:bg-bg-tertiary"
            style={{ color: 'var(--color-accent-gold)', borderColor: 'var(--color-border)', backgroundColor: 'var(--color-bg-secondary)' }}
          >
            {t('signals.graphCorpusChanged')}
          </button>
        )}
      </div>
      <div ref={setHostEl} className="relative flex flex-col" style={{ flex: '1 1 0%', minHeight: 0 }}>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={handleNodesChange}
          onEdgesChange={onEdgesChange}
          onNodeClick={onNodeClick}
          onPaneClick={onPaneClick}
          onNodeMouseEnter={onNodeMouseEnter}
          onNodeMouseLeave={onNodeMouseLeave}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          onInit={onInit}
          proOptions={{ hideAttribution: true }}
          minZoom={0.1}
          maxZoom={2}
          nodesDraggable
          nodesConnectable={false}
          elementsSelectable
          style={{ flex: '1 1 0%', minHeight: 0 }}
        >
          <ZoomCssVar />
          <LabelCollisionLayer />
          {/* SVG presentation attributes cannot resolve var(): concrete per theme. */}
          <Background color={isLight ? '#DDDAD2' : '#2A2A2A'} gap={20} />
          <Controls
            showInteractive={false}
            style={{ backgroundColor: 'var(--color-bg-secondary)', borderColor: 'var(--color-border)', borderRadius: 8 }}
          />
        </ReactFlow>
        {selectedNode && <GraphDetailPanel key={selectedNode.id} node={selectedNode} onClose={closePanel} />}
      </div>
      <ContentGraphFooter meta={graph.meta} days={days} onDaysChange={setDays} />
    </div>
  );
}
