// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';

// ============================================================================
// Sub-components
// ============================================================================

function LoadingSkeleton() {
  return (
    <div className="p-5 space-y-4">
      <div className="grid grid-cols-4 gap-3">
        {[1, 2, 3, 4].map(i => (
          <div key={i} className="h-20 bg-bg-tertiary rounded-lg animate-pulse" />
        ))}
      </div>
      <div className="h-12 bg-bg-tertiary rounded-lg animate-pulse" />
      <div className="h-20 bg-bg-tertiary rounded-lg animate-pulse" />
    </div>
  );
}

function precisionColor(precision: number): string {
  if (precision >= 80) return 'text-green-400';
  if (precision >= 60) return 'text-yellow-400';
  return 'text-red-400';
}

function trendIndicator(trend: string): { symbol: string; color: string } {
  switch (trend) {
    case 'improving': return { symbol: '\u2191', color: 'text-green-400' };
    case 'declining': return { symbol: '\u2193', color: 'text-red-400' };
    default: return { symbol: '\u2192', color: 'text-text-muted' };
  }
}

function MetricCard({
  label,
  value,
  suffix,
  color,
  subtext,
}: {
  label: string;
  value: string | number;
  suffix?: string;
  color?: string;
  subtext?: string;
}) {
  return (
    <div className="bg-bg-tertiary rounded-lg border border-border px-4 py-3">
      <div className={`text-xl font-semibold ${color ?? 'text-white'}`}>
        {value}
        {suffix && <span className="text-sm ms-0.5">{suffix}</span>}
      </div>
      <div className="text-xs text-text-muted mt-0.5">{label}</div>
      {subtext && (
        <div className="text-[10px] text-text-muted/60 mt-0.5">{subtext}</div>
      )}
    </div>
  );
}

// ============================================================================
// TrustDashboard
// ============================================================================

export const TrustDashboard = memo(function TrustDashboard() {
  const { t } = useTranslation();

  const { trustSummary, trustLoading, trustError, loadTrustSummary } = useAppStore(
    useShallow(s => ({
      trustSummary: s.trustSummary,
      trustLoading: s.trustLoading,
      trustError: s.trustError,
      loadTrustSummary: s.loadTrustSummary,
    })),
  );

  useEffect(() => {
    void loadTrustSummary(30);
  }, [loadTrustSummary]);

  if (trustLoading && !trustSummary) return <LoadingSkeleton />;

  if (trustError && !trustSummary) {
    return (
      <div className="p-5">
        <div className="text-xs text-text-muted">{trustError}</div>
      </div>
    );
  }

  if (!trustSummary || trustSummary.total_surfaced === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-48 text-center px-8">
        <div className="w-3 h-3 rounded-full bg-success/60 animate-pulse mb-4" />
        <p className="text-xs text-text-muted max-w-xs">
          {t('trust.empty')}
        </p>
      </div>
    );
  }

  const s = trustSummary;
  const precisionPct = Math.round(s.precision * 100);
  const actionPct = Math.round(s.action_conversion_rate * 100);
  const fpPct = s.total_surfaced > 0
    ? Math.round((s.false_positives / s.total_surfaced) * 100)
    : 0;
  const trend = trendIndicator(s.trend);
  const leadTimeDays = s.avg_lead_time_hours != null
    ? (s.avg_lead_time_hours / 24).toFixed(1)
    : null;

  const hasEnoughData = s.total_surfaced >= 10;

  return (
    <div className="p-5 space-y-4">
      {/* Title */}
      <div>
        <h4 className="text-sm font-medium text-white">
          {t('trust.title')}
        </h4>
        <p className="text-[10px] text-text-muted mt-0.5">
          {t('trust.subtitle', { days: s.period_days })}
        </p>
      </div>

      {/* Metric cards */}
      <div className="grid grid-cols-4 gap-3">
        <MetricCard
          label={t('trust.precision')}
          value={`${precisionPct}%`}
          color={precisionColor(precisionPct)}
        />
        <MetricCard
          label={t('trust.actionRate')}
          value={`${actionPct}%`}
          color="text-white"
        />
        <MetricCard
          label={t('trust.fpRate')}
          value={`${fpPct}%`}
          color={fpPct <= 10 ? 'text-green-400' : fpPct <= 20 ? 'text-yellow-400' : 'text-red-400'}
        />
        <MetricCard
          label={t('trust.caughtEarly')}
          value={s.preemption_wins}
          color="text-accent-gold"
          subtext={leadTimeDays != null ? t('trust.avgLeadTime', { days: leadTimeDays }) : undefined}
        />
      </div>

      {/* Summary line */}
      <div className="flex items-center gap-2 text-xs text-text-secondary">
        <span>
          {t('trust.summary', {
            surfaced: s.total_surfaced,
            acted: s.acted_on,
            dismissed: s.dismissed,
          })}
        </span>
        <span className="text-text-muted">&middot;</span>
        <span>
          {t('trust.falsePositives', { count: s.false_positives })}
        </span>
        <span className="text-text-muted">&middot;</span>
        <span className={trend.color}>
          {trend.symbol} {t(`trust.trend.${s.trend}` as const)}
        </span>
      </div>

      {/* Explanation box */}
      <div className="bg-bg-tertiary rounded-lg border border-border px-4 py-3">
        <p className="text-xs text-text-secondary leading-relaxed">
          {hasEnoughData
            ? t('trust.explanation.good', {
                precision: precisionPct,
                wins: s.preemption_wins,
              })
            : t('trust.explanation.building')
          }
          {leadTimeDays != null && hasEnoughData && (
            <span className="text-text-muted">
              {' '}{t('trust.avgLeadTime', { days: leadTimeDays })}
            </span>
          )}
        </p>
      </div>
    </div>
  );
});
