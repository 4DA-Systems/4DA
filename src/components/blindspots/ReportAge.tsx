// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { formatRelativeDate } from '../../utils/format-date';

/**
 * A report older than one engine cycle (30 min) would normally have been
 * replaced — so it is a previous run's persisted report, or one still waiting
 * on its background rebuild. Only then is its age worth a line.
 */
export const REPORT_AGE_LABEL_AFTER_MS = 30 * 60 * 1000;

/** Ask again once the background rebuild has had time to land, twice at most. */
export const REPORT_RELOAD_DELAYS_MS = [15_000, 45_000] as const;

/** Milliseconds since `computedAt`, or null when absent or unparseable. */
export function reportAgeMs(computedAt: string | null | undefined, now: number = Date.now()): number | null {
  if (!computedAt) return null;
  const at = Date.parse(computedAt);
  return Number.isNaN(at) ? null : Math.max(0, now - at);
}

/**
 * "Analysis computed 3 hours ago" under a report that is older than a cycle,
 * and a quiet re-fetch so the rebuilt report replaces it without a reload.
 */
export const ReportAge = memo(function ReportAge({
  computedAt,
  onReload,
}: {
  computedAt: string | null | undefined;
  onReload: () => Promise<void> | void;
}) {
  const { t } = useTranslation();
  const age = reportAgeMs(computedAt);
  const stale = age !== null && age >= REPORT_AGE_LABEL_AFTER_MS;

  useEffect(() => {
    if (!stale) return;
    const timers = REPORT_RELOAD_DELAYS_MS.map((ms) => setTimeout(() => { void onReload(); }, ms));
    return () => timers.forEach(clearTimeout);
  }, [stale, computedAt, onReload]);

  if (!stale || !computedAt) return null;
  return (
    <p className="text-[11px] text-text-muted -mt-1" data-testid="blindspots-report-age">
      {t('blindspots.reportAge', { age: formatRelativeDate(computedAt) })}
    </p>
  );
});
