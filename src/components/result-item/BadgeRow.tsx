import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance } from '../../types';
import { getContentTypeBadge } from '../../config/content-types';
import { useAppStore } from '../../store';

interface BadgeRowProps {
  item: SourceRelevance;
}

export function BadgeRow({ item }: BadgeRowProps) {
  const { t } = useTranslation();
  const learnedAffinities = useAppStore(s => s.learnedAffinities);

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
          className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-[#D4AF37]/10 text-[#D4AF37]"
          title={`Boosted because you've shown interest in ${matchedAffinityTopic}`}
        >
          Learned
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
          {{ security_alert: 'Security', breaking_change: 'Breaking', tool_discovery: 'Tool',
             tech_trend: 'Trend', learning: 'Learn', competitive_intel: 'Intel',
          }[item.signal_type] || item.signal_type}
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
}
