import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance } from '../../types';
import { getContentTypeBadge } from '../../config/content-types';
import { useAppStore } from '../../store';

interface BadgeRowProps {
  item: SourceRelevance;
}

export const BadgeRow = memo(function BadgeRow({ item }: BadgeRowProps) {
  const { t } = useTranslation();
  const rawAffinities = useAppStore(s => s.learnedAffinities);
  const learnedAffinities = useMemo(() => rawAffinities ?? [], [rawAffinities]);

  // Find which learned topic matches this item (for tooltip)
  const matchedAffinityTopic = useMemo(() => {
    if (!item.score_breakdown || item.score_breakdown.affinity_mult <= 1.0) return null;
    const titleLower = item.title.toLowerCase();
    const positiveAffinities = learnedAffinities.filter(a => a.affinity_score > 0);
    for (const a of positiveAffinities) {
      if (titleLower.includes(a.topic.toLowerCase())) return a.topic;
    }
    return positiveAffinities[0]?.topic || null;
  }, [item.score_breakdown, item.title, learnedAffinities]);

  return (
    <>
      {matchedAffinityTopic && (
        <span
          className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-accent-gold/10 text-accent-gold"
          title={t('results.affinityBoost', { topic: matchedAffinityTopic })}
        >
          {t('results.learnedBadge')}
        </span>
      )}
      {item.decision_window_match && (
        <span
          className="flex-shrink-0 inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded font-medium bg-amber-500/10 text-amber-400 border border-amber-500/20"
          title={t('results.decisionMatch', { subject: item.decision_window_match })}
        >
          <svg
            width="10"
            height="10"
            viewBox="0 0 16 16"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            className="flex-shrink-0"
            aria-hidden="true"
          >
            <path
              d="M8 1v14M8 1L3 6h10L8 1ZM3 6L1 10c0 1.1 0.9 2 2 2s2-0.9 2-2L3 6ZM13 6l-2 4c0 1.1 0.9 2 2 2s2-0.9 2-2L13 6Z"
              stroke="currentColor"
              strokeWidth="1.2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          {t('results.decisionMatch', { subject: item.decision_window_match })}
        </span>
      )}
      {item.serendipity && (
        <span className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-fuchsia-500/20 text-fuchsia-400">
          {t('results.serendipity')}
        </span>
      )}
      {item.signal_type && (
        <span className={`flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium ${
          item.signal_priority === 'critical' ? 'bg-red-500/20 text-red-400' :
          item.signal_priority === 'high' ? 'bg-amber-500/20 text-amber-400' :
          'bg-cyan-500/20 text-cyan-400'
        }`}>
          {t(`results.signal.${item.signal_type}`, { defaultValue: item.signal_type })}
        </span>
      )}
      {item.score_breakdown?.matched_deps && item.score_breakdown.matched_deps.length > 0 && (
        <span
          className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-emerald-500/20 text-emerald-400"
          title={`Affects: ${item.score_breakdown.matched_deps.join(', ')}`}
        >
          {t('results.stackBadge')}
        </span>
      )}
      {item.score_breakdown?.novelty_mult != null && item.score_breakdown.novelty_mult > 1.05 && (
        <span className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-blue-500/20 text-blue-400">
          {t('results.newBadge')}
        </span>
      )}
      {item.score_breakdown?.intent_boost != null && item.score_breakdown.intent_boost > 0 && (
        <span className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-violet-500/20 text-violet-400">
          {t('results.workingOnBadge')}
        </span>
      )}
      {item.streets_engine && (
        <span
          className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-yellow-500/15 text-yellow-400"
          title={`STREETS ${item.streets_engine}`}
        >
          {item.streets_engine.replace(/^Engine \d+: /, '')}
        </span>
      )}
      {item.score_breakdown?.content_quality_mult != null && item.score_breakdown.content_quality_mult >= 1.1 && (
        <span className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-emerald-500/15 text-emerald-400"
          title={`Content quality: ${(item.score_breakdown.content_quality_mult * 100).toFixed(0)}%`}
        >
          {t('results.highQualityBadge')}
        </span>
      )}
      {item.score_breakdown?.content_quality_mult != null && item.score_breakdown.content_quality_mult <= 0.6 && (
        <span className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-orange-500/15 text-orange-400"
          title={`Content quality: ${(item.score_breakdown.content_quality_mult * 100).toFixed(0)}%`}
        >
          {t('results.lowQualityBadge')}
        </span>
      )}
      {(() => {
        const badge = getContentTypeBadge(item.score_breakdown?.content_type);
        return badge ? (
          <span className={`flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium ${badge.colorClass}`}>
            {badge.label}
          </span>
        ) : null;
      })()}
    </>
  );
});
