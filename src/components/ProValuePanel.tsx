import { useState, useEffect, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ProValueReport } from '../types';

const STATS_CONFIG = [
  { key: 'briefings_generated', label: 'AI Briefings' },
  { key: 'signals_detected', label: 'Signal Chains' },
  { key: 'knowledge_gaps_caught', label: 'Gaps Caught' },
  { key: 'predictions_made', label: 'Predictions' },
  { key: 'queries_run', label: 'NL Queries' },
  { key: 'attention_insights', label: 'Blind Spots' },
] as const;

function ChartIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-gray-400">
      <rect x="1" y="8" width="3" height="6" rx="0.5" fill="currentColor" opacity="0.4" />
      <rect x="6" y="4" width="3" height="10" rx="0.5" fill="currentColor" opacity="0.6" />
      <rect x="11" y="1" width="3" height="13" rx="0.5" fill="currentColor" opacity="0.9" />
    </svg>
  );
}

export const ProValuePanel = memo(function ProValuePanel() {
  const [report, setReport] = useState<ProValueReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);

  useEffect(() => {
    const load = async () => {
      try {
        const r = await invoke<ProValueReport>('get_pro_value_report');
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
          <span className="text-xs text-gray-500">Loading value report...</span>
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
    ? new Date(report.active_since).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
    : null;

  return (
    <div className="mb-6 bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border flex items-center gap-3">
        <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
          <ChartIcon />
        </div>
        <div>
          <h2 className="font-medium text-white text-sm">Pro Intelligence Value</h2>
          <p className="text-xs text-gray-500">
            Last {report.period_days} days
            {activeSinceFormatted && <span className="text-gray-600"> / Active since {activeSinceFormatted}</span>}
          </p>
        </div>
      </div>

      <div className="p-4 space-y-4">
        {/* Hours Saved Hero */}
        <div className="text-center py-3">
          {report.estimated_hours_saved > 0 ? (
            <>
              <div className="text-3xl font-semibold text-white tracking-tight">
                {report.estimated_hours_saved}<span className="text-teal-400/80 text-lg ml-1">h saved</span>
              </div>
              <p className="text-xs text-gray-500 mt-1">
                From {report.items_surfaced} items surfaced
              </p>
            </>
          ) : (
            <p className="text-sm text-gray-500">Start your trial to see value</p>
          )}
        </div>

        {/* Stats Grid */}
        {!isTrialCta && (
          <div className="grid grid-cols-3 gap-3">
            {STATS_CONFIG.map(({ key, label }) => (
              <div key={key} className="text-center p-2.5 bg-bg-primary rounded-lg border border-border">
                <div className="text-lg font-semibold text-white">
                  {report[key as keyof ProValueReport] as number}
                </div>
                <div className="text-xs text-gray-500 mt-0.5">{label}</div>
              </div>
            ))}
          </div>
        )}

        {/* Data Depth Indicator */}
        {!isTrialCta && (report.total_feedback_events > 0 || report.data_age_days > 0) && (
          <div className="px-3 py-2.5 bg-bg-primary rounded-lg border border-border">
            <div className="text-[10px] text-gray-600 uppercase tracking-wider mb-2">Your data depth</div>
            <div className="flex items-center justify-between text-xs text-gray-400 mb-2">
              <span>{report.total_feedback_events} learning interactions</span>
              <span>Data spans {report.data_age_days} days</span>
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
            <p className="text-xs text-gray-400 leading-relaxed">
              Pro features analyze your feed for patterns you'd miss. Start a free trial.
            </p>
          </div>
        )}
      </div>
    </div>
  );
});
