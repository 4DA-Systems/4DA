import { useEffect, useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { ProGate } from './ProGate';
import type { AutophagyStatus, AutophagyCycleResult } from '../types/autophagy';

function CalibrationHeatmap({ status, t }: { status: AutophagyStatus; t: (key: string) => string }) {
  const accuracy = status.total_calibrations > 0
    ? Math.min(status.total_calibrations / 10, 1.0)
    : 0;
  const pct = Math.round(accuracy * 100);
  const color = pct >= 70 ? 'bg-green-400' : pct >= 40 ? 'bg-amber-400' : 'bg-red-400';

  return (
    <div>
      <div className="flex items-center justify-between mb-1">
        <span className="text-xs text-gray-400">{t('autophagy.calibrationAccuracy')}</span>
        <span className="text-xs text-gray-300 tabular-nums">{pct}%</span>
      </div>
      <div className="h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
        <div className={`h-full rounded-full transition-all ${color}`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

function CycleHistory({ history, t }: { history: AutophagyCycleResult[]; t: (key: string) => string }) {
  if (history.length === 0) {
    return <p className="text-xs text-gray-500">{t('autophagy.noCycles')}</p>;
  }

  return (
    <div className="space-y-2">
      {history.slice(0, 5).map((cycle: AutophagyCycleResult, i: number) => (
        <div key={i} className="flex items-center justify-between text-xs">
          <div className="flex items-center gap-3">
            <span className="text-gray-500 tabular-nums">{cycle.items_analyzed} items</span>
            <span className="text-green-400/60 tabular-nums">+{cycle.calibrations_produced} cal</span>
            {cycle.anti_patterns_detected > 0 && (
              <span className="text-amber-400/60 tabular-nums">{cycle.anti_patterns_detected} anti</span>
            )}
          </div>
          <span className="text-gray-600 tabular-nums">{cycle.duration_ms}ms</span>
        </div>
      ))}
    </div>
  );
}

function AntiPatternsSummary({ count, t }: { count: number; t: (key: string, opts?: Record<string, unknown>) => string }) {
  if (count === 0) return null;
  return (
    <div className="flex items-center gap-2 px-3 py-2 bg-amber-500/5 border border-amber-500/15 rounded-lg">
      <span className="text-amber-400 text-xs">!</span>
      <span className="text-xs text-amber-300">
        {t('autophagy.antiPatternsDetected', { count })}
      </span>
    </div>
  );
}

const InsightsContent = memo(function InsightsContent() {
  const { t } = useTranslation();
  const status = useAppStore(s => s.autophagyStatus);
  const history = useAppStore(s => s.autophagyHistory);
  const loading = useAppStore(s => s.autophagyLoading);
  const loadStatus = useAppStore(s => s.loadAutophagyStatus);
  const loadHistory = useAppStore(s => s.loadAutophagyHistory);

  useEffect(() => {
    loadStatus();
    loadHistory(5);
  }, [loadStatus, loadHistory]);

  const stats = useMemo(() => {
    if (!status) return null;
    return {
      cycles: status.total_cycles,
      calibrations: status.total_calibrations,
      antiPatterns: status.total_anti_patterns,
      lastAnalyzed: status.last_cycle?.items_analyzed ?? 0,
    };
  }, [status]);

  if (loading && !status) {
    return (
      <div className="space-y-3 animate-pulse">
        <div className="h-4 bg-bg-tertiary rounded w-1/3" />
        <div className="h-20 bg-bg-tertiary rounded" />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Summary Stats */}
      {stats && (
        <div className="grid grid-cols-3 gap-3">
          <div className="text-center">
            <div className="text-lg font-semibold text-white tabular-nums">{stats.cycles}</div>
            <div className="text-[10px] text-gray-500 uppercase">{t('autophagy.cycles')}</div>
          </div>
          <div className="text-center">
            <div className="text-lg font-semibold text-green-400 tabular-nums">{stats.calibrations}</div>
            <div className="text-[10px] text-gray-500 uppercase">{t('autophagy.calibrations')}</div>
          </div>
          <div className="text-center">
            <div className="text-lg font-semibold text-amber-400 tabular-nums">{stats.antiPatterns}</div>
            <div className="text-[10px] text-gray-500 uppercase">{t('autophagy.antiPatterns')}</div>
          </div>
        </div>
      )}

      {/* Calibration Accuracy */}
      {status && <CalibrationHeatmap status={status} t={t} />}

      {/* Anti-Patterns Alert */}
      {stats && <AntiPatternsSummary count={stats.antiPatterns} t={t} />}

      {/* Cycle History */}
      <div>
        <h4 className="text-xs font-medium text-gray-300 mb-2">{t('autophagy.recentCycles')}</h4>
        <CycleHistory history={history} t={t} />
      </div>
    </div>
  );
});

export const AutophagyInsights = memo(function AutophagyInsights() {
  const { t } = useTranslation();
  return (
    <div className="bg-bg-secondary rounded-lg border border-border p-5">
      <div className="flex items-center gap-2 mb-4">
        <div className="w-6 h-6 bg-purple-500/20 rounded-md flex items-center justify-center">
          <span className="text-purple-400 text-xs">M</span>
        </div>
        <h3 className="text-sm font-medium text-white">{t('autophagy.title')}</h3>
      </div>
      <ProGate feature={t('autophagy.feature')}>
        <InsightsContent />
      </ProGate>
    </div>
  );
});
