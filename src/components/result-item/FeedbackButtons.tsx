import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance, FeedbackAction } from '../../types';

interface FeedbackButtonsProps {
  item: SourceRelevance;
  feedback: FeedbackAction | undefined;
  onRecordInteraction: (itemId: number, actionType: FeedbackAction, item: SourceRelevance) => void;
}

export const FeedbackButtons = memo(function FeedbackButtons({ item, feedback, onRecordInteraction }: FeedbackButtonsProps) {
  const { t } = useTranslation();
  return (
    <div className="flex gap-2 mb-3" role="group" aria-label={t('feedback.actions', { defaultValue: 'Feedback actions' })}>
      {item.url && (
        <a
          href={item.url}
          target="_blank"
          rel="noopener noreferrer"
          onClick={(e) => {
            e.stopPropagation();
            onRecordInteraction(item.id, 'click', item);
          }}
          className="px-3 py-1.5 text-xs bg-accent-primary text-bg-primary rounded hover:bg-text-secondary transition-colors font-medium"
        >
          {t('feedback.openLink')}
        </a>
      )}
      <button
        onClick={(e) => {
          e.stopPropagation();
          onRecordInteraction(item.id, 'save', item);
        }}
        disabled={!!feedback}
        className={`px-3 py-1.5 text-xs rounded transition-colors font-medium ${
          feedback === 'save'
            ? 'bg-success/20 text-success cursor-default'
            : feedback
            ? 'bg-bg-tertiary text-text-muted cursor-not-allowed'
            : 'bg-success/20 text-success hover:bg-success/30'
        }`}
      >
        {feedback === 'save' ? `\u2713 ${t('feedback.saved')}` : t('action.save')}
      </button>
      <button
        onClick={(e) => {
          e.stopPropagation();
          onRecordInteraction(item.id, 'dismiss', item);
        }}
        disabled={!!feedback}
        className={`px-3 py-1.5 text-xs rounded transition-colors font-medium ${
          feedback === 'dismiss'
            ? 'bg-text-muted/20 text-text-muted cursor-default'
            : feedback
            ? 'bg-bg-tertiary text-text-muted cursor-not-allowed'
            : 'bg-bg-tertiary text-text-secondary hover:bg-border'
        }`}
      >
        {feedback === 'dismiss' ? `\u2717 ${t('feedback.dismissed')}` : t('action.dismiss')}
      </button>
      <button
        onClick={(e) => {
          e.stopPropagation();
          onRecordInteraction(item.id, 'mark_irrelevant', item);
        }}
        disabled={!!feedback}
        className={`px-3 py-1.5 text-xs rounded transition-colors font-medium ${
          feedback === 'mark_irrelevant'
            ? 'bg-error/20 text-error cursor-default'
            : feedback
            ? 'bg-bg-tertiary text-text-muted cursor-not-allowed'
            : 'bg-error/10 text-error/80 hover:bg-error/20 hover:text-error'
        }`}
      >
        {feedback === 'mark_irrelevant' ? `\u2298 ${t('feedback.marked')}` : t('feedback.notRelevant')}
      </button>
    </div>
  );
});
