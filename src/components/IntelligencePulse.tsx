import { useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { ProGate } from './ProGate';
import { useAppStore } from '../store';

function formatSourceLabel(source: string): string {
  const labels: Record<string, string> = {
    hackernews: 'Hacker News',
    reddit: 'Reddit',
    github_trending: 'GitHub',
    devto: 'Dev.to',
    rss: 'RSS',
    lobsters: 'Lobsters',
  };
  return labels[source] || source;
}

export const IntelligencePulse = memo(function IntelligencePulse() {
  const { t } = useTranslation();
  const data = useAppStore(s => s.intelligencePulse);
  const loadPulse = useAppStore(s => s.loadIntelligencePulse);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    loadPulse();
  }, [loadPulse]);

  if (!data || data.total_cycles === 0) return null;

  const hasCalibrations = data.top_calibrations.length > 0;
  const hasSourceQuality = data.source_quality.length > 0;

  return (
    <ProGate feature={t('intelligence.feature', 'Intelligence Pulse')}>
      <div className="mb-4 bg-bg-secondary rounded-lg border border-border overflow-hidden">
        {/* Header row — always visible */}
        <button
          onClick={() => setExpanded(!expanded)}
          className="w-full px-4 py-3 flex items-center gap-3 text-left hover:bg-bg-tertiary/30 transition-colors"
        >
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center flex-shrink-0">
            <span className="text-sm text-gray-400">&#x2699;</span>
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <h3 className="text-sm font-medium text-white">
                {t('intelligence.title', 'Intelligence Pulse')}
              </h3>
              {data.total_cycles > 0 && (
                <span className="text-[10px] px-1.5 py-0.5 bg-green-500/10 text-green-400 border border-green-500/20 rounded">
                  {t('intelligence.cycles', { count: data.total_cycles })}
                </span>
              )}
            </div>
            {/* Summary line */}
            <p className="text-xs text-gray-500 mt-0.5">
              {t('intelligence.summary', {
                analyzed: data.items_analyzed_7d.toLocaleString(),
                surfaced: data.items_surfaced_7d,
                rate: data.rejection_rate.toFixed(1),
                defaultValue: '{{analyzed}} items processed, {{surfaced}} surfaced ({{rate}}% rejection rate)',
              })}
            </p>
          </div>
          <span className="text-gray-500 text-xs flex-shrink-0">
            {expanded ? '\u25BE' : '\u25B8'}
          </span>
        </button>

        {/* Expanded details */}
        {expanded && (
          <div className="px-4 pb-4 space-y-4 border-t border-border/50">
            {/* Calibration accuracy */}
            <div className="pt-3">
              <div className="flex items-center justify-between mb-2">
                <span className="text-[10px] text-gray-500 uppercase tracking-wider">
                  {t('intelligence.calibration', 'Calibration Accuracy')}
                </span>
                <span className="text-sm font-mono text-white">
                  {Math.round(data.calibration_accuracy * 100)}%
                </span>
              </div>
              <div className="w-full h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all ${
                    data.calibration_accuracy > 0.7 ? 'bg-green-500' :
                    data.calibration_accuracy > 0.4 ? 'bg-yellow-500' : 'bg-red-500'
                  }`}
                  style={{ width: `${data.calibration_accuracy * 100}%` }}
                />
              </div>
            </div>

            {/* What the system learned */}
            {hasCalibrations && (
              <div>
                <p className="text-[10px] text-gray-500 uppercase tracking-wider mb-2">
                  {t('intelligence.learned', 'What the system learned')}
                </p>
                <div className="space-y-1.5">
                  {data.top_calibrations.map((cal) => {
                    const isUnderScored = cal.delta > 0;
                    return (
                      <div key={cal.topic} className="flex items-center gap-2">
                        <span className={`text-xs ${isUnderScored ? 'text-green-400' : 'text-amber-400'}`}>
                          {isUnderScored ? '\u2191' : '\u2193'}
                        </span>
                        <span className="text-xs text-gray-300 flex-1 truncate">
                          {cal.topic}
                        </span>
                        <span className={`text-[10px] font-mono ${isUnderScored ? 'text-green-400' : 'text-amber-400'}`}>
                          {isUnderScored ? '+' : ''}{Math.round(cal.delta * 100)}%
                        </span>
                        <span className="text-[10px] text-gray-600">
                          {cal.sample_size} items
                        </span>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            {/* Source quality */}
            {hasSourceQuality && (
              <div>
                <p className="text-[10px] text-gray-500 uppercase tracking-wider mb-2">
                  {t('intelligence.sourceQuality', 'Source quality')}
                </p>
                <div className="space-y-1.5">
                  {data.source_quality.slice(0, 5).map((sq) => (
                    <div key={sq.source_type} className="flex items-center gap-2">
                      <span className="text-xs text-gray-300 w-24 truncate">
                        {formatSourceLabel(sq.source_type)}
                      </span>
                      <div className="flex-1 h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                        <div
                          className="h-full bg-white/20 rounded-full"
                          style={{ width: `${Math.min(sq.engagement_rate * 100, 100)}%` }}
                        />
                      </div>
                      <span className="text-[10px] font-mono text-gray-400 w-10 text-right">
                        {Math.round(sq.engagement_rate * 100)}%
                      </span>
                      <span className="text-[10px] text-gray-600 w-12 text-right">
                        {sq.items_engaged}/{sq.items_surfaced}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Anti-patterns */}
            {data.anti_patterns_detected > 0 && (
              <div className="flex items-center gap-2 text-xs">
                <span className="text-amber-400">!</span>
                <span className="text-gray-400">
                  {t('intelligence.antiPatterns', {
                    count: data.anti_patterns_detected,
                    defaultValue: '{{count}} scoring anti-pattern(s) detected and corrected',
                  })}
                </span>
              </div>
            )}
          </div>
        )}
      </div>
    </ProGate>
  );
});
