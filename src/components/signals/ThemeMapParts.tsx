// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// The theme map's building blocks: a clickable item row, a treemap tile, the
// "Your stack" column and the unthemed list. Pure presentation — the view
// (ContentGraphView) owns data, layout and selection.
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { GraphNode } from '../../types/graph';
import { CategoryMark } from './graph-marks';
import { TILE_HEADER_H, TILE_LINE_H, TILE_PAD, type Rect } from './theme-map-layout';
import {
  TILE_COLUMN_GAP,
  cleanTitle,
  displayThemeLabel,
  tileColumns,
  visibleLines,
  type Theme,
} from './theme-map-model';

interface RowProps {
  node: GraphNode;
  stack?: boolean;
  isNew: boolean;
  selected: boolean;
  onOpen: (node: GraphNode) => void;
  /** Wrap the full title over several lines instead of truncating (panels). */
  wrap?: boolean;
  /** Muted trailing context, e.g. the theme a stack item relates to. */
  context?: string;
}

/** One item: category mark + title. New since the last visit reads brighter. */
export function ItemRow({ node, stack = false, isNew, selected, onOpen, wrap = false, context }: RowProps) {
  const extra = node.member_count - 1;
  return (
    <button
      type="button"
      onClick={() => onOpen(node)}
      title={context ? `${node.title} — ${displayThemeLabel(context)}` : node.title}
      aria-pressed={selected}
      className={`flex ${wrap ? 'items-start py-1' : 'items-center'} gap-2 w-full min-w-0 text-start rounded px-1 -mx-1 transition-colors hover:bg-bg-tertiary focus-visible:outline focus-visible:outline-1`}
      style={{
        height: wrap ? undefined : TILE_LINE_H,
        backgroundColor: selected ? 'var(--color-bg-tertiary)' : undefined,
      }}
    >
      <span className="inline-flex shrink-0" style={{ marginTop: wrap ? 4 : 0 }}>
        <CategoryMark category={node.category} stack={stack} />
      </span>
      <span
        className={wrap ? 'text-[12px] leading-snug' : 'truncate text-[12px]'}
        style={{
          color: isNew || selected ? 'var(--color-text-primary)' : 'var(--color-text-secondary)',
          fontWeight: isNew ? 500 : 400,
        }}
      >
        {cleanTitle(node.title)}
        {context && (
          <span className="ms-1.5 text-[10px]" style={{ color: 'var(--color-text-muted)' }}>
            {displayThemeLabel(context)}
          </span>
        )}
      </span>
      {extra > 0 && (
        <span
          className="shrink-0 text-[10px] px-1 rounded border"
          style={{
            color: 'var(--color-text-muted)',
            borderColor: 'var(--color-border)',
            fontFamily: 'JetBrains Mono, monospace',
          }}
        >
          {`+${extra}`}
        </span>
      )}
    </button>
  );
}

interface TileProps {
  theme: Theme;
  rect: Rect;
  gap: number;
  isNew: (node: GraphNode) => boolean;
  selectedId: number | null;
  themeSelected: boolean;
  onOpenItem: (node: GraphNode) => void;
  onOpenTheme: (theme: Theme) => void;
}

/** A theme tile: name + count, then as many of its most important titles as
 *  the tile's height allows; the remainder is one "+N more" line that opens
 *  the whole theme in the side panel. A theme whose community held stack
 *  items spends one line naming them, in gold — the link between this news
 *  and the user's packages. */
export function ThemeTile({ theme, rect, gap, isNew, selectedId, themeSelected, onOpenItem, onOpenTheme }: TileProps) {
  const { t } = useTranslation();
  const h = rect.h - gap;
  const cols = tileColumns(rect.w - gap);
  const stackLine = theme.stack.length > 0 ? 1 : 0;
  const { shown, more } = visibleLines(h, theme.items.length, cols, stackLine);
  // A tile too small for a "+N more" line still says what it hides.
  const hiddenQuietly = more === 0 ? theme.items.length - shown : 0;
  const stackNames = theme.stack.map((n) => cleanTitle(n.title)).join(', ');
  const hasSecurity = theme.items.some((n) => n.category === 'security');
  return (
    <section
      aria-label={displayThemeLabel(theme.label)}
      className="absolute flex flex-col overflow-hidden rounded-lg border"
      style={{
        left: rect.x + gap / 2,
        top: rect.y + gap / 2,
        width: Math.max(0, rect.w - gap),
        height: Math.max(0, h),
        padding: TILE_PAD,
        backgroundColor: 'var(--color-bg-secondary)',
        borderColor: themeSelected ? 'var(--color-text-secondary)' : 'var(--color-border)',
        boxShadow: hasSecurity ? 'inset 3px 0 0 var(--color-error)' : undefined,
      }}
    >
      <button
        type="button"
        onClick={() => onOpenTheme(theme)}
        title={displayThemeLabel(theme.label)}
        className="flex items-baseline gap-1.5 min-w-0 text-start shrink-0 hover:underline"
        style={{ height: TILE_HEADER_H }}
      >
        <span className="truncate text-[13px] font-semibold" style={{ color: 'var(--color-text-primary)' }}>
          {displayThemeLabel(theme.label)}
        </span>
        <span className="shrink-0 text-[11px]" style={{ color: 'var(--color-text-muted)' }}>
          {theme.items.length}
        </span>
        {hiddenQuietly > 0 && (
          <span className="shrink-0 ms-auto text-[10px]" style={{ color: 'var(--color-text-muted)' }}>
            {t('signals.graphStoryMore', { count: hiddenQuietly })}
          </span>
        )}
      </button>
      {stackLine > 0 && (
        <button
          type="button"
          onClick={() => onOpenTheme(theme)}
          title={stackNames}
          className="flex items-center gap-1.5 min-w-0 text-start shrink-0 text-[11px]"
          style={{ height: TILE_LINE_H, color: 'var(--color-accent-gold)' }}
        >
          <span className="shrink-0 font-medium">{t('signals.laneStack')}:</span>
          <span className="truncate">{stackNames}</span>
        </button>
      )}
      <div
        className="grid min-w-0"
        style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`, columnGap: TILE_COLUMN_GAP }}
      >
        {theme.items.slice(0, shown).map((n) => (
          <ItemRow key={n.id} node={n} isNew={isNew(n)} selected={selectedId === n.id} onOpen={onOpenItem} />
        ))}
        {more > 0 && (
          <button
            type="button"
            onClick={() => onOpenTheme(theme)}
            className="text-start text-[11px] transition-colors hover:text-text-primary"
            style={{ height: TILE_LINE_H, color: 'var(--color-text-muted)' }}
          >
            {t('signals.graphStoryMore', { count: more })}
          </button>
        )}
      </div>
    </section>
  );
}

interface StackProps {
  items: GraphNode[];
  /** Stack item id → the theme its community belongs to, shown muted. */
  themeOf: Map<number, string>;
  isNew: (node: GraphNode) => boolean;
  selectedId: number | null;
  onOpenItem: (node: GraphNode) => void;
}

/** "Your stack": every item tied to the user's own dependencies, in one
 *  place, most urgent first — never scattered across themes or left in the
 *  unthemed pile (4 of 16 were, live 2026-10-05). */
export function StackColumn({ items, themeOf, isNew, selectedId, onOpenItem }: StackProps) {
  const { t } = useTranslation();
  return (
    <aside
      aria-label={t('signals.laneStack')}
      className="flex flex-col shrink-0 rounded-lg border overflow-hidden"
      style={{
        width: 280,
        backgroundColor: 'var(--color-bg-secondary)',
        borderColor: 'color-mix(in srgb, var(--color-accent-gold) 35%, var(--color-border))',
      }}
    >
      <div className="flex items-baseline gap-1.5 px-3 pt-2.5 pb-1.5 shrink-0">
        <span className="text-[13px] font-semibold" style={{ color: 'var(--color-accent-gold)' }}>
          {t('signals.laneStack')}
        </span>
        <span className="text-[11px]" style={{ color: 'var(--color-text-muted)' }}>
          {items.length}
        </span>
      </div>
      <div className="flex flex-col px-3 pb-2 overflow-y-auto min-h-0">
        {items.map((n) => (
          <ItemRow
            key={n.id}
            node={n}
            stack
            isNew={isNew(n)}
            selected={selectedId === n.id}
            onOpen={onOpenItem}
            context={themeOf.get(n.id)}
          />
        ))}
      </div>
    </aside>
  );
}

interface UnthemedProps {
  items: GraphNode[];
  /** Open on first render — when the window has no themes at all. */
  defaultOpen?: boolean;
  isNew: (node: GraphNode) => boolean;
  selectedId: number | null;
  onOpenItem: (node: GraphNode) => void;
}

/** Items related to no theme this window, said plainly. Collapsed by default:
 *  a pile of unrelated titles is the least informative part of the map. */
export function UnthemedList({ items, defaultOpen = false, isNew, selectedId, onOpenItem }: UnthemedProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="shrink-0 rounded-lg border" style={{ borderColor: 'var(--color-border)', backgroundColor: 'var(--color-bg-secondary)' }}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="flex items-center justify-between w-full px-3 py-2 text-start"
      >
        <span className="text-[12px] font-medium" style={{ color: 'var(--color-text-secondary)' }}>
          {t('signals.graphLaneLabel', { count: items.length })}
        </span>
        <span className="text-[11px]" style={{ color: 'var(--color-text-muted)' }}>
          {open ? t('signals.laneShowFewer') : t('signals.laneShowAll', { count: items.length })}
        </span>
      </button>
      {open && (
        <div
          className="px-3 pb-2 overflow-y-auto"
          style={{ maxHeight: 220, columnWidth: 300, columnGap: 24 }}
        >
          {items.map((n) => (
            <div key={n.id} style={{ breakInside: 'avoid' }}>
              <ItemRow node={n} isNew={isNew(n)} selected={selectedId === n.id} onOpen={onOpenItem} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
