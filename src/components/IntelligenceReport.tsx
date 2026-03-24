import { memo, useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';

// ============================================================================
// Types
// ============================================================================

interface MetricData {
  label: string;
  value: string;
  delta?: string;
  trend: 'up' | 'down' | 'flat';
  sublabel?: string;
}

interface IntelligenceData {
  accuracy_pct: number;
  accuracy_delta: number;
  metrics: MetricData[];
  secondary: MetricData[];
  items_processed: number;
}

interface IntelligenceReportRaw {
  period: string;
  accuracy_current: number;
  accuracy_previous: number;
  accuracy_delta: number;
  topics_tracked: number;
  topics_added: number;
  noise_rejected: number;
  noise_rejection_pct: number;
  time_saved_hours: number;
  security_alerts: number;
  security_acted_on: number;
  decisions_recorded: number;
  feedback_signals: number;
}

// ============================================================================
// Sub-components
// ============================================================================

function TrendArrow({ trend }: { trend: 'up' | 'down' | 'flat' }) {
  if (trend === 'up') {
    return <span className="text-success text-xs font-medium">{'\u2191'}</span>;
  }
  if (trend === 'down') {
    return <span className="text-error text-xs font-medium">{'\u2193'}</span>;
  }
  return <span className="text-text-muted text-xs">{'\u2192'}</span>;
}

function MetricRow({ metric }: { metric: MetricData }) {
  return (
    <div className="flex items-center justify-between py-2">
      <span className="text-xs text-text-muted">{metric.label}</span>
      <div className="flex items-center gap-2">
        <span className="text-sm font-medium text-white">{metric.value}</span>
        {metric.delta && (
          <span className={`text-xs font-medium ${
            metric.trend === 'up' ? 'text-success' : metric.trend === 'down' ? 'text-error' : 'text-text-muted'
          }`}>
            {metric.delta}
          </span>
        )}
        <TrendArrow trend={metric.trend} />
        {metric.sublabel && (
          <span className="text-xs text-text-muted/60">{metric.sublabel}</span>
        )}
      </div>
    </div>
  );
}

function LoadingSkeleton() {
  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <div className="h-4 bg-bg-tertiary rounded w-48 animate-pulse" />
        <div className="h-3 bg-bg-tertiary rounded w-64 mt-2 animate-pulse" />
      </div>
      <div className="p-5 space-y-4">
        <div className="h-2 bg-bg-tertiary rounded-full animate-pulse" />
        {[1, 2, 3, 4].map(i => (
          <div key={i} className="flex justify-between py-2">
            <div className="h-3 bg-bg-tertiary rounded w-24 animate-pulse" />
            <div className="h-3 bg-bg-tertiary rounded w-16 animate-pulse" />
          </div>
        ))}
      </div>
    </div>
  );
}

// ============================================================================
// Data fetching
// ============================================================================

function formatNumber(n: number): string {
  return n.toLocaleString();
}

function trendFromDelta(delta: number): 'up' | 'down' | 'flat' {
  if (delta > 0) return 'up';
  if (delta < 0) return 'down';
  return 'flat';
}

function mapReportToIntelligenceData(report: IntelligenceReportRaw, t: (key: string) => string): IntelligenceData {
  const accuracyPct = Math.round(report.accuracy_current * 10) / 10;
  const accuracyDelta = Math.round(report.accuracy_delta * 10) / 10;

  const metrics: MetricData[] = [
    {
      label: t('report.relevanceAccuracy'),
      value: `${accuracyPct}%`,
      delta: `${accuracyDelta >= 0 ? '+' : ''}${accuracyDelta.toFixed(1)}%`,
      trend: trendFromDelta(accuracyDelta),
    },
    {
      label: t('report.topicsTracked'),
      value: formatNumber(report.topics_tracked),
      delta: report.topics_added > 0 ? `+${report.topics_added} new` : undefined,
      trend: trendFromDelta(report.topics_added),
    },
    {
      label: t('report.noiseRejected'),
      value: formatNumber(report.noise_rejected),
      delta: `${report.noise_rejection_pct.toFixed(1)}%`,
      trend: report.noise_rejection_pct > 50 ? 'up' : 'flat',
      sublabel: t('report.rejectionRate'),
    },
    {
      label: t('report.timeSaved'),
      value: `${report.time_saved_hours.toFixed(1)}h`,
      trend: report.time_saved_hours > 0 ? 'up' : 'flat',
    },
  ];

  const secondary: MetricData[] = [
    {
      label: t('report.securityAlerts'),
      value: `${report.security_alerts}`,
      delta: report.security_acted_on > 0 ? `${report.security_acted_on} ${t('report.acted')}` : undefined,
      trend: report.security_acted_on > 0 ? 'up' : 'flat',
    },
    {
      label: t('report.decisions'),
      value: `${report.decisions_recorded}`,
      trend: report.decisions_recorded > 0 ? 'up' : 'flat',
    },
    {
      label: t('report.feedbackSignals'),
      value: `${report.feedback_signals}`,
      trend: report.feedback_signals > 0 ? 'up' : 'flat',
    },
  ];

  return {
    accuracy_pct: accuracyPct,
    accuracy_delta: accuracyDelta,
    metrics,
    secondary,
    items_processed: report.noise_rejected + report.topics_tracked,
  };
}

async function fetchIntelligenceData(t: (key: string) => string): Promise<IntelligenceData> {
  const report = await cmd('get_intelligence_report', { period: undefined }) as IntelligenceReportRaw;
  return mapReportToIntelligenceData(report, t);
}

// ============================================================================
// IntelligenceReportCard
// ============================================================================

const IntelligenceReportCard = memo(function IntelligenceReportCard() {
  const { t } = useTranslation();
  const [data, setData] = useState<IntelligenceData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);

  const loadData = useCallback(() => {
    setLoading(true);
    setError(false);
    fetchIntelligenceData(t)
      .then(setData)
      .catch(() => {
        setData(null);
        setError(true);
      })
      .finally(() => setLoading(false));
  }, [t]);

  useEffect(() => { loadData(); }, [loadData]);

  if (loading) return <LoadingSkeleton />;

  if (error || !data) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
        <div className="px-5 py-4 border-b border-border">
          <h3 className="text-sm font-medium text-white">{t('report.title')}</h3>
        </div>
        <div className="p-5 text-center">
          <p className="text-xs text-text-muted mb-3">{t('report.unableToLoad')}</p>
          <button
            onClick={loadData}
            className="text-xs px-3 py-1.5 rounded bg-bg-tertiary border border-border text-text-secondary hover:text-white transition-colors"
          >
            {t('action.retry')}
          </button>
        </div>
      </div>
    );
  }

  const now = new Date();
  const monthName = now.toLocaleString('default', { month: 'long', year: 'numeric' });

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-sm font-medium text-white">{t('report.title')}</h3>
            <p className="text-xs text-text-muted mt-1">
              {t('report.subtitle', { month: monthName })}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <TrendArrow trend={data.accuracy_delta > 0 ? 'up' : 'down'} />
            <span className={`text-xs font-medium ${data.accuracy_delta >= 0 ? 'text-success' : 'text-error'}`}>
              {data.accuracy_delta >= 0 ? '+' : ''}{data.accuracy_delta.toFixed(1)}%
            </span>
          </div>
        </div>
      </div>

      <div className="p-5 space-y-5">
        {/* Accuracy Progress Bar */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs text-text-muted">{t('report.accuracyImprovement')}</span>
            <span className="text-xs text-white font-medium">{data.accuracy_pct}%</span>
          </div>
          <div
            className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden"
            role="progressbar"
            aria-valuenow={data.accuracy_pct}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <div
              className="h-full rounded-full bg-gradient-to-r from-accent-gold/60 to-accent-gold"
              style={{ width: `${Math.min(data.accuracy_pct, 100)}%` }}
            />
          </div>
          <div className="flex justify-between mt-1.5">
            <span className="text-xs text-text-muted/50">0%</span>
            <span className="text-xs text-text-muted/50">100%</span>
          </div>
        </div>

        {/* Primary Metrics */}
        <div className="bg-bg-primary rounded-lg border border-border/50 px-4 py-1 divide-y divide-border/30">
          {data.metrics.map(metric => (
            <MetricRow key={metric.label} metric={metric} />
          ))}
        </div>

        {/* Secondary Metrics */}
        <div className="grid grid-cols-3 gap-3">
          {data.secondary.map(metric => (
            <div
              key={metric.label}
              className="bg-bg-tertiary rounded-lg border border-border px-3 py-3"
            >
              <div className="flex items-center gap-1.5 mb-1">
                <span className="text-lg font-semibold text-white">{metric.value}</span>
                {metric.delta && (
                  <span className={`text-xs font-medium ${
                    metric.trend === 'up' ? 'text-success' : 'text-text-muted'
                  }`}>
                    {metric.delta}
                  </span>
                )}
              </div>
              <div className="text-xs text-text-muted">{metric.label}</div>
              {metric.sublabel && (
                <div className="text-xs text-text-muted/60 mt-0.5">{metric.sublabel}</div>
              )}
            </div>
          ))}
        </div>

        {/* Footer */}
        <div className="pt-3 border-t border-border/30 flex items-center justify-between">
          <span className="text-xs text-text-muted/50">
            {t('report.basedOn', { items: formatNumber(data.items_processed) })}
          </span>
          <span className="text-xs text-text-muted/50">
            {t('report.updatedToday')}
          </span>
        </div>
      </div>
    </div>
  );
});

export default IntelligenceReportCard;
