// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// One virtualized listbox of result rows. The Signal list renders one of these
// per lane, all sharing the outer scroll element: each list offsets its
// virtualizer by its own position inside that element (scrollMargin), so a
// lane of 300 rows still mounts only the rows on screen.
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { ResultItem } from './ResultItem';
import type { SourceRelevance, FeedbackAction, FeedbackGiven } from '../types';

export interface ResultLaneListProps {
  id?: string;
  items: SourceRelevance[];
  /** Global index of items[0] in the visible order (keyboard focus space). */
  indexOffset: number;
  scrollElementRef: RefObject<HTMLDivElement | null>;
  labelledBy?: string;
  ariaLabel?: string;
  focusedIndex: number;
  expandedItem: number | null;
  feedbackGiven: FeedbackGiven;
  onToggleExpand: (itemId: number) => void;
  onRecordInteraction: (itemId: number, actionType: FeedbackAction, item: SourceRelevance) => void;
  comparePool: SourceRelevance[];
  /**
   * Deep-link target: when this list holds the item, scroll it to centre. A
   * fresh object per request, so re-targeting the same item scrolls again and
   * an unrelated re-render never does.
   */
  scrollTarget?: { id: number } | null;
  /** Optional per-row prefix (flat-mode group and topic headers). */
  renderPrefix?: (item: SourceRelevance, localIndex: number) => ReactNode;
}

/** Offset of `el` from the top of the scroll element's content. */
function offsetWithin(el: HTMLElement, scrollEl: HTMLElement): number {
  return el.getBoundingClientRect().top - scrollEl.getBoundingClientRect().top + scrollEl.scrollTop;
}

export function ResultLaneList({
  id, items, indexOffset, scrollElementRef, labelledBy, ariaLabel, focusedIndex,
  expandedItem, feedbackGiven, onToggleExpand, onRecordInteraction, comparePool,
  scrollTarget, renderPrefix,
}: ResultLaneListProps) {
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollMargin, setScrollMargin] = useState(0);

  // Re-measure our offset whenever content above us changes size (a lane
  // expanding, rows being measured). Guarded so it never loops.
  useLayoutEffect(() => {
    const scrollEl = scrollElementRef.current;
    const el = listRef.current;
    if (!scrollEl || !el) return;
    const measure = () => {
      const next = Math.round(offsetWithin(el, scrollEl));
      setScrollMargin((prev) => (prev === next ? prev : next));
    };
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(measure);
    const content = scrollEl.firstElementChild;
    if (content) ro.observe(content);
    return () => ro.disconnect();
  }, [scrollElementRef]);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollElementRef.current,
    estimateSize: () => 120,
    overscan: 5,
    scrollMargin,
  });

  const handledTarget = useRef<{ id: number } | null>(null);
  useEffect(() => {
    if (!scrollTarget || handledTarget.current === scrollTarget) return;
    const idx = items.findIndex((r) => r.id === scrollTarget.id);
    if (idx < 0) return;
    handledTarget.current = scrollTarget;
    requestAnimationFrame(() => virtualizer.scrollToIndex(idx, { align: 'center' }));
  }, [scrollTarget, items, virtualizer]);

  const focusedLocal = focusedIndex - indexOffset;
  const activeItem = focusedLocal >= 0 ? items[focusedLocal] : undefined;

  return (
    <div
      ref={listRef}
      id={id}
      role="listbox"
      aria-labelledby={labelledBy}
      aria-label={labelledBy ? undefined : ariaLabel}
      aria-activedescendant={activeItem ? `result-item-${activeItem.id}` : undefined}
      tabIndex={-1}
      style={{ height: `${virtualizer.getTotalSize()}px`, width: '100%', position: 'relative' }}
    >
      {virtualizer.getVirtualItems().map((virtualRow) => {
        const item = items[virtualRow.index]!;
        const globalIdx = indexOffset + virtualRow.index;
        return (
          <div
            key={item.id}
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              width: '100%',
              transform: `translateY(${virtualRow.start - scrollMargin}px)`,
            }}
            ref={virtualizer.measureElement}
            data-index={virtualRow.index}
          >
            {renderPrefix?.(item, virtualRow.index)}
            <div className="pb-3">
              <ResultItem
                item={item}
                isExpanded={expandedItem === item.id}
                isFocused={focusedIndex === globalIdx}
                onToggleExpand={onToggleExpand}
                feedbackGiven={feedbackGiven}
                onRecordInteraction={onRecordInteraction}
                comparePool={expandedItem === item.id ? comparePool : undefined}
                itemIndex={globalIdx}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
