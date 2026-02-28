import { memo, useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import type { ScoreBreakdown, SourceRelevance } from '../../types';
import { extractFactors, FACTOR_DESCRIPTIONS } from './score-breakdown/factor-utils';
import { FactorGroup } from './score-breakdown/FactorGroup';

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
          <span className="text-lg font-bold font-mono text-white">
            {Math.round(finalScore * 100)}%
          </span>
          <span className="text-[10px] text-gray-500 uppercase tracking-wider">
            {t('scoreDrawer.title', 'Score Breakdown')}
          </span>
          {feedbackCount > 0 && (
            <span className="text-[10px] text-green-400">
              {feedbackCount} factor{feedbackCount !== 1 ? 's' : ''} rated this session
            </span>
          )}
        </div>
        <button
          onClick={onClose}
          className="text-gray-500 hover:text-white transition-colors text-sm px-2 py-1"
          aria-label="Close score breakdown"
        >
          &times;
        </button>
      </div>

      <div className="px-4 py-3 space-y-4 max-h-[50vh] overflow-y-auto">
        {/* Confirmation Gate */}
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-[10px] text-gray-500 uppercase tracking-wider">
            {t('scoreDrawer.signals', 'Signals')}
          </span>
          {['context', 'interest', 'ace', 'learned', 'dependency'].map(axis => {
            const confirmed = confirmedSignals.includes(axis);
            return (
              <span
                key={axis}
                className={`text-[10px] px-1.5 py-0.5 rounded border ${
                  confirmed
                    ? 'bg-green-500/15 text-green-400 border-green-500/30'
                    : 'bg-bg-tertiary text-gray-600 border-border'
                }`}
              >
                {confirmed ? '\u2713' : '\u2717'} {axis}
              </span>
            );
          })}
          <span className="text-[10px] text-gray-500 ml-1">
            {signalCount}/5
          </span>
        </div>

        {/* Boost factors */}
        {boosts.length > 0 && (
          <FactorGroup
            label={t('scoreDrawer.whyMatched', 'Why it matched')}
            factors={boosts}
            comparisons={compareFactors}
            itemId={itemId}
            onFeedbackGiven={onFeedbackGiven}
          />
        )}

        {/* Penalty factors */}
        {penalties.length > 0 && (
          <FactorGroup
            label={t('scoreDrawer.whatReduced', 'What reduced it')}
            factors={penalties}
            comparisons={compareFactors}
            itemId={itemId}
            onFeedbackGiven={onFeedbackGiven}
          />
        )}

        {/* Neutral factors */}
        {neutrals.length > 0 && (
          <FactorGroup
            label={t('scoreDrawer.neutral', 'Neutral')}
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
                <p className="text-[10px] text-gray-500 uppercase tracking-wider mb-1">
                  {t('scoreDrawer.comparing', 'Comparing with')}
                </p>
                <p className="text-xs text-gray-400 truncate">{compareTitle}</p>
                <p className="text-sm font-mono text-white mt-1">
                  {Math.round(compareScore * 100)}% vs {Math.round(finalScore * 100)}%
                  <span className={`ml-2 text-xs ${compareScore > finalScore ? 'text-green-400' : compareScore < finalScore ? 'text-amber-400' : 'text-gray-500'}`}>
                    ({compareScore > finalScore ? '+' : ''}{Math.round((compareScore - finalScore) * 100)})
                  </span>
                </p>
              </div>
              {selectedCompareId != null && (
                <button
                  onClick={() => setSelectedCompareId(null)}
                  className="text-[10px] text-gray-600 hover:text-white px-2 py-1"
                  aria-label="Clear comparison"
                >
                  &times;
                </button>
              )}
            </div>
          ) : comparePool && comparePool.length > 1 ? (
            <div>
              <p className="text-[10px] text-gray-500 uppercase tracking-wider mb-1.5">
                {t('scoreDrawer.compareWith', 'Compare with')}
              </p>
              <select
                value=""
                onChange={(e) => setSelectedCompareId(Number(e.target.value))}
                className="w-full bg-bg-tertiary text-xs text-gray-300 rounded border border-border px-2 py-1.5 focus:border-white/30 focus:outline-none"
              >
                <option value="" disabled>{t('scoreDrawer.selectItem', 'Select an item...')}</option>
                {comparePool
                  .filter(i => i.id !== itemId && i.score_breakdown)
                  .slice(0, 20)
                  .map(i => (
                    <option key={i.id} value={i.id}>
                      {Math.round(i.top_score * 100)}% — {i.title.slice(0, 60)}
                    </option>
                  ))
                }
              </select>
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
});
