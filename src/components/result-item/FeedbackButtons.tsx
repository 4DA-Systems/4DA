// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance, FeedbackAction } from '../../types';
import { isSafeUrl } from '../../utils/sanitize-html';
import { useAppStore } from '../../store';
import { recordTrustEvent } from '../../lib/trust-feedback';
import { cmd } from '../../lib/commands';

interface FeedbackButtonsProps {
  item: SourceRelevance;
  feedback: FeedbackAction | undefined;
  onRecordInteraction: (itemId: number, actionType: FeedbackAction, item: SourceRelevance) => void;
  sessionSaveCount?: number;
}

export const FeedbackButtons = memo(function FeedbackButtons({ item, feedback, onRecordInteraction, sessionSaveCount }: FeedbackButtonsProps) {
  const { t } = useTranslation();
  const [savePulse, setSavePulse] = useState(false);
  const [dismissFlash, setDismissFlash] = useState(false);
  const [moreOpen, setMoreOpen] = useState(false);
  const moreRef = useRef<HTMLDivElement>(null);

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

  useEffect(() => {
    if (!dismissFlash) return;
    const timeout = setTimeout(() => setDismissFlash(false), 400);
    return () => clearTimeout(timeout);
  }, [dismissFlash]);

  useEffect(() => {
    if (!moreOpen) return;
    const close = (e: MouseEvent) => {
      if (moreRef.current && !moreRef.current.contains(e.target as Node)) {
        setMoreOpen(false);
      }
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, [moreOpen]);

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
    setMoreOpen(false);
    onRecordInteraction(item.id, 'dismiss', item);
    recordTrustEvent({
      eventType: 'dismissed',
      signalId: String(item.id),
      sourceType: item.source_type || undefined,
      topic: item.title,
      notes: 'dismiss',
    });
  }, [item, onRecordInteraction]);

  const handleSnooze = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onRecordInteraction(item.id, 'snooze', item);
    cmd('snooze_item', { sourceItemId: item.id, days: 7 }).catch(() => {});
    recordTrustEvent({
      eventType: 'dismissed',
      signalId: String(item.id),
      sourceType: item.source_type || undefined,
      topic: item.title,
      notes: 'snoozed_7d',
    });
  }, [item, onRecordInteraction]);

  const extractTopic = useCallback((): string => {
    const deps = item.score_breakdown?.matched_deps;
    if (deps && deps.length > 0) return deps[0]!;
    const triggers = item.signal_triggers;
    if (triggers && triggers.length > 0) return triggers[0]!;
    const words = item.title.toLowerCase().split(/\s+/);
    return words.find(w => w.length > 3 && !/^(this|that|from|with|have|been|will|what|when|where|about|into|your|more|some)$/.test(w)) || words[0] || 'unknown';
  }, [item]);

  const handleSuppress = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setMoreOpen(false);
    const topic = extractTopic();
    void cmd('add_exclusion', { topic }).then(() => {
      useAppStore.getState().addToast('success', t('feedback.topicSuppressed', { topic }));
      void useAppStore.getState().loadLearnedBehavior();
    }).catch(() => {
      useAppStore.getState().addToast('warning', t('feedback.suppressFailed'));
    });
    recordTrustEvent({
      eventType: 'false_positive',
      signalId: String(item.id),
      sourceType: item.source_type || undefined,
      topic: item.title,
      notes: `suppress_topic:${topic}`,
    });
  }, [item, t, extractTopic]);

  const handleWatch = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setMoreOpen(false);
    const topic = extractTopic();
    void cmd('watch_item', { sourceItemId: item.id, topic, title: item.title }).then(() => {
      useAppStore.getState().addToast('success', t('feedback.watchAdded', { topic }));
    }).catch(() => {
      useAppStore.getState().addToast('warning', t('feedback.watchFailed'));
    });
    recordTrustEvent({
      eventType: 'acted_on',
      signalId: String(item.id),
      sourceType: item.source_type || undefined,
      topic: item.title,
      notes: `watch_topic:${topic}`,
    });
  }, [item, t, extractTopic]);

  const handleShare = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setMoreOpen(false);
    const scorePercent = Math.round((item.top_score ?? 0) * 100);
    const text = `${item.title} — Scored ${scorePercent}% by 4DA (https://4da.ai)`;
    navigator.clipboard.writeText(text).then(() => {
      useAppStore.getState().addToast('success', t('feedback.shareCopied'));
    }).catch(() => {});
  }, [item.title, item.top_score, t]);

  return (
    <div className="flex items-center gap-2 mb-3" role="group" aria-label={t('feedback.actions')}>
      {/* Primary actions */}
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
        {feedback === 'save' ? `✓ ${t('feedback.saved')}` : t('action.save')}
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
        onClick={handleSnooze}
        disabled={!!feedback}
        aria-label={feedback === 'snooze' ? t('feedback.snoozed') : t('action.snooze')}
        className={`px-3 py-1.5 text-xs rounded font-medium transition-colors ${
          feedback === 'snooze'
            ? 'bg-amber-500/20 text-amber-400 cursor-default'
            : feedback
            ? 'bg-bg-tertiary text-text-muted cursor-not-allowed'
            : 'bg-amber-500/10 text-amber-400/80 hover:bg-amber-500/20 hover:text-amber-400'
        }`}
      >
        {feedback === 'snooze' ? `⏰ ${t('feedback.snoozed')}` : t('action.snooze')}
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
        {feedback === 'mark_irrelevant' ? `⊘ ${t('feedback.marked')}` : t('feedback.notRelevant')}
      </button>

      {/* Overflow menu — secondary actions */}
      <div className="relative" ref={moreRef}>
        <button
          onClick={(e) => { e.stopPropagation(); setMoreOpen(v => !v); }}
          aria-label={t('feedback.moreActions', 'More actions')}
          aria-expanded={moreOpen}
          className="px-2 py-1.5 text-xs rounded font-medium bg-bg-tertiary text-text-muted hover:bg-border hover:text-text-secondary transition-colors"
        >
          &middot;&middot;&middot;
        </button>
        {moreOpen && (
          <div className="absolute bottom-full mb-1 end-0 z-50 min-w-[140px] py-1 bg-bg-secondary border border-border rounded-lg shadow-lg">
            <button
              onClick={handleDismiss}
              disabled={!!feedback}
              className={`w-full text-start px-3 py-1.5 text-xs hover:bg-bg-tertiary transition-colors ${
                dismissFlash ? 'opacity-50' : ''
              } ${
                feedback === 'dismiss'
                  ? 'text-text-muted'
                  : feedback
                  ? 'text-text-muted opacity-40'
                  : 'text-text-secondary'
              }`}
            >
              {feedback === 'dismiss' ? `✗ ${t('feedback.dismissed')}` : t('action.dismiss')}
            </button>
            <button
              onClick={handleSuppress}
              disabled={!!feedback}
              className="w-full text-start px-3 py-1.5 text-xs text-purple-400 hover:bg-bg-tertiary transition-colors disabled:opacity-40"
            >
              {t('feedback.suppressTopic')}
            </button>
            <button
              onClick={handleWatch}
              disabled={!!feedback}
              className="w-full text-start px-3 py-1.5 text-xs text-cyan-400 hover:bg-bg-tertiary transition-colors disabled:opacity-40"
            >
              {t('feedback.watchItem')}
            </button>
            <button
              onClick={handleShare}
              className="w-full text-start px-3 py-1.5 text-xs text-blue-400 hover:bg-bg-tertiary transition-colors"
            >
              {t('action.share')}
            </button>
          </div>
        )}
      </div>
    </div>
  );
});
