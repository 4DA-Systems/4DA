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
/** Minimum time between probe builds (and after the map's own load). A
 *  probe is a full graph build (~4 s of CPU); while an analysis re-scores,
 *  the surfaced set changes every few seconds, and the 3 s debounce alone
 *  let the app build the graph ~30 times in 3 minutes (live log,
 *  2026-10-06). The pill only informs, so a minute of lag costs nothing. */
const PROBE_MIN_INTERVAL_MS = 60_000;

/** Delay before the next probe may start: the debounce, or the rest of the
 *  minimum interval since the last build started, whichever is longer. */
export function probeDelay(now: number, lastBuildAt: number, debounceMs = PROBE_DEBOUNCE_MS, minIntervalMs = PROBE_MIN_INTERVAL_MS): number {
  return Math.max(debounceMs, lastBuildAt + minIntervalMs - now);
}

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

/** The last map built per window, shared by the Themes and Graph views:
 *  switching between them shows the map already built instead of paying
 *  another ~4 s build (the probe below still catches real changes). Module
 *  scope = this app session; a restart builds fresh. */
interface CachedGraph {
  graph: ContentGraph;
  /** Surfaced-set signature the graph was built against. */
  sig: string;
  builtAt: number;
}
const graphCache = new Map<number, CachedGraph>();

/** Test hook: forget every cached map. */
export function clearGraphCache() {
  graphCache.clear();
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
  const [graph, setGraph] = useState<ContentGraph | null>(() => graphCache.get(days)?.graph ?? null);
  const [loading, setLoading] = useState(() => !graphCache.has(days));
  const [loadError, setLoadError] = useState(false);
  const [reloadToken, setReloadToken] = useState(0);
  const [fresh, setFresh] = useState<ContentGraph | null>(null);
  const relevanceResults = useAppStore((s) => s.appState.relevanceResults);
  const builtSigRef = useRef<string | null>(null);
  const shownKeyRef = useRef<string | null>(null);
  const lastBuildAtRef = useRef(0);
  const probeInFlightRef = useRef(false);
  const daysRef = useRef(days);
  daysRef.current = days;
  // Bumped when a probe lands, so a change that arrived mid-flight is
  // re-evaluated instead of starting a second, overlapping build.
  const [probeTick, setProbeTick] = useState(0);

  const reload = useCallback(() => setReloadToken((n) => n + 1), []);
  // Reload tokens already honoured: a cache hit serves only a mount or a
  // window change, never an explicit retry.
  const servedReloadRef = useRef(0);

  useEffect(() => {
    const hit = graphCache.get(days);
    if (hit && reloadToken === servedReloadRef.current) {
      builtSigRef.current = hit.sig;
      shownKeyRef.current = graphNodeSetKey(hit.graph);
      lastBuildAtRef.current = hit.builtAt;
      setGraph(hit.graph);
      setFresh(null);
      setLoadError(false);
      setLoading(false);
      return;
    }
    servedReloadRef.current = reloadToken;
    let cancelled = false;
    setLoading(true);
    setLoadError(false);
    setFresh(null);
    lastBuildAtRef.current = Date.now();
    cmd('build_content_graph', { days, maxNodes: 150 })
      .then((g: ContentGraph) => {
        if (cancelled) return;
        builtSigRef.current = surfacedSignature(useAppStore.getState().appState.relevanceResults);
        shownKeyRef.current = graphNodeSetKey(g);
        graphCache.set(days, { graph: g, sig: builtSigRef.current, builtAt: lastBuildAtRef.current });
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

  // One probe at a time, at most one a minute, and a finished probe always
  // counts. The old effect discarded a probe whose results changed while it
  // ran — the backend build cannot be cancelled, so the CPU was spent anyway
  // and the next change started another one.
  useEffect(() => {
    if (loading || builtSigRef.current === null) return;
    if (surfacedSignature(relevanceResults) === builtSigRef.current) return;
    if (probeInFlightRef.current) return;
    const timer = setTimeout(() => {
      const probeDays = daysRef.current;
      // Probe what is surfaced NOW, not what was surfaced when scheduled.
      const sig = surfacedSignature(useAppStore.getState().appState.relevanceResults);
      probeInFlightRef.current = true;
      lastBuildAtRef.current = Date.now();
      cmd('build_content_graph', { days: probeDays, maxNodes: 150 })
        .then((g: ContentGraph) => {
          if (probeDays !== daysRef.current) return;
          builtSigRef.current = sig;
          const same = graphNodeSetKey(g) === shownKeyRef.current;
          // Same map: the cache now vouches for the newer signature too.
          const cached = graphCache.get(probeDays);
          if (same && cached) graphCache.set(probeDays, { ...cached, sig, builtAt: lastBuildAtRef.current });
          setFresh(same ? null : g);
        })
        .catch((err) => console.warn('[ContentGraph] Staleness probe failed:', err))
        .finally(() => {
          probeInFlightRef.current = false;
          setProbeTick((n) => n + 1);
        });
    }, probeDelay(Date.now(), lastBuildAtRef.current));
    return () => clearTimeout(timer);
  }, [relevanceResults, days, loading, probeTick]);

  const applyFresh = useCallback(() => {
    if (!fresh) return;
    shownKeyRef.current = graphNodeSetKey(fresh);
    graphCache.set(daysRef.current, { graph: fresh, sig: builtSigRef.current ?? '', builtAt: lastBuildAtRef.current });
    setGraph(fresh);
    setFresh(null);
  }, [fresh]);

  return { graph, loading, loadError, stale: fresh !== null, reload, applyFresh };
}
