// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Data flow for the content graph: build, retry, and an HONEST staleness
// signal. Extracted from ContentGraphView so the view owns rendering only.
import { useCallback, useEffect, useRef, useState } from 'react';

import { cmd } from '../../lib/commands';
import { useAppStore } from '../../store';
import type { ContentGraph } from '../../types/graph';
import type { SourceRelevance } from '../../types/analysis';
import { isSurfacedSignal } from '../../utils/score';

/** Quiet period after the last results change before probing a rebuild —
 *  a background merge writes the results array several times in a burst. */
const PROBE_DEBOUNCE_MS = 3000;

/** Identity of what the map SHOWS: every node and every member it stands for.
 *  Positions, labels and edges follow from this set; two builds with the same
 *  set are the same map for the user. */
export function graphNodeSetKey(graph: ContentGraph): string {
  return graph.nodes
    .map((n) => `${n.id}:${[...n.member_ids].sort((a, b) => a - b).join(',')}`)
    .sort()
    .join('|');
}

/** Cheap pre-filter over the analysis results: the surfaced-item set. When it
 *  is unchanged a background merge cannot have changed the graph's corpus, so
 *  no probe build runs at all. */
export function surfacedSignature(results: SourceRelevance[]): string {
  return results
    .filter(isSurfacedSignal)
    .map((r) => r.id)
    .sort((a, b) => a - b)
    .join(',');
}

export interface ContentGraphData {
  graph: ContentGraph | null;
  loading: boolean;
  loadError: boolean;
  /** A rebuild probe produced a DIFFERENT node set than the one on screen. */
  stale: boolean;
  /** Retry after an error (full rebuild with spinner). */
  reload: () => void;
  /** Show the already-built newer graph (no second build, no spinner). */
  applyFresh: () => void;
}

/**
 * The "Corpus updated — refresh" pill used to compare the results ARRAY by
 * identity: every background merge (~10 min) replaces the array, so the pill
 * fired even when nothing the map shows had changed (audit 2026-10-02). Now:
 * a results change whose surfaced-id set differs from the one the map was
 * built against triggers a quiet probe build; the pill appears only if the
 * probe's node set differs from the map on screen, and clicking it swaps in
 * the probe result directly.
 */
export function useContentGraph(days: number): ContentGraphData {
  const [graph, setGraph] = useState<ContentGraph | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [reloadToken, setReloadToken] = useState(0);
  const [fresh, setFresh] = useState<ContentGraph | null>(null);
  const relevanceResults = useAppStore((s) => s.appState.relevanceResults);
  const builtSigRef = useRef<string | null>(null);
  const shownKeyRef = useRef<string | null>(null);

  const reload = useCallback(() => setReloadToken((n) => n + 1), []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setLoadError(false);
    setFresh(null);
    cmd('build_content_graph', { days, maxNodes: 150 })
      .then((g: ContentGraph) => {
        if (cancelled) return;
        builtSigRef.current = surfacedSignature(useAppStore.getState().appState.relevanceResults);
        shownKeyRef.current = graphNodeSetKey(g);
        setGraph(g);
      })
      .catch((err) => {
        if (cancelled) return;
        console.error('[ContentGraph] Failed to load:', err);
        setLoadError(true);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [days, reloadToken]);

  useEffect(() => {
    if (loading || builtSigRef.current === null) return;
    const sig = surfacedSignature(relevanceResults);
    if (sig === builtSigRef.current) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      cmd('build_content_graph', { days, maxNodes: 150 })
        .then((g: ContentGraph) => {
          if (cancelled) return;
          builtSigRef.current = sig;
          setFresh(graphNodeSetKey(g) === shownKeyRef.current ? null : g);
        })
        .catch((err) => console.warn('[ContentGraph] Staleness probe failed:', err));
    }, PROBE_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [relevanceResults, days, loading]);

  const applyFresh = useCallback(() => {
    if (!fresh) return;
    shownKeyRef.current = graphNodeSetKey(fresh);
    setGraph(fresh);
    setFresh(null);
  }, [fresh]);

  return { graph, loading, loadError, stale: fresh !== null, reload, applyFresh };
}
