// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState, useCallback, useMemo, useRef } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  Panel,
  type Node,
  type Edge,
  type NodeChange,
  useNodesState,
  useEdgesState,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import { useTheme } from '../../lib/theme';
import { useAppStore } from '../../store';
import type { ContentGraph } from '../../types/graph';
import ContentGraphNodeComponent, { type ContentNode } from './ContentGraphNode';
import ContentGraphEdgeComponent from './ContentGraphEdge';
import GraphDetailPanel from './GraphDetailPanel';
import {
  ClusterLabelNode,
  ClusterHullNode,
  LoadingState,
  EmptyState,
  ErrorState,
  GraphLegend,
} from './ContentGraphChrome';
import {
  NON_STACK_OPACITY,
  ZoomCssVar,
  toFlowNodes,
  toFlowEdges,
  minimapNodeColor,
  markGraphViewed,
} from './ContentGraphFlowHelpers';

const nodeTypes = {
  contentNode: ContentGraphNodeComponent,
  clusterLabel: ClusterLabelNode,
  clusterHull: ClusterHullNode,
};
const edgeTypes = { contentEdge: ContentGraphEdgeComponent };



// Fixed legend order: most-urgent first. Category identity is color + shape
// (never hue alone), so the legend swatches repeat the node silhouettes.
const LEGEND_CATEGORIES = ['security', 'release', 'discussion', 'research'] as const;

const TIME_WINDOWS = [7, 14, 30] as const;

export default function ContentGraphView() {
  const { t } = useTranslation();
  const { isLight } = useTheme();
  const [days, setDays] = useState(7);
  const [loading, setLoading] = useState(true);
  // A failed build is an ERROR, not an empty corpus — rendering EmptyState on
  // failure told users "no data" when the backend was down (audit 2026-07-19).
  const [loadError, setLoadError] = useState(false);
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [meta, setMeta] = useState<ContentGraph['meta'] | null>(null);
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [baseEdges, setBaseEdges] = useState<Edge[]>([]);
  // Corpus snapshot the current graph was built against — when a new analysis
  // lands, the map is stale and says so instead of silently contradicting the
  // header (live: corpus went 32→430 mid-session with no refresh path).
  const relevanceResults = useAppStore((s) => s.appState.relevanceResults);
  const builtAgainstRef = useRef<unknown>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const reload = useCallback(() => setReloadToken((n) => n + 1), []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setLoadError(false);

    cmd('build_content_graph', { days, maxNodes: 150 })
      .then((graph: ContentGraph) => {
        if (cancelled) return;
        needsFitRef.current = true;
        setSelectedNodeId(null);
        setNodes(toFlowNodes(graph.nodes, graph.clusters));
        const flowEdges = toFlowEdges(graph.edges);
        setEdges(flowEdges);
        setBaseEdges(flowEdges);
        setMeta(graph.meta);
        builtAgainstRef.current = useAppStore.getState().appState.relevanceResults;
        markGraphViewed();
      })
      .catch((err) => {
        if (cancelled) return;
        console.error('[ContentGraph] Failed to load:', err);
        setLoadError(true);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [days, reloadToken, setNodes, setEdges]);

  const corpusChanged =
    !loading && builtAgainstRef.current !== null && relevanceResults !== builtAgainstRef.current;

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

  // Single source of node opacity. Resting state: stack nodes at 1, the rest
  // at NON_STACK_OPACITY (figure-ground). Hovering: the hovered node and its
  // neighbors go to full brightness — even non-stack ones, focus intent wins
  // — and everything else drops to 0.25. The unhover reset branch is
  // load-bearing: an early return on null left 128/129 nodes stuck at 25%
  // after the first hover (live-verified 2026-07-19).
  useEffect(() => {
    setNodes((nds) => nds.map((n) => {
      if (n.type !== 'contentNode') return n;
      const d = n.data as ContentNode['data'];
      let opacity: number;
      if (hoveredNodeId === null) {
        opacity = d.affects_you ? 1 : NON_STACK_OPACITY;
      } else if (n.id === hoveredNodeId || connectedNodeIds.has(n.id)) {
        opacity = 1;
      } else {
        opacity = 0.25;
      }
      return { ...n, style: { opacity, transition: 'opacity 200ms ease' } };
    }));
  }, [hoveredNodeId, connectedNodeIds, setNodes]);

  // Selecting a node opens the in-app detail panel — the graph is a tool, not
  // a launcher. Engagement ('click') is recorded when the user actually opens
  // a link from the panel, so panel-browsing never pollutes the learning loop.
  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    if (node.type === 'clusterLabel') return;
    setSelectedNodeId(node.id);
  }, []);

  const closePanel = useCallback(() => {
    setSelectedNodeId(null);
    // Clear React Flow's own selection so the ring matches the panel state.
    setNodes((nds) => (nds.some((n) => n.selected) ? nds.map((n) => (n.selected ? { ...n, selected: false } : n)) : nds));
  }, [setNodes]);

  const onPaneClick = useCallback(() => {
    setSelectedNodeId(null);
  }, []);

  const onNodeMouseEnter = useCallback((_: React.MouseEvent, node: Node) => {
    if (node.type !== 'clusterLabel') setHoveredNodeId(node.id);
  }, []);

  const onNodeMouseLeave = useCallback(() => {
    setHoveredNodeId(null);
  }, []);

  const flowRef = useRef<{ fitView: (opts?: { padding?: number }) => void } | null>(null);
  const needsFitRef = useRef(false);

  const onInit = useCallback((instance: { fitView: (opts?: { padding?: number }) => void }) => {
    flowRef.current = instance;
    instance.fitView();
  }, []);

  // The graph loads AFTER React Flow mounts, so the onInit fitView runs on an
  // empty canvas and data arrives into a stale viewport (tiny graph in one
  // corner). Fitting on a timer/rAF is unreliable — React Flow computes
  // bounds from MEASURED node dimensions, which land asynchronously. So: arm
  // a flag on data load, and fit on the first batch of dimension changes.
  const handleNodesChange = useCallback(
    (changes: NodeChange[]) => {
      onNodesChange(changes);
      if (needsFitRef.current && changes.some((c) => c.type === 'dimensions')) {
        needsFitRef.current = false;
        requestAnimationFrame(() => flowRef.current?.fitView({ padding: 0.12 }));
      }
    },
    [onNodesChange],
  );

  // Categories + edge types present in the current graph (fixed order, never
  // re-ranked) + whether any node touches the user's stack — drives the legend.
  const legend = useMemo(() => {
    const seen = new Set<string>();
    let anyAffects = false;
    for (const n of nodes) {
      if (n.type !== 'contentNode') continue;
      const d = n.data as ContentNode['data'];
      if (d.category) seen.add(d.category);
      if (d.affects_you) anyAffects = true;
    }
    const edgeTypes = new Set<string>();
    for (const e of baseEdges) {
      const d = e.data as { edge_type?: string } | undefined;
      if (d?.edge_type) edgeTypes.add(d.edge_type);
    }
    return {
      categories: LEGEND_CATEGORIES.filter((c) => seen.has(c)),
      anyAffects,
      edgeTypes: [...edgeTypes],
    };
  }, [nodes, baseEdges]);

  const isEmpty = !loading && !loadError && nodes.length === 0;

  const selectedNode = selectedNodeId
    ? nodes.find((n) => n.id === selectedNodeId && n.type === 'contentNode')
    : undefined;

  if (loading) return <LoadingState />;
  if (loadError) return <ErrorState onRetry={reload} />;
  if (isEmpty) return <EmptyState />;

  return (
    // Flex column with a DEFINITE height. React Flow's root is `height:100%`,
    // which resolves to 0 against a parent that only sets `min-height` — so the
    // canvas must live in a flex child (`flex-1 min-h-0`) inside a container with
    // a real height, or the whole graph renders invisibly (React Flow error #004).
    <div
      className="flex flex-col"
      style={{ height: 'calc(100vh - 190px)', minHeight: 500, backgroundColor: 'var(--color-bg-primary)' }}
    >
      {/* Relative wrapper so the detail panel can overlay the canvas without
          reflowing it (React Flow needs its definite flex height intact). */}
      <div className="relative flex flex-col" style={{ flex: '1 1 0%', minHeight: 0 }}>
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
        fitView
        nodesDraggable
        nodesConnectable={false}
        elementsSelectable
        style={{ flex: '1 1 0%', minHeight: 0 }}
      >
        <ZoomCssVar />
        {legend.categories.length > 0 && (
          <Panel position="top-left">
            <GraphLegend
              categories={legend.categories}
              anyAffects={legend.anyAffects}
              edgeTypes={legend.edgeTypes}
            />
          </Panel>
        )}

        {/* Stale-corpus pill: a new analysis landed after this map was built.
            Refresh is explicit — silent rebuilds would yank the viewport. */}
        {corpusChanged && (
          <Panel position="top-right">
            <button
              onClick={reload}
              className="px-2.5 py-1 text-[11px] rounded border transition-colors hover:bg-bg-tertiary"
              style={{
                color: 'var(--color-accent-gold, #D4AF37)',
                borderColor: 'var(--color-border)',
                backgroundColor: 'var(--color-bg-secondary)',
                fontFamily: 'Inter, sans-serif',
              }}
            >
              {t('signals.graphCorpusChanged', 'Corpus updated — refresh')}
            </button>
          </Panel>
        )}

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
      {selectedNode && (
        <GraphDetailPanel
          key={selectedNode.id}
          nodeId={Number(selectedNode.id)}
          data={selectedNode.data as ContentNode['data']}
          onClose={closePanel}
        />
      )}
      </div>
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
              {meta.collapsed_items > 0 && (
                <span>{t('signals.graphCollapsedNote', { items: meta.collapsed_items, stories: meta.story_count })}</span>
              )}
              {/* Honest coverage: the map is the top slice of the window, not
                  the window. The old "+2 in List only" line counted just the
                  cap overflow while thousands sat below the load cutoff. */}
              {meta.window_candidates > meta.total_items + meta.collapsed_items && (
                <span>{t('signals.graphCoverageNote', 'top {{shown}} of {{total}} this window', {
                  shown: meta.total_items + meta.collapsed_items,
                  total: meta.window_candidates,
                })}</span>
              )}
              {/* Corpus parity ramp (Phase 95): curated vs unjudged in ITEM
                  units (P2.14 — story collapse can't inflate the ramp). */}
              {meta.curated_items > 0 && meta.curated_items < meta.total_items + meta.collapsed_items && (
                <span>{t('signals.graphCuratedNote', '{{curated}} curated · {{recent}} recent unjudged', {
                  curated: meta.curated_items,
                  recent: meta.total_items + meta.collapsed_items - meta.curated_items,
                })}</span>
              )}
            </>
          )}
        </div>
        {/* The 7/14/30d toggle renders only when the windows would actually
            differ (curated verdicts older than 7d exist) — a control that
            does nothing is a cold-start-doctrine violation. Kept visible if
            the user already switched off the default so they can get back. */}
        {(meta?.windows_differ || days !== 7) && (
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
        )}
      </div>
    </div>
  );
}
