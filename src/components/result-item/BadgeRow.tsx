import type { SourceRelevance } from '../../types';
import { getContentTypeBadge } from '../../config/content-types';

interface BadgeRowProps {
  item: SourceRelevance;
}

export function BadgeRow({ item }: BadgeRowProps) {
  return (
    <>
      {item.serendipity && (
        <span className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-fuchsia-500/20 text-fuchsia-400">
          Serendipity
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
          Stack
        </span>
      )}
      {item.score_breakdown?.novelty_mult != null && item.score_breakdown.novelty_mult > 1.05 && (
        <span className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-blue-500/20 text-blue-400">
          New
        </span>
      )}
      {item.score_breakdown?.intent_boost != null && item.score_breakdown.intent_boost > 0 && (
        <span className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium bg-violet-500/20 text-violet-400">
          Working on
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
