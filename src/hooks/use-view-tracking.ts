/* global IntersectionObserver */
import { useRef, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface UseViewTrackingOptions {
  itemId: number;
  sourceType: string;
  /** Minimum seconds visible before recording (default: 2) */
  threshold?: number;
  /** Whether tracking is enabled (default: true) */
  enabled?: boolean;
}

/**
 * Track view-time for a content item using IntersectionObserver.
 * When the element is visible for `threshold` seconds, emits a passive
 * 'scroll' interaction to the backend for behavior learning.
 */
export function useViewTracking({
  itemId,
  sourceType,
  threshold = 2,
  enabled = true,
}: UseViewTrackingOptions) {
  const containerRef = useRef<HTMLDivElement>(null);
  const visibleSince = useRef<number | null>(null);
  const recorded = useRef(false);

  const recordView = useCallback(
    (visibleSeconds: number) => {
      if (recorded.current) return;
      recorded.current = true;

      invoke('ace_record_interaction', {
        itemId,
        actionType: 'scroll',
        actionData: JSON.stringify({ visible_seconds: visibleSeconds }),
        itemTopics: [],
        itemSource: sourceType,
      }).catch(() => {
        // Silent — passive signal, not critical
      });
    },
    [itemId, sourceType],
  );

  useEffect(() => {
    if (!enabled || !containerRef.current || typeof IntersectionObserver === 'undefined') return;

    const el = containerRef.current;
    recorded.current = false;
    visibleSince.current = null;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          if (!visibleSince.current) {
            visibleSince.current = Date.now();
          }
        } else if (visibleSince.current) {
          const elapsed = (Date.now() - visibleSince.current) / 1000;
          if (elapsed >= threshold) {
            recordView(elapsed);
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
      }
    };
  }, [enabled, threshold, recordView]);

  return containerRef;
}
