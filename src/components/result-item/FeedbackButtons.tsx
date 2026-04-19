// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance, FeedbackAction } from '../../types';
import { isSafeUrl } from '../../utils/sanitize-html';
import { useAppStore } from '../../store';
import { recordTrustEvent } from '../../lib/trust-feedback';

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
    recordTrustEvent({
      eventType: 'acted_on',
      signalId: String(item.id),
      sourceType: item.source_type || undefined,
      topic: item.title,
      notes: 'save',
    });
  }, [item, onRecordInteraction]);

  const handleDismiss = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setDismissFlash(true);
    onRecordInteraction(item.id, 'dismiss', item);
    recordTrustEvent({
      eventType: 'dismissed',
      signalId: String(item.id),
      sourceType: item.source_type || undefined,
      topic: item.title,
      notes: 'dismiss',
    });
  }, [item, onRecordInteraction]);

  const handleShare = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    const scorePercent = Math.round((item.top_score ?? 0) * 100);
    const text = `${item.title} — Scored ${scorePercent}% by 4DA (https://4da.ai)`;
    navigator.clipboard.writeText(text).then(() => {
      useAppStore.getState().addToast('success', t('feedback.shareCopied'));
    }).catch(() => {
      // Clipboard write failed silently
    });
  }, [item.title, item.top_score, t]);

  return (
    <div className="flex gap-2 mb-3" role="group" aria-label={t('feedback.actions')}>
      {item.url && isSafeUrl(item.url) && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            onRecordInteraction(item.id, 'click', item);
            recordTrustEvent({
              eventType: 'acted_on',
              signalId: String(item.id),
              sourceType: item.source_type || undefined,
              topic: item.title,
              notes: 'open_link',
            });
            import('@tauri-apps/plugin-opener').then(({ openUrl }) => {
              void openUrl(item.url!);
            }).catch(() => {
              window.open(item.url!, '_blank', 'noopener,noreferrer');
            });
          }}
          aria-label={`${t('feedback.openLink')}: ${item.title}`}
          className="px-3 py-1.5 text-xs bg-accent-primary text-bg-primary rounded hover:bg-text-secondary transition-colors font-medium cursor-pointer"
        >
          {t('feedback.openLink')}
        </button>
      )}
      <button
        onClick={handleSave}
        disabled={!!feedback}
        aria-label={feedback === 'save' ? t('feedback.saved') : t('action.save')}
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
            className="absolute -top-1.5 -end-1.5 min-w-[16px] h-4 px-1 text-[10px] leading-4 text-center bg-success text-white rounded-full font-mono"
            aria-label={t('feedback.sessionSaves', { count: sessionSaveCount })}
          >
            {sessionSaveCount}
          </span>
        )}
      </button>
      <button
        onClick={handleDismiss}
        disabled={!!feedback}
        aria-label={feedback === 'dismiss' ? t('feedback.dismissed') : t('action.dismiss')}
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
          recordTrustEvent({
            eventType: 'false_positive',
            signalId: String(item.id),
            sourceType: item.source_type || undefined,
            topic: item.title,
            notes: 'mark_irrelevant',
          });
        }}
        disabled={!!feedback}
        aria-label={feedback === 'mark_irrelevant' ? t('feedback.marked') : t('feedback.notRelevant')}
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
      <button
        onClick={handleShare}
        aria-label={`${t('action.share')}: ${item.title}`}
        className="px-2.5 py-1 text-xs rounded font-medium bg-blue-500/10 text-blue-400 hover:bg-blue-500/20 transition-colors"
      >
        {t('action.share')}
      </button>
    </div>
  );
});
