// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * The header's freshness line (audit 2026-11-11).
 *
 * It used to show a green "Analysis complete" that never aged and a "LIVE"
 * label that only meant background monitoring was switched on. In a tray app
 * both could be days old. It now states the two ages the app knows: when the
 * last cycle that actually fetched finished, and when advisories were last
 * verified (the same readiness Preemption and Signal read). Past 24 hours,
 * or when the advisory check failed or never ran, it turns amber.
 */
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { AdvisorySync } from '../../../src-tauri/bindings/bindings/AdvisorySync';
import { formatRelativeDate } from '../../utils/format-date';

export const STALE_AFTER_MS = 24 * 60 * 60 * 1000;

export interface FreshnessInput {
  lastCycleAt?: string | null;
  advisorySync?: AdvisorySync | null;
}

export interface FreshnessView {
  /** When the last successful cycle finished; null = none yet. */
  cycleAt: Date | null;
  /** When advisories were last verified; null = never (or unknown). */
  advisoriesAt: Date | null;
  /** The last advisory check failed (data may still be served from disk). */
  advisoryFailed: boolean;
  /** The advisory data is older than the backend's trust horizon. */
  advisoryStale: boolean;
  /** No advisory readiness was reported at all: not checked, never clean. */
  advisoryUnknown: boolean;
  /** Anything here is older than a day, missing, or failed. */
  amber: boolean;
}

function parse(at: string | null | undefined): Date | null {
  if (!at) return null;
  // SQLite times have no zone and are UTC; RFC 3339 times carry one.
  const iso = /[zZ]|[+-]\d\d:?\d\d$/.test(at) ? at : `${at.replace(' ', 'T')}Z`;
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? null : d;
}

export function freshnessView(input: FreshnessInput, now: number = Date.now()): FreshnessView {
  const cycleAt = parse(input.lastCycleAt);
  const sync = input.advisorySync ?? null;
  // Unknown readiness (unreadable, older backend) is "not checked", never clean.
  const advisoryUnknown = sync === null;
  // No ecosystem to check is not a failure of the check.
  const advisoriesApply = sync !== null && sync.state !== 'no_inventory';
  const advisoriesAt = advisoriesApply ? parse(sync.synced_at) : null;
  const advisoryFailed = advisoriesApply && sync.state === 'failed';
  const advisoryStale = advisoriesApply && sync.state === 'stale';
  const old = (d: Date | null) => d === null || now - d.getTime() > STALE_AFTER_MS;
  const amber = old(cycleAt) || advisoryUnknown || advisoryFailed || advisoryStale
    || (advisoriesApply && old(advisoriesAt) && sync.state !== 'syncing' && sync.state !== 'matching');
  return { cycleAt, advisoriesAt, advisoryFailed, advisoryStale, advisoryUnknown, amber };
}

/** Re-render every minute so an age keeps moving on an idle screen. */
function useMinuteTick(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 60_000);
    return () => clearInterval(id);
  }, []);
  return now;
}

export function HeaderFreshness({ lastCycleAt, advisorySync }: FreshnessInput) {
  const { t } = useTranslation();
  const now = useMinuteTick();
  const view = freshnessView({ lastCycleAt, advisorySync }, now);
  const advisoriesApply = advisorySync != null && advisorySync.state !== 'no_inventory';

  const cycle = view.cycleAt
    ? t('header.freshness.updated', { age: formatRelativeDate(view.cycleAt) })
    : t('header.freshness.notUpdated');
  let advisories: string | null = null;
  if (view.advisoryUnknown) {
    advisories = t('header.freshness.advisoriesNotChecked');
  } else if (advisoriesApply) {
    if (view.advisoryStale) {
      advisories = view.advisoriesAt
        ? t('header.freshness.advisoriesStale', { age: formatRelativeDate(view.advisoriesAt) })
        : t('header.freshness.advisoriesNotChecked');
    } else if (view.advisoryFailed) {
      advisories = view.advisoriesAt
        ? t('header.freshness.advisoriesFailedAsOf', { age: formatRelativeDate(view.advisoriesAt) })
        : t('header.freshness.advisoriesFailed');
    } else if (advisorySync.state === 'syncing' || advisorySync.state === 'matching') {
      advisories = t('header.freshness.advisoriesChecking');
    } else {
      advisories = view.advisoriesAt
        ? t('header.freshness.advisories', { age: formatRelativeDate(view.advisoriesAt) })
        : t('header.freshness.advisoriesNever');
    }
  }

  return (
    <span
      data-testid="header-freshness"
      data-amber={view.amber ? 'true' : 'false'}
      className={`text-xs whitespace-nowrap ${view.amber ? 'text-amber-400' : 'text-text-secondary'}`}
    >
      {cycle}
      {advisories && <span className="text-text-muted"> · </span>}
      {advisories}
    </span>
  );
}
