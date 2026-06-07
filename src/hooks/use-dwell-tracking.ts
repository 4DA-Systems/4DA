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

    void cmd('ace_record_interaction', {
      itemId: itemIdRef.current,
      actionType: 'click',
      actionData: JSON.stringify({
        type: 'click',
        dwell_time_seconds: dwellSeconds,
        pattern,
      }),
      itemTopics: topics,
      itemSource: source,
    }).catch(() => {});
  }, [source, topics]);

  return { onVisible, onHidden };
}
