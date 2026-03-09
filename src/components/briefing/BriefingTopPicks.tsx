import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { BriefingCard } from '../BriefingCard';
import type { SourceRelevance, FeedbackAction } from '../../types';

interface BriefingTopPicksProps {
  topItems: SourceRelevance[];
  feedbackGiven: Record<number, FeedbackAction>;
  onSave: (item: SourceRelevance) => void;
  onDismiss: (item: SourceRelevance) => void;
  onRecordClick: (item: SourceRelevance) => void;
}

export const BriefingTopPicks = memo(function BriefingTopPicks({
  topItems,
  feedbackGiven,
  onSave,
  onDismiss,
  onRecordClick,
}: BriefingTopPicksProps) {
  const { t } = useTranslation();

  if (topItems.length === 0) return null;

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-medium text-white">{t('briefing.topPicks')}</h3>
        <span className="text-xs text-text-muted">{t('briefing.itemCount', { count: topItems.length })}</span>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        {topItems.map(item => {
          const hasWorkMatch = item.score_breakdown?.intent_boost && item.score_breakdown.intent_boost > 0;
          const hasDep = item.score_breakdown?.dep_match_score && item.score_breakdown.dep_match_score > 0;
          const matchedDeps = item.score_breakdown?.matched_deps;
          return (
            <div key={item.id} className="relative">
              {(hasWorkMatch || hasDep) && (
                <div className="flex items-center gap-1.5 mb-1.5">
                  {hasWorkMatch && (
                    <span className="text-[10px] px-1.5 py-0.5 bg-purple-500/10 text-purple-400 border border-purple-500/20 rounded font-medium">
                      {t('briefing.workingOn')}
                    </span>
                  )}
                  {hasDep && (
                    <span className="text-[10px] px-1.5 py-0.5 bg-blue-500/10 text-blue-400 border border-blue-500/20 rounded font-medium">
                      {matchedDeps ? t('briefing.stackDeps', { deps: matchedDeps.slice(0, 3).join(', ') }) : t('briefing.stack')}
                    </span>
                  )}
                </div>
              )}
              <BriefingCard
                item={item}
                explanation={item.explanation}
                feedbackGiven={feedbackGiven[item.id]}
                onSave={onSave}
                onDismiss={onDismiss}
                onRecordInteraction={onRecordClick}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
});
