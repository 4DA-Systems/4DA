// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect } from 'react';
import { cmd } from '../lib/commands';

// ============================================================================
// Privacy-by-default activity tracking gate
// ============================================================================
//
// Activity tracking is OFF until the user explicitly opts in (per the
// project invariants in .ai/INVARIANTS.md).
//
// Every event below is a no-op unless `setActivityTrackingEnabled(true)`
// has been called — which only happens after the app bootstrap reads
// `settings.privacy.activity_tracking_opt_in` from disk AND the user
// has toggled it on in Settings -> Privacy.
//
// Ref: docs/ADVERSARIAL-AUDIT-2026-04-19.md P2 alignment fix.
// ============================================================================

let activityTrackingEnabled: boolean | null = null; // null = unknown -> drop

/**
 * Called by the settings bootstrap (and any runtime toggle in the
 * Privacy settings panel) to enable or disable local activity tracking.
 *
 * While the flag is null or false, every trackEvent/useTelemetryView
 * call is a no-op — no IPC, no SQLite write, nothing.
 */
export function setActivityTrackingEnabled(enabled: boolean): void {
  activityTrackingEnabled = enabled;
}

/**
 * Returns the current runtime state. Tests can read this to assert
 * the no-op default.
 */
export function isActivityTrackingEnabled(): boolean {
  return activityTrackingEnabled === true;
}

/**
 * Fire-and-forget telemetry event.
 *
 * All data stays in local SQLite — no network calls, no external
 * telemetry provider. The gate here is for the user-consent layer:
 * even local recording is off until the user opts in.
 */
export function trackEvent(
  eventType: string,
  viewId?: string,
  metadata?: Record<string, unknown>,
): void {
  if (activityTrackingEnabled !== true) return;
  cmd('track_event', {
    eventType,
    viewId,
    metadata: metadata ? JSON.stringify(metadata) : undefined,
  }).catch((e) => console.debug('[telemetry] track_event:', e));
}

/**
 * Track view open on mount and view duration on unmount.
 *
 * Respects the same opt-in gate as trackEvent. On unmount, the close
 * event is skipped if tracking was off throughout.
 */
export function useTelemetryView(viewId: string): void {
  useEffect(() => {
    const start = Date.now();
    trackEvent(`view_open:${viewId}`, viewId);
    return () => {
      const seconds = Math.round((Date.now() - start) / 1000);
      trackEvent(`view_close:${viewId}`, viewId, { duration_seconds: seconds });
    };
  }, [viewId]);
}
