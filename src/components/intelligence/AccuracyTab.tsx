// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type {
  AccuracyReport,
  IntelligenceReportData,
  TemporalSnapshot,
  KnowledgeDecayEntry,
} from '../../lib/commands';

// ============================================================================
// Sub-components
// ============================================================================

function StatCard({ label, value, suffix, color }: { label: string; value: string | number; suffix?: string; color?: string }) {
  return (
    <div className="bg-bg-tertiary rounded-lg border border-border px-4 py-3">
      <div className={`text-xl font-semibold ${color ?? 'text-white'}`}>
        {value}{suffix && <span className="text-sm ms-0.5">{suffix}</span>}
      </div>
      <div className="text-xs text-text-muted mt-0.5">{label}</div>
    </div>
  );
}

function LoadingSkeleton() {
  return (
    <div className="p-5 space-y-4">
      <div className="grid grid-cols-4 gap-3">
        {[1, 2, 3, 4].map(i => (
          <div key={i} className="h-16 bg-bg-tertiary rounded-lg animate-pulse" />
        ))}
      </div>
      <div className="h-32 bg-bg-tertiary rounded-lg animate-pulse" />
      <div className="h-24 bg-bg-tertiary rounded-lg animate-pulse" />
    </div>
  );
}

const RISK_COLORS: Record<string, string> = {
  critical: 'text-error',
  high: 'text-[var(--color-accent-action)]',
  medium: 'text-accent-gold',
  low: 'text-text-muted',
};

// ============================================================================
// AccuracyTab
// ============================================================================

export const AccuracyTab = memo(function AccuracyTab() {
  const { t } = useTranslation();
  const [accuracy, setAccuracy] = useState<AccuracyReport | null>(null);
  const [report, setReport] = useState<IntelligenceReportData | null>(null);
  const [snapshot, setSnapshot] = useState<TemporalSnapshot | null>(null);
  const [decaying, setDecaying] = useState<KnowledgeDecayEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    setError(null);

    Promise.allSettled([
      cmd('get_accuracy_report', { period: 'month' }),
      cmd('get_intelligence_report', { period: 'month' }),
      cmd('get_temporal_snapshot', { period: 'month' }),
      cmd('get_knowledge_decay_report'),
    ])
      .then(([accResult, repResult, snapResult, decayResult]) => {
        if (accResult.status === 'fulfilled') setAccuracy(accResult.value);
        if (repResult.status === 'fulfilled') setReport(repResult.value);
        if (snapResult.status === 'fulfilled') setSnapshot(snapResult.value);
        if (decayResult.status === 'fulfilled') setDecaying(decayResult.value);
      })
      .catch(() => setError('Failed to load accuracy data.'))
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <LoadingSkeleton />;

  const hasData = accuracy || report || snapshot || decaying.length > 0;

  if (error && !hasData) {
    return (
      <div className="p-5">
        <div className="bg-error/10 rounded-lg border border-error/20 p-4">
          <p className="text-xs text-error">{error}</p>
        </div>
      </div>
    );
  }

  if (!hasData) {
    return (
      <div className="p-5">
        <div className="bg-bg-primary rounded-lg border border-border/50 p-6 text-center">
          <p className="text-sm text-text-muted mb-2">{t('accuracy.noData')}</p>
          <p className="text-xs text-text-muted/60 leading-relaxed">
            {t('accuracy.noDataDesc')}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-5 space-y-5 overflow-y-auto">
      {/* Summary Stats */}
      {(accuracy || report) && (
        <div className="grid grid-cols-4 gap-3">
          {accuracy && (
            <StatCard
              label={t('accuracy.accuracy')}
              value={accuracy.accuracy_pct.toFixed(1)}
              suffix="%"
              color={accuracy.accuracy_pct >= 70 ? 'text-success' : accuracy.accuracy_pct >= 50 ? 'text-accent-gold' : 'text-error'}
            />
          )}
          {report && (
            <>
              <StatCard label={t('accuracy.topicsTracked')} value={report.topics_tracked} />
              <StatCard label={t('accuracy.noiseRejected')} value={`${report.noise_rejection_pct.toFixed(0)}`} suffix="%" color="text-success" />
              <StatCard label={t('accuracy.timeSaved')} value={report.time_saved_hours.toFixed(1)} suffix="h" color="text-accent-gold" />
            </>
          )}
        </div>
      )}

      {/* Intelligence Report Detail */}
      {report && (
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            {t('accuracy.monthlyIntelligence')}
          </h4>
          <div className="grid grid-cols-2 gap-3">
            <div className="bg-bg-primary rounded-lg border border-border/50 px-4 py-3">
              <div className="flex items-center justify-between">
                <span className="text-xs text-text-muted">{t('accuracy.accuracyDelta')}</span>
                <span className={`text-sm font-medium ${report.accuracy_delta >= 0 ? 'text-success' : 'text-error'}`}>
                  {report.accuracy_delta >= 0 ? '+' : ''}{report.accuracy_delta.toFixed(1)}%
                </span>
              </div>
            </div>
            <div className="bg-bg-primary rounded-lg border border-border/50 px-4 py-3">
              <div className="flex items-center justify-between">
                <span className="text-xs text-text-muted">{t('accuracy.newTopics')}</span>
                <span className="text-sm font-medium text-text-primary">{report.topics_added}</span>
              </div>
            </div>
            <div className="bg-bg-primary rounded-lg border border-border/50 px-4 py-3">
              <div className="flex items-center justify-between">
                <span className="text-xs text-text-muted">{t('accuracy.decisions')}</span>
                <span className="text-sm font-medium text-text-primary">{report.decisions_recorded}</span>
              </div>
            </div>
            <div className="bg-bg-primary rounded-lg border border-border/50 px-4 py-3">
              <div className="flex items-center justify-between">
                <span className="text-xs text-text-muted">{t('accuracy.securityAlerts')}</span>
                <span className={`text-sm font-medium ${report.security_alerts > 0 ? 'text-error' : 'text-success'}`}>
                  {report.security_alerts}
                </span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Tech Snapshot */}
      {snapshot && snapshot.tech_snapshot.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            {t('accuracy.techSnapshot')}
          </h4>
          <div className="space-y-2">
            {snapshot.tech_snapshot.map(tech => (
              <div key={tech.name} className="flex items-center gap-3 bg-bg-primary rounded-lg border border-border/50 px-4 py-2.5">
                <span className="text-sm text-text-primary font-mono flex-1 min-w-0 truncate">{tech.name}</span>
                <div className="flex items-center gap-3 shrink-0">
                  <div className="w-20 h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                    <div
                      className="h-full bg-accent-gold rounded-full"
                      style={{ width: `${Math.min(tech.confidence * 100, 100)}%` }}
                    />
                  </div>
                  <span className="text-xs text-text-muted w-10 text-end">
                    {(tech.confidence * 100).toFixed(0)}%
                  </span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Knowledge Decay */}
      {decaying.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            {t('accuracy.knowledgeDecay')}
          </h4>
          <div className="overflow-hidden rounded-lg border border-border">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-bg-tertiary text-text-muted text-xs uppercase tracking-wider">
                  <th className="text-start px-4 py-2.5 font-medium">{t('accuracy.thTechnology')}</th>
                  <th className="text-start px-4 py-2.5 font-medium">{t('accuracy.thWeeksIdle')}</th>
                  <th className="text-start px-4 py-2.5 font-medium">{t('accuracy.thRisk')}</th>
                  <th className="text-start px-4 py-2.5 font-medium">{t('accuracy.thAction')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {decaying.map(entry => (
                  <tr key={entry.tech_name} className="hover:bg-[#1A1A1A] transition-colors">
                    <td className="px-4 py-2.5 font-mono text-text-primary">{entry.tech_name}</td>
                    <td className="px-4 py-2.5 text-text-secondary">{entry.weeks_since_engagement}</td>
                    <td className="px-4 py-2.5">
                      <span className={`text-xs font-medium ${RISK_COLORS[entry.risk_level] ?? 'text-text-muted'}`}>
                        {entry.risk_level}
                      </span>
                    </td>
                    <td className="px-4 py-2.5 text-xs text-text-muted">{entry.recommendation}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
});
