// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
 * While the flag is null or false, every trackEvent call is a no-op
 * — no IPC, no SQLite write, nothing.
 */
export function setActivityTrackingEnabled(enabled: boolean): void {
  activityTrackingEnabled = enabled;
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
  void cmd('track_event', {
    eventType,
    viewId,
    metadata: metadata ? JSON.stringify(metadata) : undefined,
  }).catch((e) => console.debug('[telemetry] track_event:', e));
}

