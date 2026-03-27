/* global IntersectionObserver */
import { useRef, useEffect, useCallback } from 'react';
import { cmd } from '../lib/commands';

interface UseViewTrackingOptions {
  itemId: number;
  sourceType: string;
  /** Minimum seconds visible before recording scroll (default: 2) */
  threshold?: number;
  /** Whether tracking is enabled (default: true) */
  enabled?: boolean;
  /** Whether the user has given explicit feedback (save/dismiss/click) */
  hasExplicitFeedback?: boolean;
  /** Content topics for behavior learning */
  itemTopics?: string[];
  /** Zero-based index of this item in the result list (for scroll depth calculation) */
  itemIndex?: number;
  /** Total number of items in the result list (for scroll depth calculation) */
  totalItems?: number;
}

/** Minimum seconds visible to emit an 'ignore' signal for seen-but-skipped */
const IGNORE_THRESHOLD_SECONDS = 5;

/**
 * Track view-time for a content item using IntersectionObserver.
 * When the element is visible for `threshold` seconds, emits a passive
 * 'scroll' interaction to the backend for behavior learning.
 *
 * When visible for 5+ seconds with no explicit feedback, also emits an
 * 'ignore' signal — the user saw it and chose not to interact.
 */
export function useViewTracking({
  itemId,
  sourceType,
  threshold = 2,
  enabled = true,
  hasExplicitFeedback = false,
  itemTopics = [],
  itemIndex,
  totalItems,
}: UseViewTrackingOptions) {
  const containerRef = useRef<HTMLDivElement>(null);
  const visibleSince = useRef<number | null>(null);
  const scrollRecorded = useRef(false);
  const ignoreRecorded = useRef(false);

  const recordView = useCallback(
    (visibleSeconds: number) => {
      if (scrollRecorded.current) return;
      scrollRecorded.current = true;

      // Approximate scroll depth: how far down the list this item sits (0.0 = top, 1.0 = bottom)
      const scrollDepthPct =
        itemIndex != null && totalItems != null && totalItems > 0
          ? Math.min((itemIndex + 1) / totalItems, 1.0)
          : undefined;

      const actionData: Record<string, number> = { visible_seconds: visibleSeconds };
      if (scrollDepthPct != null) {
        actionData.scroll_depth_pct = Math.round(scrollDepthPct * 100) / 100;
      }

      cmd('ace_record_interaction', {
        item_id: itemId,
        action_type: 'scroll',
        action_data: JSON.stringify(actionData),
        item_topics: itemTopics,
        item_source: sourceType,
      }).catch((e) => console.debug('[view-tracking] record view:', e));
    },
    [itemId, sourceType, itemTopics, itemIndex, totalItems],
  );

  const recordIgnore = useCallback(() => {
    if (ignoreRecorded.current || hasExplicitFeedback) return;
    ignoreRecorded.current = true;

    cmd('ace_record_interaction', {
      item_id: itemId,
      action_type: 'ignore',
      action_data: null,
      item_topics: itemTopics,
      item_source: sourceType,
    }).catch((e) => console.debug('[view-tracking] record ignore:', e));
  }, [itemId, sourceType, hasExplicitFeedback, itemTopics]);

  useEffect(() => {
    if (!enabled || !containerRef.current || typeof IntersectionObserver === 'undefined') return;

    const el = containerRef.current;
    scrollRecorded.current = false;
    ignoreRecorded.current = false;
    visibleSince.current = null;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry!.isIntersecting) {
          if (!visibleSince.current) {
            visibleSince.current = Date.now();
          }
        } else if (visibleSince.current) {
          const elapsed = (Date.now() - visibleSince.current) / 1000;
          if (elapsed >= threshold) {
            recordView(elapsed);
          }
          // Emit ignore if visible 5+ seconds with no feedback
          if (elapsed >= IGNORE_THRESHOLD_SECONDS) {
            recordIgnore();
          }
          visibleSince.current = null;
        }
      },
      { threshold: 0.5 },
    );

    observer.observe(el);

    // Also check on unmount
    return () => {
      observer.disconnect();
      if (visibleSince.current) {
        const elapsed = (Date.now() - visibleSince.current) / 1000;
        if (elapsed >= threshold) {
          recordView(elapsed);
        }
        if (elapsed >= IGNORE_THRESHOLD_SECONDS) {
          recordIgnore();
        }
      }
    };
  }, [enabled, threshold, recordView, recordIgnore]);

  return containerRef;
}
