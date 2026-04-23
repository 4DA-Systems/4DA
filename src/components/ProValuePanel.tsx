// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { formatLocalDate } from '../utils/format-date';
import type { ProValueReport } from '../types';

const STATS_CONFIG = [
  { key: 'briefings_generated', labelKey: 'proValue.aiBriefings' },
  { key: 'signals_detected', labelKey: 'proValue.signalChains' },
  { key: 'knowledge_gaps_caught', labelKey: 'proValue.gapsCaught' },
  { key: 'predictions_made', labelKey: 'proValue.predictions' },
  { key: 'queries_run', labelKey: 'proValue.nlQueries' },
  { key: 'attention_insights', labelKey: 'proValue.blindSpots' },
] as const;

function ChartIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-text-secondary">
      <rect x="1" y="8" width="3" height="6" rx="0.5" fill="currentColor" opacity="0.4" />
      <rect x="6" y="4" width="3" height="10" rx="0.5" fill="currentColor" opacity="0.6" />
      <rect x="11" y="1" width="3" height="13" rx="0.5" fill="currentColor" opacity="0.9" />
    </svg>
  );
}

export const ProValuePanel = memo(function ProValuePanel() {
  const { t } = useTranslation();
  const [report, setReport] = useState<ProValueReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);

  useEffect(() => {
    const load = async () => {
      try {
        const r = await cmd('get_pro_value_report') as unknown as ProValueReport;
        setReport(r);
      } catch {
        setError(true);
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

  if (error) return null;

  if (loading) {
    return (
      <div className="mb-6 bg-bg-secondary rounded-lg border border-border p-5">
        <div className="flex items-center gap-3">
          <div className="w-4 h-4 border-2 border-gray-600 border-t-gray-300 rounded-full animate-spin" />
          <span className="text-xs text-text-muted">{t('proValue.loading')}</span>
        </div>
      </div>
    );
  }

  if (!report) return null;

  const isTrialCta = !report.active_since &&
    report.estimated_hours_saved === 0 &&
    report.briefings_generated === 0 &&
    report.signals_detected === 0;

  const depthPercent = Math.min(100, Math.round((report.data_age_days / 365) * 100));
  const activeSinceFormatted = report.active_since
    ? formatLocalDate(new Date(report.active_since))
    : null;

  return (
    <div className="mb-6 bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border flex items-center gap-3">
        <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
          <ChartIcon />
        </div>
        <div>
          <h2 className="font-medium text-white text-sm">{t('proValue.title')}</h2>
          <p className="text-xs text-text-muted">
            {t('proValue.lastDays', { days: report.period_days })}
            {activeSinceFormatted && <span className="text-text-muted"> / {t('proValue.activeSince', { date: activeSinceFormatted })}</span>}
          </p>
        </div>
      </div>

      <div className="p-4 space-y-4">
        {/* Hours Saved Hero */}
        <div className="text-center py-3">
          {report.estimated_hours_saved > 0 ? (
            <>
              <div className="text-3xl font-semibold text-white tracking-tight">
                {report.estimated_hours_saved}<span className="text-teal-400/80 text-lg ms-1">{t('proValue.hoursSaved')}</span>
              </div>
              <p className="text-xs text-text-muted mt-1">
                {t('proValue.itemsSurfaced', { count: report.items_surfaced })}
              </p>
            </>
          ) : (
            <p className="text-sm text-text-muted">{t('proValue.startTrial')}</p>
          )}
        </div>

        {/* Stats Grid */}
        {!isTrialCta && (() => {
          const activeStats = STATS_CONFIG.filter(({ key }) => (report[key as keyof ProValueReport] as number) > 0);
          if (activeStats.length === 0) return null;
          const cols = activeStats.length <= 2 ? 'grid-cols-2' : 'grid-cols-3';
          return (
            <div className={`grid ${cols} gap-3`}>
              {activeStats.map(({ key, labelKey }) => (
                <div key={key} className="text-center p-2.5 bg-bg-primary rounded-lg border border-border">
                  <div className="text-lg font-semibold text-white">
                    {report[key as keyof ProValueReport] as number}
                  </div>
                  <div className="text-xs text-text-muted mt-0.5">{t(labelKey)}</div>
                </div>
              ))}
            </div>
          );
        })()}

        {/* Data Depth Indicator */}
        {!isTrialCta && (report.total_feedback_events > 0 || report.data_age_days > 0) && (
          <div className="px-3 py-2.5 bg-bg-primary rounded-lg border border-border">
            <div className="text-[10px] text-text-muted uppercase tracking-wider mb-2">{t('proValue.dataDepth')}</div>
            <div className="flex items-center justify-between text-xs text-text-secondary mb-2">
              <span>{t('proValue.learningInteractions', { count: report.total_feedback_events })}</span>
              <span>{t('proValue.dataSpans', { days: report.data_age_days })}</span>
            </div>
            <div className="w-full h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
              <div
                className="h-full bg-gray-500 rounded-full transition-all"
                style={{ width: `${depthPercent}%` }}
              />
            </div>
          </div>
        )}

        {/* Value Bar / Trial CTA */}
        {isTrialCta && (
          <div className="px-4 py-3 bg-bg-primary rounded-lg border border-border text-center">
            <p className="text-xs text-text-secondary leading-relaxed">
              {t('proValue.trialCta')}
            </p>
          </div>
        )}
      </div>
    </div>
  );
});
