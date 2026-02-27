import { useState, useEffect, useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import type { DelegationScoreEntry } from '../store/agent-slice';

const RECOMMENDATION_STYLES: Record<string, { color: string; bg: string; border: string; label: string }> = {
  fully_delegate: {
    color: '#22C55E',
    bg: 'bg-green-500/10',
    border: 'border-green-500/20',
    label: 'Fully Delegate',
  },
  delegate_with_review: {
    color: '#D4AF37',
    bg: 'bg-yellow-500/10',
    border: 'border-yellow-500/20',
    label: 'Delegate w/ Review',
  },
  collaborate_realtime: {
    color: '#F59E0B',
    bg: 'bg-orange-500/10',
    border: 'border-orange-500/20',
    label: 'Collaborate',
  },
  human_only: {
    color: '#EF4444',
    bg: 'bg-red-500/10',
    border: 'border-red-500/20',
    label: 'Human Only',
  },
};

const DEFAULT_RECOMMENDATION_STYLE = {
  color: '#A0A0A0',
  bg: 'bg-gray-500/10',
  border: 'border-gray-500/20',
  label: 'Unknown',
};

const FACTOR_LABELS: Record<string, string> = {
  pattern_stability: 'Pattern Stability',
  security_sensitivity: 'Security Sensitivity',
  codebase_complexity: 'Codebase Complexity',
  decision_density: 'Decision Density',
  ai_track_record: 'AI Track Record',
};

function ScoreBar({ value, color }: { value: number; color: string }) {
  const pct = Math.round(value * 100);
  return (
    <div className="w-full h-2 bg-bg-primary rounded-full overflow-hidden">
      <div
        className="h-full rounded-full transition-all duration-300"
        style={{ width: `${pct}%`, backgroundColor: color }}
      />
    </div>
  );
}

function FactorBar({ label, value }: { label: string; value: number }) {
  const pct = Math.round(value * 100);
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between">
        <span className="text-[10px] text-text-secondary">{label}</span>
        <span className="text-[10px] font-mono text-text-muted">{pct}%</span>
      </div>
      <div className="w-full h-1.5 bg-bg-primary rounded-full overflow-hidden">
        <div
          className="h-full rounded-full bg-white/30 transition-all duration-300"
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

function DelegationCard({ entry }: { entry: DelegationScoreEntry }) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const style = RECOMMENDATION_STYLES[entry.recommendation] || DEFAULT_RECOMMENDATION_STYLE;
  const pct = Math.round(entry.overall_score * 100);

  return (
    <div className="rounded-lg border border-border bg-bg-tertiary/50 transition-all hover:border-white/10">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full p-4 text-left"
      >
        <div className="flex items-center justify-between mb-2">
          <span className="text-sm text-white font-medium">{entry.subject}</span>
          <span className="text-xs font-mono" style={{ color: style.color }}>
            {pct}%
          </span>
        </div>
        <ScoreBar value={entry.overall_score} color={style.color} />
        <div className="mt-2 flex items-center justify-between">
          <span
            className={`text-[10px] px-1.5 py-0.5 rounded ${style.bg} border ${style.border}`}
            style={{ color: style.color }}
          >
            {style.label}
          </span>
          <span className="text-text-muted text-xs">{expanded ? '\u25BE' : '\u25B8'}</span>
        </div>
      </button>

      {expanded && (
        <div className="px-4 pb-4 border-t border-border/50 space-y-3">
          {/* Factor breakdown */}
          <div className="mt-3">
            <div className="text-[10px] text-text-muted uppercase tracking-wider mb-2">
              {t('delegation.factorBreakdown')}
            </div>
            <div className="space-y-2">
              {Object.entries(entry.factors).map(([key, val]) => (
                <FactorBar
                  key={key}
                  label={FACTOR_LABELS[key] || key}
                  value={val}
                />
              ))}
            </div>
          </div>

          {/* Caveats */}
          {entry.caveats.length > 0 && (
            <div>
              <div className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
                {t('delegation.caveats')}
              </div>
              <ul className="space-y-1">
                {entry.caveats.map((caveat, i) => (
                  <li key={i} className="text-xs text-text-secondary flex items-start gap-1.5">
                    <span className="text-text-muted mt-0.5">-</span>
                    <span>{caveat}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export const DelegationDashboard = memo(function DelegationDashboard() {
  const { t } = useTranslation();
  const { delegationScores } = useAppStore(
    useShallow((s) => ({
      delegationScores: s.delegationScores,
    })),
  );

  const loadDelegationScores = useAppStore((s) => s.loadDelegationScores);

  useEffect(() => {
    loadDelegationScores();
  }, [loadDelegationScores]);

  const summaryStats = useMemo(() => {
    if (delegationScores.length === 0) return null;
    const fullyDelegatable = delegationScores.filter(
      (s) => s.recommendation === 'fully_delegate',
    ).length;
    const pct = Math.round((fullyDelegatable / delegationScores.length) * 100);
    return { fullyDelegatable, total: delegationScores.length, pct };
  }, [delegationScores]);

  if (delegationScores.length === 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
        <div className="px-5 py-4 border-b border-border flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <span className="text-sm text-text-muted">S</span>
          </div>
          <h2 className="font-medium text-white text-sm">{t('delegation.title')}</h2>
        </div>
        <div className="p-8 text-center">
          <div className="text-sm text-text-secondary">{t('delegation.empty')}</div>
          <div className="text-xs text-text-muted mt-1">
            {t('delegation.emptyHint')}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <span className="text-sm text-text-muted">S</span>
          </div>
          <div>
            <h2 className="font-medium text-white text-sm">{t('delegation.title')}</h2>
            <p className="text-xs text-text-muted">
              {t('delegation.count', { count: delegationScores.length })}
            </p>
          </div>
        </div>
      </div>

      {/* Summary stats */}
      {summaryStats && (
        <div className="px-5 py-3 border-b border-border flex items-center gap-3">
          <div className="flex items-center gap-2">
            <span className="text-xs text-text-secondary">
              <span className="text-white font-medium">{summaryStats.pct}%</span> {t('delegation.delegatable')}
            </span>
          </div>
          <div className="flex-1 h-1.5 bg-bg-primary rounded-full overflow-hidden ml-3">
            <div
              className="h-full rounded-full transition-all duration-300"
              style={{
                width: `${summaryStats.pct}%`,
                backgroundColor: '#22C55E',
              }}
            />
          </div>
          <span className="text-[10px] font-mono text-text-muted">
            {summaryStats.fullyDelegatable}/{summaryStats.total}
          </span>
        </div>
      )}

      {/* Card grid */}
      <div className="p-4 grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
        {delegationScores.map((entry) => (
          <DelegationCard key={entry.subject} entry={entry} />
        ))}
      </div>
    </div>
  );
});
