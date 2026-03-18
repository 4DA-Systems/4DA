import { memo } from 'react';

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

// ============================================================================
// Mock Data
// ============================================================================

const ACCURACY = 87.4;
const ACCURACY_DELTA = 3.2;

const METRICS: MetricData[] = [
  { label: 'Relevance Accuracy', value: '87.4%', delta: '+3.2%', trend: 'up' },
  { label: 'Topics Tracked', value: '24', delta: '+3', trend: 'up' },
  { label: 'Noise Rejected', value: '1,847', delta: '92.3%', trend: 'up', sublabel: 'rejection rate' },
  { label: 'Time Saved', value: '14.2h', trend: 'up' },
];

const SECONDARY_METRICS: MetricData[] = [
  { label: 'Security Alerts', value: '3', sublabel: '2 acted on', trend: 'flat' },
  { label: 'Decisions Recorded', value: '12', delta: '+4', trend: 'up' },
  { label: 'Feedback Signals', value: '89', delta: '+22', trend: 'up' },
];

// ============================================================================
// Sub-components
// ============================================================================

function TrendArrow({ trend }: { trend: 'up' | 'down' | 'flat' }) {
  if (trend === 'up') {
    return <span className="text-[#22C55E] text-xs font-medium">{'\u2191'}</span>;
  }
  if (trend === 'down') {
    return <span className="text-[#EF4444] text-xs font-medium">{'\u2193'}</span>;
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
            metric.trend === 'up' ? 'text-[#22C55E]' : metric.trend === 'down' ? 'text-[#EF4444]' : 'text-text-muted'
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

// ============================================================================
// IntelligenceReportCard
// ============================================================================

const IntelligenceReportCard = memo(function IntelligenceReportCard() {
  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-sm font-medium text-white">Your Intelligence This Month</h3>
            <p className="text-xs text-text-muted mt-1">
              March 2026 — how 4DA compounded for you.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <TrendArrow trend={ACCURACY_DELTA > 0 ? 'up' : 'down'} />
            <span className="text-xs text-[#22C55E] font-medium">+{ACCURACY_DELTA}%</span>
          </div>
        </div>
      </div>

      <div className="p-5 space-y-5">
        {/* Accuracy Progress Bar */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs text-text-muted">Accuracy Improvement</span>
            <span className="text-xs text-white font-medium">{ACCURACY}%</span>
          </div>
          <div
            className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden"
            role="progressbar"
            aria-valuenow={ACCURACY}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <div
              className="h-full rounded-full bg-gradient-to-r from-[#D4AF37]/60 to-[#D4AF37]"
              style={{ width: `${ACCURACY}%` }}
            />
          </div>
          <div className="flex justify-between mt-1.5">
            <span className="text-xs text-text-muted/50">0%</span>
            <span className="text-xs text-text-muted/50">100%</span>
          </div>
        </div>

        {/* Primary Metrics */}
        <div className="bg-bg-primary rounded-lg border border-border/50 px-4 py-1 divide-y divide-border/30">
          {METRICS.map(metric => (
            <MetricRow key={metric.label} metric={metric} />
          ))}
        </div>

        {/* Secondary Metrics */}
        <div className="grid grid-cols-3 gap-3">
          {SECONDARY_METRICS.map(metric => (
            <div
              key={metric.label}
              className="bg-bg-tertiary rounded-lg border border-border px-3 py-3"
            >
              <div className="flex items-center gap-1.5 mb-1">
                <span className="text-lg font-semibold text-white">{metric.value}</span>
                {metric.delta && (
                  <span className={`text-xs font-medium ${
                    metric.trend === 'up' ? 'text-[#22C55E]' : 'text-text-muted'
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
            Based on 2,003 items processed this month
          </span>
          <span className="text-xs text-text-muted/50">
            Updated today
          </span>
        </div>
      </div>
    </div>
  );
});

export default IntelligenceReportCard;
