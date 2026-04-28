// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useRef, useCallback } from 'react';
import { cmd } from '../lib/commands';
import { classifyInteractionPattern } from './use-expand-tracking';

export function useDwellTracking(
  itemId: number | null,
  source: string,
  topics: string[],
) {
  const startTimeRef = useRef<number | null>(null);
  const itemIdRef = useRef(itemId);

  itemIdRef.current = itemId;

  const onVisible = useCallback(() => {
    startTimeRef.current = Date.now();
  }, []);

  const onHidden = useCallback(() => {
    if (!startTimeRef.current || !itemIdRef.current) return;

    const dwellSeconds = Math.round(
      (Date.now() - startTimeRef.current) / 1000,
    );
    startTimeRef.current = null;

    if (dwellSeconds < 2 || dwellSeconds > 300) return;

    const pattern = classifyInteractionPattern(dwellSeconds);

    cmd('ace_record_interaction', {
      item_id: itemIdRef.current,
      action_type: 'click',
      action_data: JSON.stringify({
        type: 'click',
        dwell_time_seconds: dwellSeconds,
        pattern,
      }),
      item_topics: topics,
      item_source: source,
    }).catch(() => {});
  }, [source, topics]);

  return { onVisible, onHidden };
}
