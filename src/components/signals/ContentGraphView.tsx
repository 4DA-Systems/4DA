// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Signal → Graph: the theme map. "Your stack" in its own column, this
// window's themes as an order-preserving treemap that always fills the view
// with readable titles, and the unthemed items in a collapsed list. Replaces
// the force canvas that needed zoom 0.275 to fit and hid 136 of 150 titles
// (theme-map-model.ts has the measurements).
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { GraphNode } from '../../types/graph';
import GraphDetailPanel from './GraphDetailPanel';
import ThemeDetailPanel from './ThemeDetailPanel';
import { LoadingState, EmptyState, ErrorState, GraphLegend } from './ContentGraphChrome';
import ContentGraphFooter from './ContentGraphFooter';
import { StackColumn, ThemeTile, UnthemedList } from './ThemeMapParts';
import { useContentGraph } from './use-content-graph';
import { GRAPH_CATEGORIES } from './graph-marks';
import { TILE_GAP, buildThemeMap, stripTreemap, themeWeights, type Theme } from './theme-map-model';

const LAST_VIEW_KEY = '4da:graph:lastViewedAt';

function readLastViewed(): number {
  try {
    const raw = localStorage.getItem(LAST_VIEW_KEY);
    return raw ? new Date(raw).getTime() || 0 : 0;
  } catch {
    return 0;
  }
}

function markViewed() {
  try {
    localStorage.setItem(LAST_VIEW_KEY, new Date().toISOString());
  } catch {
    // Private window / blocked storage: "new" highlighting is a convenience.
  }
}

/** Live client size of an element, via a callback ref — the treemap host
 *  only mounts once the graph has loaded, so a mount-time effect would miss it. */
function useElementSize<T extends HTMLElement>() {
  const [el, setEl] = useState<T | null>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  useEffect(() => {
    if (!el) return;
    const update = () =>
      setSize((s) => (s.w === el.clientWidth && s.h === el.clientHeight ? s : { w: el.clientWidth, h: el.clientHeight }));
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [el]);
  return [setEl, size] as const;
}

type Selection = { kind: 'item'; node: GraphNode } | { kind: 'theme'; theme: Theme } | null;

export default function ContentGraphView() {
  const { t } = useTranslation();
  const [days, setDays] = useState(7);
  const { graph, loading, loadError, stale, reload, applyFresh } = useContentGraph(days);
  const [selection, setSelection] = useState<Selection>(null);
  const [lastViewedMs, setLastViewedMs] = useState(0);
  const [treemapRef, treemapSize] = useElementSize<HTMLDivElement>();

  // "New" = arrived since the previous visit: read the old stamp, then move
  // it to now, once per loaded map.
  useEffect(() => {
    if (!graph) return;
    setSelection(null);
    setLastViewedMs(readLastViewed());
    markViewed();
  }, [graph]);

  const map = useMemo(() => (graph ? buildThemeMap(graph) : null), [graph]);

  const rects = useMemo(() => {
    if (!map || treemapSize.w === 0 || treemapSize.h === 0) return [];
    const weights = themeWeights(
      map.themes.map((t) => t.items.length),
      treemapSize.w * treemapSize.h,
    );
    return stripTreemap(weights, { x: 0, y: 0, w: treemapSize.w, h: treemapSize.h });
  }, [map, treemapSize.w, treemapSize.h]);

  const categories = useMemo(() => {
    const seen = new Set(graph?.nodes.map((n) => n.category) ?? []);
    return GRAPH_CATEGORIES.filter((c) => seen.has(c));
  }, [graph]);

  const isNew = useCallback(
    (n: GraphNode) => (n.created_at ? new Date(n.created_at).getTime() > lastViewedMs : false),
    [lastViewedMs],
  );
  const openItem = useCallback((node: GraphNode) => setSelection({ kind: 'item', node }), []);
  const openTheme = useCallback((theme: Theme) => setSelection({ kind: 'theme', theme }), []);
  const closePanel = useCallback(() => setSelection(null), []);

  if (loading) return <LoadingState />;
  if (loadError) return <ErrorState onRetry={reload} />;
  if (!graph || !map || graph.nodes.length === 0) return <EmptyState />;

  const selectedId = selection?.kind === 'item' ? selection.node.id : null;
  const selectedThemeId = selection?.kind === 'theme' ? selection.theme.id : null;

  return (
    <div
      className="flex flex-col"
      style={{ height: 'calc(100vh - 190px)', minHeight: 500, backgroundColor: 'var(--color-bg-primary)' }}
    >
      <div className="relative flex gap-3 px-4 pt-2 pb-3" style={{ flex: '1 1 0%', minHeight: 0 }}>
        {map.stack.length > 0 && (
          <StackColumn items={map.stack} isNew={isNew} selectedId={selectedId} onOpenItem={openItem} />
        )}
        <div className="flex flex-col gap-2 min-w-0" style={{ flex: '1 1 0%', minHeight: 0 }}>
          <div className="flex items-center justify-between gap-3 shrink-0" style={{ minHeight: 24 }}>
            <GraphLegend categories={categories} />
            {/* Stale-map pill: a quiet probe rebuild found a DIFFERENT item
                set (use-content-graph). Swapping is explicit — a silent swap
                would reflow the map mid-read. */}
            {stale && (
              <button
                onClick={applyFresh}
                className="px-2.5 py-1 text-[11px] rounded border transition-colors hover:bg-bg-tertiary shrink-0"
                style={{
                  color: 'var(--color-accent-gold)',
                  borderColor: 'var(--color-border)',
                  backgroundColor: 'var(--color-bg-secondary)',
                }}
              >
                {t('signals.graphCorpusChanged')}
              </button>
            )}
          </div>
          <div ref={treemapRef} className="relative" style={{ flex: '1 1 0%', minHeight: 0, margin: -TILE_GAP / 2 }}>
            {map.themes.map((theme, i) =>
              rects[i] ? (
                <ThemeTile
                  key={theme.id}
                  theme={theme}
                  rect={rects[i]}
                  gap={TILE_GAP}
                  isNew={isNew}
                  selectedId={selectedId}
                  themeSelected={selectedThemeId === theme.id}
                  onOpenItem={openItem}
                  onOpenTheme={openTheme}
                />
              ) : null,
            )}
          </div>
          {map.unthemed.length > 0 && (
            <UnthemedList
              key={graph.meta.time_window_days}
              items={map.unthemed}
              defaultOpen={map.themes.length === 0}
              isNew={isNew}
              selectedId={selectedId}
              onOpenItem={openItem}
            />
          )}
        </div>
        {selection?.kind === 'theme' && (
          <ThemeDetailPanel theme={selection.theme} isNew={isNew} onOpenItem={openItem} onClose={closePanel} />
        )}
        {selection?.kind === 'item' && (
          <GraphDetailPanel key={selection.node.id} node={selection.node} onClose={closePanel} />
        )}
      </div>
      <ContentGraphFooter meta={graph.meta} days={days} onDaysChange={setDays} />
    </div>
  );
}
