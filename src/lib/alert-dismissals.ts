// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * One store for alert dismissals (audit 2026-11-11).
 *
 * The critical banner triaged into SQLite (`alert_triage`) with no expiry,
 * while Preemption kept its own 7-day list in localStorage: two policies, two
 * stores, and an "Investigate" click that hid a banner alert forever. Both
 * surfaces now write the backend triage table, keyed by `alert_key`
 * (`item:<source item id>` for banner alerts, `evidence:<id>` for Preemption
 * cards). Every dismissal expires (the backend sets it, see `dismissalDays`)
 * and can be undone: a deliberate "not applicable" or accepted risk holds 90
 * days so it does not nag weekly; investigating, snoozed, acknowledged and
 * fixed hold 7 (product decision 2026-11-13, mirrors alert_triage.rs). `get_preemption_alerts` reads the live Preemption keys
 * itself, so its counts and items still move together (AD-035).
 */
import { cmd } from './commands';

/** How long a dismissal hides an alert before it comes back if still true. */
export const DISMISSAL_DAYS = 7;
/** How long a deliberate judgement (not applicable, accepted risk) holds. */
export const DELIBERATE_DAYS = 90;

/** Days a decision of this kind holds (backend `alert_triage::default_days`). */
export function dismissalDays(action: string): number {
  return action === 'not_applicable' || action === 'accepted_risk' ? DELIBERATE_DAYS : DISMISSAL_DAYS;
}

/** Triage actions that hide an alert. "investigating" never hides one. */
export const HIDING_ACTIONS: ReadonlySet<string> = new Set([
  'fixed',
  'not_applicable',
  'accepted_risk',
  'snoozed',
  'acknowledged',
]);

export function evidenceAlertKey(evidenceId: string): string {
  return `evidence:${evidenceId}`;
}

export interface AlertRef {
  itemId?: number | null;
  alertKey?: string | null;
}

export function dismissAlert(
  ref: AlertRef,
  action: string,
  advisoryId: string | null = null,
  expiresAt: string | null = null,
): Promise<void> {
  return cmd('triage_alert', {
    itemId: ref.itemId ?? null,
    alertKey: ref.alertKey ?? null,
    action,
    advisoryId,
    reason: null,
    expiresAt,
  });
}

export function undoDismissal(ref: AlertRef): Promise<void> {
  return cmd('untriage_alert', { itemId: ref.itemId ?? null, alertKey: ref.alertKey ?? null });
}

// ---------------------------------------------------------------------------
// One-time move of the old localStorage Preemption list into the store.
// ---------------------------------------------------------------------------

const LEGACY_KEY = 'preemption_dismissed';
const LEGACY_TTL_MS = 7 * 24 * 60 * 60 * 1000;
let legacyMove: Promise<void> | null = null;

/**
 * Carry dismissals made before the store moved, each keeping its remaining
 * time. The old entry is removed only after every write succeeded, so a
 * failed move is retried on the next load, never lost.
 */
export function moveLegacyPreemptionDismissals(): Promise<void> {
  legacyMove ??= (async () => {
    let entries: Array<{ id: string; ts: number }> = [];
    try {
      const raw = localStorage.getItem(LEGACY_KEY);
      if (!raw) return;
      const parsed: unknown = JSON.parse(raw);
      // Anything but a list cannot hold a dismissal: clear it, never retry
      // (a non-array used to throw on every load).
      if (!Array.isArray(parsed)) {
        localStorage.removeItem(LEGACY_KEY);
        return;
      }
      entries = parsed as Array<{ id: string; ts: number }>;
    } catch {
      try { localStorage.removeItem(LEGACY_KEY); } catch { /* storage unavailable */ }
      return;
    }
    const now = Date.now();
    try {
      for (const e of entries) {
        if (typeof e?.id !== 'string' || typeof e.ts !== 'number' || now - e.ts >= LEGACY_TTL_MS) continue;
        await dismissAlert(
          { alertKey: evidenceAlertKey(e.id) },
          'acknowledged',
          null,
          new Date(e.ts + LEGACY_TTL_MS).toISOString(),
        );
      }
      localStorage.removeItem(LEGACY_KEY);
    } catch {
      legacyMove = null;
    }
  })();
  return legacyMove;
}

/** Test seam. */
export function resetLegacyMoveForTests(): void {
  legacyMove = null;
}
