import { useRef, useEffect } from 'react';
import { cmd } from '../lib/commands';

/**
 * Emit both click (dwell) and engagement_complete signals for an expand session.
 * Called on collapse and unmount to capture the full engagement depth.
 */
function emitExpandSignals(
  itemId: number,
  sourceType: string,
  itemTopics: string[],
  dwellSeconds: number,
  label: string,
) {
  // Existing click + dwell signal
  cmd('ace_record_interaction', {
    item_id: itemId,
    action_type: 'click',
    action_data: JSON.stringify({ type: 'click', dwell_time_seconds: dwellSeconds }),
    item_topics: itemTopics,
    item_source: sourceType,
  }).catch((e) => console.debug(`[expand-tracking] ${label} click:`, e));

  // Layer 2: engagement_complete — deep engagement depth signal
  // Expanded items show full content, so scroll_depth_pct = 1.0
  cmd('ace_record_interaction', {
    item_id: itemId,
    action_type: 'engagement_complete',
    action_data: JSON.stringify({
      total_seconds: dwellSeconds,
      scroll_depth_pct: 1.0,
    }),
    item_topics: itemTopics,
    item_source: sourceType,
  }).catch((e) => console.debug(`[expand-tracking] ${label} engagement:`, e));
}

/**
 * Track dwell time when a content item is expanded.
 * On collapse or unmount, emits a 'click' interaction with dwell_time_seconds
 * AND an 'engagement_complete' signal for Layer 2 behavioral microlearning.
 * This captures the strongest implicit signal: the user chose to read deeper.
 */
export function useExpandTracking(
  itemId: number,
  sourceType: string,
  isExpanded: boolean,
  itemTopics: string[],
) {
  const expandedAt = useRef<number | null>(null);
  const emitted = useRef(false);

  useEffect(() => {
    if (isExpanded) {
      expandedAt.current = Date.now();
      emitted.current = false;
    } else if (expandedAt.current && !emitted.current) {
      // Collapsed — emit dwell + engagement signals
      const dwellSeconds = Math.round((Date.now() - expandedAt.current) / 1000);
      if (dwellSeconds >= 1) {
        emitted.current = true;
        emitExpandSignals(itemId, sourceType, itemTopics, dwellSeconds, 'collapse');
      }
      expandedAt.current = null;
    }
  }, [isExpanded, itemId, sourceType, itemTopics]);

  // Also emit on unmount if still expanded
  useEffect(() => {
    return () => {
      if (expandedAt.current && !emitted.current) {
        const dwellSeconds = Math.round((Date.now() - expandedAt.current) / 1000);
        if (dwellSeconds >= 1) {
          emitted.current = true;
          emitExpandSignals(itemId, sourceType, itemTopics, dwellSeconds, 'unmount');
        }
      }
    };
  }, [itemId, sourceType, itemTopics]);
}
