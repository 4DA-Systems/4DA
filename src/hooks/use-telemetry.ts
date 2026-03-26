import { useEffect } from 'react';
import { cmd } from '../lib/commands';

/**
 * Fire-and-forget telemetry event — never blocks UI.
 * All data stays in local SQLite. No network calls.
 */
export function trackEvent(
  eventType: string,
  viewId?: string,
  metadata?: Record<string, unknown>,
) {
  cmd('track_event', {
    eventType,
    viewId,
    metadata: metadata ? JSON.stringify(metadata) : undefined,
  }).catch((e) => console.debug('[telemetry] track_event:', e));
}

/**
 * Track view open on mount and view duration on unmount.
 */
export function useTelemetryView(viewId: string) {
  useEffect(() => {
    const start = Date.now();
    trackEvent(`view_open:${viewId}`, viewId);
    return () => {
      const seconds = Math.round((Date.now() - start) / 1000);
      trackEvent(`view_close:${viewId}`, viewId, { duration_seconds: seconds });
    };
  }, [viewId]);
}
