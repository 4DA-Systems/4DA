// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';

const SCORE_TIERS = [
  { max: 25, color: 'text-green-400', bg: 'bg-green-500', labelKey: 'blindspots.score.good' },
  { max: 50, color: 'text-yellow-400', bg: 'bg-yellow-500', labelKey: 'blindspots.score.moderate' },
  { max: 75, color: 'text-orange-400', bg: 'bg-orange-500', labelKey: 'blindspots.score.significant' },
  { max: 100, color: 'text-red-400', bg: 'bg-red-500', labelKey: 'blindspots.score.critical' },
] as const;

const RISK_COLORS: Record<string, string> = {
  critical: 'text-red-400',
  high: 'text-orange-400',
  medium: 'text-yellow-400',
  low: 'text-blue-400',
};

const PRIORITY_STYLES: Record<string, string> = {
  high: 'text-red-400 bg-red-500/10 border-red-500/20',
  medium: 'text-yellow-400 bg-yellow-500/10 border-yellow-500/20',
  low: 'text-blue-400 bg-blue-500/10 border-blue-500/20',
};

function getScoreTier(score: number) {
  return SCORE_TIERS.find(t => score <= t.max) ?? SCORE_TIERS[3];
}

function formatDaysAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const days = Math.floor(diff / 86_400_000);
  if (days === 0) return 'today';
  if (days === 1) return '1 day ago';
  return `${days} days ago`;
}

const ScoreBar = memo(function ScoreBar({ score }: { score: number }) {
  const { t } = useTranslation();
  const tier = getScoreTier(score);
  return (
    <div className="bg-bg-secondary rounded-lg border border-border p-5">
      <div className="flex items-baseline gap-3 mb-3">
        <span className={`text-3xl font-semibold tabular-nums ${tier.color}`}>{score}</span>
        <span className="text-text-muted text-sm">/100</span>
        <span className={`text-sm ${tier.color}`}>{t(tier.labelKey)}</span>
      </div>
      <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-500 ${tier.bg}`}
          style={{ width: `${Math.min(score, 100)}%` }}
        />
      </div>
    </div>
  );
});

const UncoveredDeps = memo(function UncoveredDeps({ deps }: { deps: NonNullable<ReturnType<typeof useAppStore.getState>['blindSpotReport']>['uncovered_dependencies'] }) {
  const { t } = useTranslation();
  if (deps.length === 0) return null;
  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">
          {t('blindspots.uncovered')} ({deps.length})
        </h3>
      </div>
      <div className="divide-y divide-border">
        {deps.map((dep) => (
          <div key={dep.name} className="px-5 py-4">
            <div className="flex items-center gap-2 mb-1">
              <span className="text-sm font-medium text-white">{dep.name}</span>
              <span className="text-[10px] text-text-muted px-1.5 py-0.5 bg-bg-tertiary rounded">
                {dep.dep_type}
              </span>
              {dep.projects_using.length > 0 && (
                <span className="text-[10px] text-text-muted">
                  {dep.projects_using.length} {dep.projects_using.length === 1 ? 'project' : 'projects'}
                </span>
              )}
              <span className={`ms-auto text-[10px] px-1.5 py-0.5 rounded ${RISK_COLORS[dep.risk_level] ?? 'text-text-muted'}`}>
                {dep.risk_level}
              </span>
            </div>
            <p className="text-xs text-text-muted">
              {t('blindspots.uncovered.days', { days: dep.days_since_last_signal })}
              {' \u00B7 '}
              {t('blindspots.uncovered.available', { count: dep.available_signal_count })}
            </p>
          </div>
        ))}
      </div>
    </div>
  );
});

const MissedSignals = memo(function MissedSignals({ signals }: { signals: NonNullable<ReturnType<typeof useAppStore.getState>['blindSpotReport']>['missed_signals'] }) {
  const { t } = useTranslation();
  if (signals.length === 0) return null;
  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">
          {t('blindspots.missed')} ({signals.length})
        </h3>
      </div>
      <div className="divide-y divide-border">
        {signals.map((signal) => (
          <div key={signal.item_id} className="px-5 py-4">
            <div className="mb-1">
              {signal.url ? (
                <a
                  href={signal.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-sm text-white hover:text-amber-400 transition-colors"
                >
                  {signal.title}
                </a>
              ) : (
                <span className="text-sm text-white">{signal.title}</span>
              )}
            </div>
            <div className="flex items-center gap-2 text-xs text-text-muted">
              <span>{t('blindspots.missed.relevance', { score: signal.relevance_score.toFixed(2) })}</span>
              <span>{'\u00B7'}</span>
              <span>{signal.source_type}</span>
              <span>{'\u00B7'}</span>
              <span>{formatDaysAgo(signal.created_at)}</span>
            </div>
            {signal.why_relevant && (
              <p className="text-[11px] text-text-secondary mt-1">{signal.why_relevant}</p>
            )}
          </div>
        ))}
      </div>
    </div>
  );
});

const Recommendations = memo(function Recommendations({ items }: { items: NonNullable<ReturnType<typeof useAppStore.getState>['blindSpotReport']>['recommendations'] }) {
  const { t } = useTranslation();
  if (items.length === 0) return null;
  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">{t('blindspots.recommendations')}</h3>
      </div>
      <div className="p-4 space-y-2">
        {items.map((rec, i) => {
          const style = PRIORITY_STYLES[rec.priority] ?? PRIORITY_STYLES.low;
          return (
            <div key={i} className={`px-4 py-3 rounded-lg border ${style}`}>
              <div className="flex items-center gap-2">
                <span className="text-sm">{rec.action}</span>
                <span className="ms-auto text-[10px] uppercase font-medium">{rec.priority}</span>
              </div>
              {rec.reason && (
                <p className="text-[11px] text-text-secondary mt-1">{rec.reason}</p>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});

const BlindSpotsView = memo(function BlindSpotsView() {
  const { t } = useTranslation();
  const { report, loading, error } = useAppStore(
    useShallow((s) => ({
      report: s.blindSpotReport,
      loading: s.blindSpotsLoading,
      error: s.blindSpotsError,
    })),
  );
  const loadBlindSpots = useAppStore((s) => s.loadBlindSpots);

  useEffect(() => {
    loadBlindSpots();
  }, [loadBlindSpots]);

  const handleRetry = useCallback(() => {
    loadBlindSpots();
  }, [loadBlindSpots]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20 text-text-secondary text-sm">
        {t('blindspots.loading')}
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-20 gap-3">
        <p className="text-red-400 text-sm">{error}</p>
        <button onClick={handleRetry} className="text-sm text-text-secondary hover:text-white transition-colors">
          {t('action.retry')}
        </button>
      </div>
    );
  }

  if (!report) {
    return (
      <div className="flex items-center justify-center py-20 text-text-muted text-sm">
        {t('blindspots.empty')}
      </div>
    );
  }

  const hasContent =
    report.uncovered_dependencies.length > 0 ||
    report.missed_signals.length > 0 ||
    report.recommendations.length > 0;

  return (
    <div className="space-y-4 pb-8">
      <div className="mb-2">
        <h2 className="text-lg font-semibold text-white">{t('blindspots.title')}</h2>
        <p className="text-sm text-text-muted">{t('blindspots.subtitle')}</p>
      </div>

      <ScoreBar score={report.overall_score} />

      {!hasContent ? (
        <div className="bg-bg-secondary rounded-lg border border-border px-5 py-8 text-center">
          <p className="text-sm text-text-muted">{t('blindspots.empty')}</p>
        </div>
      ) : (
        <>
          <UncoveredDeps deps={report.uncovered_dependencies} />
          <MissedSignals signals={report.missed_signals} />
          <Recommendations items={report.recommendations} />
        </>
      )}
    </div>
  );
});

export default BlindSpotsView;
