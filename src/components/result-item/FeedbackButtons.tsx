import { memo, useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance, FeedbackAction } from '../../types';

interface FeedbackButtonsProps {
  item: SourceRelevance;
  feedback: FeedbackAction | undefined;
  onRecordInteraction: (itemId: number, actionType: FeedbackAction, item: SourceRelevance) => void;
  /** Optional: session save count to display as badge on save button */
  sessionSaveCount?: number;
}

export const FeedbackButtons = memo(function FeedbackButtons({ item, feedback, onRecordInteraction, sessionSaveCount }: FeedbackButtonsProps) {
  const { t } = useTranslation();
  const [savePulse, setSavePulse] = useState(false);
  const [dismissFlash, setDismissFlash] = useState(false);

  // Clear save pulse after 600ms — uses rAF to ensure the glow renders before starting the timer
  useEffect(() => {
    if (!savePulse) return;
    let timeout: ReturnType<typeof setTimeout>;
    const rafId = requestAnimationFrame(() => {
      timeout = setTimeout(() => setSavePulse(false), 600);
    });
    return () => {
      cancelAnimationFrame(rafId);
      clearTimeout(timeout);
    };
  }, [savePulse]);

  // Clear dismiss flash after 400ms
  useEffect(() => {
    if (!dismissFlash) return;
    const timeout = setTimeout(() => setDismissFlash(false), 400);
    return () => clearTimeout(timeout);
  }, [dismissFlash]);

  const handleSave = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setSavePulse(true);
    onRecordInteraction(item.id, 'save', item);
  }, [item, onRecordInteraction]);

  const handleDismiss = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setDismissFlash(true);
    onRecordInteraction(item.id, 'dismiss', item);
  }, [item, onRecordInteraction]);

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
        onClick={handleSave}
        disabled={!!feedback}
        className={`px-3 py-1.5 text-xs rounded font-medium relative transition-all duration-200 ${
          feedback === 'save'
            ? 'bg-success/20 text-success cursor-default'
            : feedback
            ? 'bg-bg-tertiary text-text-muted cursor-not-allowed'
            : 'bg-success/20 text-success hover:bg-success/30'
        }`}
        style={savePulse ? {
          boxShadow: '0 0 12px rgba(34, 197, 94, 0.5)',
          transition: 'box-shadow 0.6s ease-out',
        } : {
          boxShadow: 'none',
          transition: 'box-shadow 0.6s ease-out',
        }}
      >
        {feedback === 'save' ? `\u2713 ${t('feedback.saved')}` : t('action.save')}
        {sessionSaveCount != null && sessionSaveCount > 0 && (
          <span
            className="absolute -top-1.5 -right-1.5 min-w-[16px] h-4 px-1 text-[10px] leading-4 text-center bg-success text-white rounded-full font-mono"
            aria-label={t('feedback.sessionSaves', { count: sessionSaveCount, defaultValue: '{{count}} saved this session' })}
          >
            {sessionSaveCount}
          </span>
        )}
      </button>
      <button
        onClick={handleDismiss}
        disabled={!!feedback}
        className={`px-3 py-1.5 text-xs rounded font-medium transition-all duration-200 ${
          dismissFlash
            ? 'opacity-50'
            : ''
        } ${
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
