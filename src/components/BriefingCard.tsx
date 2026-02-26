import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance, FeedbackAction } from '../types';
import { formatScore, getScoreColor } from '../utils/score';
import { getSourceLabel, getSourceColorClass } from '../config/sources';
import { getContentTypeBadge } from '../config/content-types';

interface BriefingCardProps {
  item: SourceRelevance;
  explanation?: string;
  feedbackGiven?: FeedbackAction;
  onSave?: (item: SourceRelevance) => void;
  onDismiss?: (item: SourceRelevance) => void;
  onRecordInteraction?: (item: SourceRelevance) => void;
}

export const BriefingCard = memo(function BriefingCard({
  item,
  explanation,
  feedbackGiven,
  onSave,
  onDismiss,
  onRecordInteraction,
}: BriefingCardProps) {
  const { t } = useTranslation();
  const source = item.source_type || 'hackernews';
  const colorClass = getSourceColorClass(source);
  const label = getSourceLabel(source);

  return (
    <div className="bg-bg-tertiary rounded-lg border border-border p-4 hover:border-[#3A3A3A] transition-all">
      <div className="flex items-start gap-3">
        {/* Source badge + score */}
        <div className="flex flex-col items-center gap-1 flex-shrink-0">
          <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${colorClass}`}>
            {label}
          </span>
          {(() => {
            const badge = getContentTypeBadge(item.score_breakdown?.content_type);
            return badge ? (
              <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${badge.colorClass}`}>
                {badge.label}
              </span>
            ) : null;
          })()}
          <span className={`text-xs font-mono font-medium ${getScoreColor(item.top_score)}`}>
            {formatScore(item.top_score)}
          </span>
        </div>

        {/* Content */}
        <div className="flex-1 min-w-0">
          {item.url ? (
            <button
              onClick={() => {
                window.open(item.url!, '_blank', 'noopener,noreferrer');
                onRecordInteraction?.(item);
              }}
              className="text-sm text-white hover:text-orange-400 hover:underline decoration-orange-400/50 text-left font-medium transition-colors"
            >
              {item.title}
            </button>
          ) : (
            <p className="text-sm text-white font-medium">{item.title}</p>
          )}

          {(explanation || item.explanation) && (
            <p className="text-xs text-gray-400 mt-1.5 leading-relaxed">
              {explanation || item.explanation}
            </p>
          )}
        </div>

        {/* Actions */}
        <div className="flex items-center gap-1.5 flex-shrink-0">
          {feedbackGiven ? (
            <span className={`text-xs px-2 py-1 rounded ${
              feedbackGiven === 'save' ? 'bg-green-500/20 text-green-400' : 'bg-gray-500/20 text-gray-500'
            }`}>
              {feedbackGiven === 'save' ? t('feedback.saved') : t('feedback.dismissed')}
            </span>
          ) : (
            <>
              {onSave && (
                <button
                  onClick={() => onSave(item)}
                  className="px-2.5 py-1.5 text-xs bg-green-500/10 text-green-400 border border-green-500/20 rounded-lg hover:bg-green-500/20 transition-all font-medium"
                >
                  {t('action.save')}
                </button>
              )}
              {onDismiss && (
                <button
                  onClick={() => onDismiss(item)}
                  className="px-2.5 py-1.5 text-xs bg-border text-gray-500 border border-[#333] rounded-lg hover:bg-red-500/10 hover:text-red-400 hover:border-red-500/20 transition-all font-medium"
                >
                  {t('action.dismiss')}
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
});
