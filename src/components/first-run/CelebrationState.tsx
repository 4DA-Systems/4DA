import { useTranslation } from 'react-i18next';

import { VoidEngine } from '../void-engine/VoidEngine';
import { getCelebrationMessage } from '../../utils/first-run-messages';
import { getSourceFullName } from '../../config/sources';

interface TopSignal {
  title: string;
  url: string | null;
  score_breakdown?: {
    dep_match_score?: number;
    matched_deps?: string[];
    skill_gap_boost?: number;
  };
}

interface CelebrationStateProps {
  relevantCount: number;
  totalCount: number;
  sourceBreakdown: Array<[string, number]>;
  topSignal: TopSignal | null;
  stackInsights: string[];
  embeddingMode: string | null;
  onDismiss: (view: 'briefing' | 'results' | 'playbook') => void;
}

export function CelebrationState({
  relevantCount,
  totalCount,
  sourceBreakdown,
  topSignal,
  stackInsights,
  embeddingMode,
  onDismiss,
}: CelebrationStateProps) {
  const { t } = useTranslation();

  return (
    <div className="text-center px-8 max-w-lg">
      <div className="mb-6">
        <VoidEngine size={80} />
      </div>

      {/* Big relevant count */}
      <div className="mb-4">
        <span className="text-6xl font-bold text-white tabular-nums">{relevantCount}</span>
        <p className="text-sm text-gray-400 mt-2">
          {getCelebrationMessage(relevantCount, totalCount)}
        </p>
      </div>

      {/* Stack-specific insights */}
      {stackInsights.length > 0 && (
        <div className="mb-6 space-y-2 max-w-sm mx-auto">
          {stackInsights.slice(0, 3).map((insight, i) => (
            <div key={i} className="px-4 py-2.5 bg-bg-secondary rounded-lg border border-border text-left">
              <p className="text-xs text-gray-300 leading-relaxed">{insight}</p>
            </div>
          ))}
        </div>
      )}

      {/* Source breakdown */}
      {sourceBreakdown.length > 0 && (
        <div className="flex flex-wrap justify-center gap-2 mb-6">
          {sourceBreakdown.map(([src, count]) => (
            <span key={src} className="px-2.5 py-1 text-xs bg-bg-secondary text-gray-300 rounded-lg border border-border">
              {getSourceFullName(src)} <span className="text-gray-500">{count}</span>
            </span>
          ))}
        </div>
      )}

      {/* Top signal highlight */}
      {topSignal && (
        <div className="mb-6 p-4 bg-bg-secondary rounded-lg border border-orange-500/20 text-left max-w-sm mx-auto">
          <p className="text-[10px] text-orange-400 font-medium uppercase tracking-wider mb-1">
            {topSignal.score_breakdown?.dep_match_score && topSignal.score_breakdown.dep_match_score > 0
              ? t('firstRun.topMatchStack', 'Matches your stack')
              : t('firstRun.topMatch')}
          </p>
          <p className="text-sm text-white font-medium leading-snug line-clamp-2">{topSignal.title}</p>
          {topSignal.score_breakdown?.matched_deps && topSignal.score_breakdown.matched_deps.length > 0 && (
            <p className="text-[10px] text-blue-400 mt-1">
              {topSignal.score_breakdown.matched_deps.slice(0, 3).join(', ')}
            </p>
          )}
          <p className="text-xs text-gray-500 mt-1 truncate">{topSignal.url}</p>
        </div>
      )}

      {/* Basic Mode indicator */}
      {embeddingMode === 'keyword-only' && (
        <div className="mb-6 px-4 py-3 bg-amber-500/10 border border-amber-500/30 rounded-lg max-w-sm mx-auto text-left">
          <p className="text-xs font-medium text-amber-400">
            {t('firstRun.basicMode')}
          </p>
          <p className="text-[11px] text-amber-400/70 mt-1">
            {t('firstRun.basicModeHint')}
          </p>
        </div>
      )}

      {/* CTAs */}
      <div className="flex flex-col items-center gap-3">
        <button
          onClick={() => onDismiss('briefing')}
          aria-label="View your intelligence briefing"
          className="px-8 py-3 bg-orange-500 text-white font-medium rounded-lg hover:bg-orange-600 hover:scale-105 active:scale-95 transition-all"
        >
          {t('firstRun.seeBriefing')}
        </button>
        <button
          onClick={() => onDismiss('results')}
          aria-label={`Browse ${totalCount} analyzed results`}
          className="text-sm text-gray-500 hover:text-gray-300 transition-colors"
        >
          {t('firstRun.browseResults', { count: totalCount })}
        </button>
      </div>

      {/* STREETS nudge */}
      <div className="mt-8 pt-4 border-t border-border/50">
        <p className="text-xs text-gray-500 mb-1">{t('firstRun.streetsNudge')}</p>
        <button
          onClick={() => onDismiss('playbook')}
          aria-label="Explore the STREETS developer playbook"
          className="text-xs font-medium hover:underline transition-colors"
          style={{ color: '#D4AF37' }}
        >
          {t('firstRun.exploreStreets')}
        </button>
      </div>
    </div>
  );
}
