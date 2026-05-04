// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';
import type { SourceRelevance } from '../../types';
import { getSourceLabel } from '../../config/sources';
import { ConfidenceIndicator } from '../ConfidenceIndicator';

interface ScoreBreakdownRowProps {
  item: SourceRelevance;
  isNew?: boolean;
  isTopPick: boolean;
  isHighConfidence: boolean;
}

export function ScoreBreakdownRow({ item, isNew, isTopPick, isHighConfidence }: ScoreBreakdownRowProps) {
  const { t } = useTranslation();
  return (
    <div className="mb-3 flex flex-wrap items-center gap-2">
      <ConfidenceIndicator
        confidence={item.confidence}
        signalCount={item.score_breakdown?.signal_count}
        confirmedSignals={item.score_breakdown?.confirmed_signals}
      />
      {isNew && (
        <span className="text-[10px] px-1.5 py-0.5 bg-blue-500/20 text-blue-400 rounded font-medium animate-pulse">
          {t('score.new', 'New')}
        </span>
      )}
      {isTopPick && (
        <span className="text-[10px] px-1.5 py-0.5 bg-orange-500/20 text-orange-400 rounded font-medium">
          {isHighConfidence ? t('score.topPick', 'Top pick') : t('score.hot', 'Strong match')}
        </span>
      )}
      {item.seen_on && item.seen_on.length > 1 && (
        <span className="text-[10px] px-1.5 py-0.5 bg-indigo-500/20 text-indigo-400 rounded font-medium">
          {item.seen_on.map(s => getSourceLabel(s)).join(' + ')}
        </span>
      )}
      {item.score_breakdown && (
        <div className="flex gap-2 text-[10px]">
          {item.score_breakdown.context_score > 0.05 && (
            <span className="text-emerald-400/80">
              {t('score.context', 'context')} {Math.round(item.score_breakdown.context_score * 100)}%
            </span>
          )}
          {item.score_breakdown.interest_score > 0.05 && (
            <span className="text-cyan-400/80">
              {t('score.interest', 'interest')} {Math.round(item.score_breakdown.interest_score * 100)}%
            </span>
          )}
          {item.score_breakdown.ace_boost > 0.05 && (
            <span
              className="text-amber-400/80"
              title={t('score.aceTooltip', 'Boosted because this matches your recent work')}
            >
              {t('score.ace', 'recent work')} +{Math.round(item.score_breakdown.ace_boost * 100)}%
            </span>
          )}
          {item.score_breakdown.matched_deps && item.score_breakdown.matched_deps.length > 0 && (
            <span className="text-violet-400/80" title={item.score_breakdown.matched_deps.join(', ')}>
              {t('score.deps', { count: item.score_breakdown.matched_deps.length, defaultValue: `${item.score_breakdown.matched_deps.length} deps` })}
            </span>
          )}
          {item.score_breakdown.affinity_mult > 1.05 && (
            <span className="text-pink-400/80">
              {t('score.affinity', 'affinity')} x{item.score_breakdown.affinity_mult.toFixed(1)}
            </span>
          )}
          {item.score_breakdown.anti_penalty < 0.95 && (
            <span className="text-red-400/80">
              {t('score.penalty', 'penalty')} x{item.score_breakdown.anti_penalty.toFixed(1)}
            </span>
          )}
          {item.score_breakdown.freshness_mult != null && item.score_breakdown.freshness_mult !== 1.0 && (
            <span className={item.score_breakdown.freshness_mult > 1.0 ? 'text-teal-400/80' : 'text-text-muted/80'}>
              {t('score.fresh', 'fresh')} {item.score_breakdown.freshness_mult > 1.0 ? '+' : ''}{Math.round((item.score_breakdown.freshness_mult - 1.0) * 100)}%
            </span>
          )}
        </div>
      )}
    </div>
  );
}
