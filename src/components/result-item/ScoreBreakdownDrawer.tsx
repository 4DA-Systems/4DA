// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useLicense } from '../../hooks/use-license';
import { SignalUpgradeCTA } from '../SignalUpgradeCTA';
import type { ScoreBreakdown, SourceRelevance } from '../../types';
import { getRelevancePresentation } from '../../utils/score';
import { extractFactors, FACTOR_DESCRIPTIONS } from './score-breakdown/factor-utils';
import { FactorGroup } from './score-breakdown/FactorGroup';
import { AdvisorPanel } from './AdvisorPanel';

interface ScoreBreakdownDrawerProps {
  breakdown: ScoreBreakdown;
  finalScore: number;
  itemId: number;
  onClose: () => void;
  /** Optional second breakdown for comparison mode */
  compareBreakdown?: ScoreBreakdown;
  compareScore?: number;
  compareTitle?: string;
  /** Pool of items available for comparison selection */
  comparePool?: SourceRelevance[];
}

export const ScoreBreakdownDrawer = memo(function ScoreBreakdownDrawer({
  breakdown,
  finalScore,
  itemId,
  onClose,
  compareBreakdown: compareBreakdownProp,
  compareScore: compareScoreProp,
  compareTitle: compareTitleProp,
  comparePool,
}: ScoreBreakdownDrawerProps) {
  const { t } = useTranslation();
  const { isPro } = useLicense();
  const addToast = useAppStore(s => s.addToast);
  const [selectedCompareId, setSelectedCompareId] = useState<number | null>(null);
  const [feedbackCount, setFeedbackCount] = useState(0);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [onClose]);

  const onFeedbackGiven = useCallback((factorKey: string, vote: 'up' | 'down') => {
    setFeedbackCount(c => c + 1);
    const desc = FACTOR_DESCRIPTIONS[factorKey] || factorKey;
    addToast('info', `Noted: I'll ${vote === 'up' ? 'boost' : 'reduce'} ${desc} for similar content`);
  }, [addToast]);

  // Resolve comparison: prop takes priority, then user selection from pool
  const selectedItem = selectedCompareId != null
    ? comparePool?.find(i => i.id === selectedCompareId)
    : null;
  const compareBreakdown = compareBreakdownProp ?? selectedItem?.score_breakdown;
  const compareScore = compareScoreProp ?? selectedItem?.top_score;
  const compareTitle = compareTitleProp ?? selectedItem?.title;

  const factors = extractFactors(breakdown);
  const compareFactors = compareBreakdown ? extractFactors(compareBreakdown) : null;

  // Group factors
  const boosts = factors.filter(f => f.effect === 'boost');
  const penalties = factors.filter(f => f.effect === 'penalty');
  const neutrals = factors.filter(f => f.effect === 'neutral');

  // Signal gate status
  const signalCount = breakdown.signal_count ?? 0;
  const confirmedSignals = breakdown.confirmed_signals ?? [];

  return (
    <div className="border-t border-border bg-bg-primary/95 backdrop-blur-sm">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-border/50">
        <div className="flex items-center gap-3">
          <span className={`text-sm font-medium uppercase tracking-wider ${getRelevancePresentation(finalScore).colorClass}`}>
            {t(getRelevancePresentation(finalScore).labelKey)}
          </span>
          <span className="text-[10px] text-text-muted font-mono">
            {Math.round(finalScore * 100)}%
          </span>
          <span className="text-[10px] text-text-muted uppercase tracking-wider">
            {t('scoreDrawer.whyThisAppeared', 'Why this appeared')}
          </span>
          {/* eslint-disable i18next/no-literal-string */}
          {feedbackCount > 0 && (
            <span className="text-[10px] text-green-400">
              {feedbackCount} factor{feedbackCount !== 1 ? 's' : ''} rated this session
            </span>
          )}
          {/* eslint-enable i18next/no-literal-string */}
        </div>
        <button
          onClick={onClose}
          className="text-text-muted hover:text-text-primary transition-colors text-sm px-2 py-1"
          aria-label={t('scoreDrawer.close')}
        >
          &times;
        </button>
      </div>

      {/* Free tier: show score + upgrade prompt */}
      {!isPro && (
        <div className="px-4 py-5 space-y-3 text-center">
          <p className="text-sm text-text-secondary">
            {t('scoreDrawer.freeTeaser', {
              score: Math.round(finalScore * 100),
            })}
          </p>
          <SignalUpgradeCTA compact />
        </div>
      )}

      {isPro && <div className="px-4 py-3 space-y-4 max-h-[50vh] overflow-y-auto">
        {/* Confirmation Gate with Signal Strengths */}
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-[10px] text-text-muted uppercase tracking-wider">
            {t('scoreDrawer.signals')}
          </span>
          {(['context', 'interest', 'ace', 'learned', 'dependency'] as const).map(axis => {
            const confirmed = confirmedSignals.includes(axis);
            // Show strength value for confirmed signals from confidence_by_signal
            const strengthMap: Record<string, string> = {
              context: 'context',
              interest: 'interest',
              ace: 'ace_boost',
              learned: 'feedback',
              dependency: 'dependency',
            };
            const mappedKey = strengthMap[axis];
            const strengthVal = mappedKey != null ? breakdown.confidence_by_signal?.[mappedKey] : undefined;
            return (
              <span
                key={axis}
                role="status"
                className={`text-[10px] px-1.5 py-0.5 rounded border ${
                  confirmed
                    ? 'bg-green-500/15 text-green-400 border-green-500/30'
                    : 'bg-bg-tertiary text-text-muted border-border'
                }`}
                title={confirmed && strengthVal != null ? t('scoreDrawer.signalStrength', { axis, pct: Math.round(strengthVal * 100) }) : undefined}
                aria-label={confirmed
                  ? (strengthVal != null
                    ? t('scoreDrawer.signalConfirmedStrength', { axis, pct: Math.round(strengthVal * 100) })
                    : t('scoreDrawer.signalConfirmed', { axis }))
                  : t('scoreDrawer.signalNotConfirmed', { axis })}
              >
                {confirmed ? '\u2713' : '\u2717'} {axis}
                {confirmed && strengthVal != null && (
                  <span className="text-green-300/60 ms-0.5 font-mono" aria-hidden="true">{Math.round(strengthVal * 100)}</span>
                )}
              </span>
            );
          })}
          <span className="text-[10px] text-text-muted ms-1">
            {signalCount}/5
            {(breakdown.signal_strength_bonus ?? 0) > 0.01 && (
              <span className="text-green-400 ms-1" title={t('scoreDrawer.signalBonusTooltip')}>
                +{Math.round((breakdown.signal_strength_bonus ?? 0) * 100)}
              </span>
            )}
          </span>
        </div>

        {/* Advisor panel — Intelligence Mesh Phase 7b receipts */}
        <AdvisorPanel
          advisorSignals={breakdown.advisor_signals}
          disagreement={breakdown.disagreement}
        />

        {/* Boost factors */}
        {boosts.length > 0 && (
          <FactorGroup
            label={t('scoreDrawer.whyMatched')}
            factors={boosts}
            comparisons={compareFactors}
            itemId={itemId}
            onFeedbackGiven={onFeedbackGiven}
          />
        )}

        {/* Penalty factors */}
        {penalties.length > 0 && (
          <FactorGroup
            label={t('scoreDrawer.whatReduced')}
            factors={penalties}
            comparisons={compareFactors}
            itemId={itemId}
            onFeedbackGiven={onFeedbackGiven}
          />
        )}

        {/* Neutral factors */}
        {neutrals.length > 0 && (
          <FactorGroup
            label={t('scoreDrawer.neutral')}
            factors={neutrals}
            comparisons={compareFactors}
            itemId={itemId}
            onFeedbackGiven={onFeedbackGiven}
          />
        )}

        {/* Comparison section */}
        <div className="pt-2 border-t border-border/50">
          {compareBreakdown && compareScore != null ? (
            <div className="flex items-center justify-between">
              <div className="min-w-0">
                <p className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
                  {t('scoreDrawer.comparing')}
                </p>
                <p className="text-xs text-text-secondary truncate">{compareTitle}</p>
                {/* eslint-disable i18next/no-literal-string */}
                <p className="text-sm font-mono text-text-primary mt-1">
                  {Math.round(compareScore * 100)}% vs {Math.round(finalScore * 100)}%
                  <span className={`ms-2 text-xs ${compareScore > finalScore ? 'text-green-400' : compareScore < finalScore ? 'text-amber-400' : 'text-text-muted'}`}>
                    ({compareScore > finalScore ? '+' : ''}{Math.round((compareScore - finalScore) * 100)})
                  </span>
                </p>
                {/* eslint-enable i18next/no-literal-string */}
              </div>
              {selectedCompareId != null && (
                <button
                  onClick={() => setSelectedCompareId(null)}
                  className="text-[10px] text-text-muted hover:text-text-primary px-2 py-1"
                  aria-label={t('scoreDrawer.clearComparison')}
                >
                  &times;
                </button>
              )}
            </div>
          ) : comparePool && comparePool.length > 1 ? (
            <div>
              <p className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">
                {t('scoreDrawer.compareWith')}
              </p>
              <select
                value=""
                onChange={(e) => setSelectedCompareId(Number(e.target.value))}
                className="w-full bg-bg-tertiary text-xs text-text-secondary rounded border border-border px-2 py-1.5 focus:border-text-primary/30 focus:outline-none"
              >
                <option value="" disabled>{t('scoreDrawer.selectItem')}</option>
                {comparePool
                  .filter(i => i.id !== itemId && i.score_breakdown)
                  .slice(0, 20)
                  /* eslint-disable i18next/no-literal-string */
                  .map(i => (
                    <option key={i.id} value={i.id}>
                      {Math.round(i.top_score * 100)}% — {i.title.slice(0, 60)}
                    </option>
                  ))
                  /* eslint-enable i18next/no-literal-string */
                }
              </select>
            </div>
          ) : null}
        </div>
      </div>}
    </div>
  );
});
