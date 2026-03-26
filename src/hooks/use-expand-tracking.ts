import { useRef, useEffect } from 'react';
import { cmd } from '../lib/commands';

/**
 * Track dwell time when a content item is expanded.
 * On collapse or unmount, emits a 'click' interaction with dwell_time_seconds.
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
      // Collapsed — emit dwell signal
      const dwellSeconds = Math.round((Date.now() - expandedAt.current) / 1000);
      if (dwellSeconds >= 1) {
        emitted.current = true;
        cmd('ace_record_interaction', {
          item_id: itemId,
          action_type: 'click',
          action_data: JSON.stringify({ type: 'click', dwell_time_seconds: dwellSeconds }),
          item_topics: itemTopics,
          item_source: sourceType,
        }).catch((e) => console.debug('[expand-tracking] record:', e));
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
          cmd('ace_record_interaction', {
            item_id: itemId,
            action_type: 'click',
            action_data: JSON.stringify({ type: 'click', dwell_time_seconds: dwellSeconds }),
            item_topics: itemTopics,
            item_source: sourceType,
          }).catch((e) => console.debug('[expand-tracking] unmount record:', e));
        }
      }
    };
  }, [itemId, sourceType, itemTopics]);
}
