// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Signal → Themes: the reading view. "Your stack" in its own column, this
// window's themes as an order-preserving treemap that always fills the view
// with readable titles, and the unthemed items in a collapsed list. (Signal →
// Graph is the relationship view of the same payload.) The force canvas this
// replaced as the default needed zoom 0.275 to fit and hid 136 of 150 titles.
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { GraphNode } from '../../types/graph';
import GraphDetailPanel from './GraphDetailPanel';
import ThemeDetailPanel from './ThemeDetailPanel';
import { LoadingState, EmptyState, ErrorState, GraphLegend } from './ContentGraphChrome';
import ContentGraphFooter from './ContentGraphFooter';
import { StackColumn, ThemeTile, UnthemedList } from './ThemeMapParts';
import { useContentGraph } from './use-content-graph';
import { markViewed, readLastViewed, readMemory, writeMemory } from './graph-visit';
import { GRAPH_CATEGORIES } from './graph-marks';
import { buildThemeMap, type Theme } from './theme-map-model';
import { TILE_GAP, stableThemeLayout, themeWeights, type RememberedLayout } from './theme-map-layout';

/** Where each window's last layout is remembered (per viewer, per device —
 *  a convenience: without it the map simply lays out fresh). */
const layoutKey = (days: number) => `4da:graph:layout:${days}d`;
const isRememberedLayout = (v: unknown): v is RememberedLayout =>
  !!v && Array.isArray((v as RememberedLayout).rows) && typeof (v as RememberedLayout).members === 'object';

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

export default function ThemeMapView() {
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
  // The layout remembered from the previous build, read once per loaded map.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const previousLayout = useMemo(() => readMemory(layoutKey(days), isRememberedLayout), [days, graph]);

  // Tiles stay where the user last saw them while that still fits
  // (stableThemeLayout); a fresh layout otherwise.
  const layout = useMemo(() => {
    if (!map || treemapSize.w === 0 || treemapSize.h === 0) return null;
    const weights = themeWeights(
      map.themes.map((t) => t.items.length),
      treemapSize.w * treemapSize.h,
    );
    const themes = map.themes.map((t, i) => ({
      id: t.id,
      members: t.items.flatMap((n) => n.member_ids),
      weight: weights[i]!,
    }));
    return stableThemeLayout(themes, { x: 0, y: 0, w: treemapSize.w, h: treemapSize.h }, previousLayout);
  }, [map, treemapSize.w, treemapSize.h, previousLayout]);
  const rects = layout?.rects ?? [];

  useEffect(() => {
    if (layout) writeMemory(layoutKey(days), layout.remembered);
  }, [layout, days]);

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
      // 210px = the app chrome above this view (measured 204px at
      // 1700×1184): 190px pushed the footer 14px below the window.
      style={{ height: 'calc(100vh - 210px)', minHeight: 500, backgroundColor: 'var(--color-bg-primary)' }}
    >
      <div className="relative flex gap-3 px-4 pt-2 pb-3" style={{ flex: '1 1 0%', minHeight: 0 }}>
        {map.stack.length > 0 && (
          <StackColumn
            items={map.stack}
            themeOf={map.stackTheme}
            isNew={isNew}
            selectedId={selectedId}
            onOpenItem={openItem}
          />
        )}
        <div className="flex flex-col gap-2 min-w-0" style={{ flex: '1 1 0%', minHeight: 0 }}>
          <div className="flex items-center justify-between gap-3 shrink-0" style={{ minHeight: 24 }}>
            <GraphLegend categories={categories} hasStack={map.stack.length > 0} />
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
