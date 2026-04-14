import { memo, useCallback, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { getSourceLabel, getSourceColorClass } from '../../config/sources';
import { formatScore } from '../../utils/score';
import { isSafeUrl } from '../../utils/sanitize-html';
import { useFourdaComponent } from '../../hooks/use-fourda-component';
import { useTranslatedContent } from '../ContentTranslationProvider';
import type { SourceRelevance, FeedbackAction } from '../../types';

interface AttentionCardsProps {
  signalItems: SourceRelevance[];
  topItems: SourceRelevance[];
  feedbackGiven: Record<number, FeedbackAction>;
  onSave: (item: SourceRelevance) => void;
  onDismiss: (item: SourceRelevance) => void;
  onRecordClick: (item: SourceRelevance) => void;
}

const PRIORITY_STYLES: Record<string, { border: string; dot: string }> = {
  critical: { border: 'border-red-500/30', dot: 'bg-red-400' },
  alert: { border: 'border-amber-500/30', dot: 'bg-amber-400' },
};

/**
 * Zone 2: Attention — 3-5 cards max, horizontally scrollable.
 * Answers: "What needs my attention?"
 * Shows signal items first, then top-scoring items to fill up to 5.
 */
export const AttentionCards = memo(function AttentionCards({
  signalItems,
  topItems,
  feedbackGiven,
  onSave,
  onDismiss,
  onRecordClick,
}: AttentionCardsProps) {
  // Merge signals + top picks, max 5, sorted by necessity score (highest first)
  const items = useMemo(() => {
    const merged = [...signalItems, ...topItems.slice(0, Math.max(0, 5 - signalItems.length))];
    return merged.sort((a, b) => {
      const aNecessity = a.score_breakdown?.necessity_score ?? 0;
      const bNecessity = b.score_breakdown?.necessity_score ?? 0;
      if (aNecessity !== bNecessity) return bNecessity - aNecessity;
      return 0; // preserve existing order as tiebreaker
    });
  }, [signalItems, topItems]);

  if (items.length === 0) return null;

  return (
    <div className="px-1">
      <div className="flex gap-3 overflow-x-auto pb-2 scrollbar-thin scrollbar-thumb-border scrollbar-track-transparent">
        {items.map(item => (
          <AttentionCard
            key={item.id}
            item={item}
            isSignal={signalItems.some(s => s.id === item.id)}
            feedback={feedbackGiven[item.id]}
            onSave={onSave}
            onDismiss={onDismiss}
            onRecordClick={onRecordClick}
          />
        ))}
      </div>
    </div>
  );
});

interface AttentionCardProps {
  item: SourceRelevance;
  isSignal: boolean;
  feedback: FeedbackAction | undefined;
  onSave: (item: SourceRelevance) => void;
  onDismiss: (item: SourceRelevance) => void;
  onRecordClick: (item: SourceRelevance) => void;
}

const AttentionCard = memo(function AttentionCard({
  item,
  isSignal,
  feedback,
  onSave,
  onDismiss,
  onRecordClick,
}: AttentionCardProps) {
  const { t } = useTranslation();
  const { getTranslated } = useTranslatedContent();
  const displayTitle = getTranslated(String(item.id), item.title);
  const necessityScore = item.score_breakdown?.necessity_score ?? 0;
  const necessityCategory = item.score_breakdown?.necessity_category;

  // Priority from necessity: security vulnerabilities get critical styling,
  // other high-necessity items get alert styling, otherwise fall back to signal_priority
  const priority = necessityCategory === 'security_vulnerability'
    ? 'critical'
    : necessityScore > 0.60
      ? 'alert'
      : item.signal_priority || 'alert';
  const style = (PRIORITY_STYLES[priority] ?? PRIORITY_STYLES.alert)!;
  const source = item.source_type || 'hackernews';

  // GAME score fingerprint — unique visual identity per item
  const { containerRef: fpRef, elementRef: fpElRef } = useFourdaComponent('fourda-score-fingerprint');

  useEffect(() => {
    const el = fpElRef.current;
    if (!el) return;
    el.setParam?.('relevance', item.score_breakdown?.context_score ?? 0);
    el.setParam?.('freshness', item.score_breakdown?.freshness_mult ?? 0.5);
    el.setParam?.('depth', item.score_breakdown?.content_quality_mult ?? 0.5);
    el.setParam?.('confidence', item.confidence ?? 0.5);
  }, [item, fpElRef]);

  const handleOpen = useCallback(() => {
    onRecordClick(item);
    if (item.url && isSafeUrl(item.url)) {
      import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl(item.url!)).catch(() => {
        window.open(item.url!, '_blank', 'noopener,noreferrer');
      });
    }
  }, [item, onRecordClick]);

  if (feedback) {
    return null; // Already acted on — remove from attention
  }

  const hasNecessityHighlight = isSignal || necessityScore > 0.60 || necessityCategory === 'security_vulnerability';

  return (
    <div
      className={`flex-shrink-0 w-72 rounded-lg border ${
        hasNecessityHighlight ? style.border : 'border-border'
      } bg-bg-secondary p-4 flex flex-col gap-3 hover:border-white/20 transition-colors`}
    >
      {/* Header: signal badge + source + score */}
      <div className="flex items-center gap-2">
        {hasNecessityHighlight && (
          <span className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${style.dot}`} />
        )}
        <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${getSourceColorClass(source)}`}>
          {getSourceLabel(source)}
        </span>
        <div ref={fpRef} className="w-6 h-6 rounded flex-shrink-0 ms-auto" aria-hidden="true" />
        <span className="text-xs font-mono text-text-muted">
          {formatScore(item.top_score)}
        </span>
      </div>

      {/* Title */}
      <button
        onClick={handleOpen}
        className="text-sm text-white text-start leading-snug line-clamp-2 hover:text-orange-400 transition-colors"
      >
        {isSignal && item.signal_action ? item.signal_action : displayTitle}
      </button>

      {/* Why — single line */}
      {item.explanation && (
        <p className="text-xs text-text-muted line-clamp-1">{item.explanation}</p>
      )}

      {/* Actions */}
      <div className="flex items-center gap-2 mt-auto">
        {item.url && isSafeUrl(item.url) && (
          <button
            onClick={handleOpen}
            className="px-2.5 py-1 text-xs bg-bg-tertiary text-text-secondary border border-border rounded hover:bg-border transition-all"
          >
            {t('briefing.read', 'Read')}
          </button>
        )}
        <button
          onClick={() => onSave(item)}
          aria-label={t('action.save', 'Save') + ': ' + item.title}
          className="px-2.5 py-1 text-xs bg-green-500/10 text-green-400 border border-green-500/20 rounded hover:bg-green-500/20 transition-all"
        >
          {t('action.save', 'Save')}
        </button>
        <button
          onClick={() => onDismiss(item)}
          aria-label={t('action.dismiss', 'Dismiss') + ': ' + item.title}
          className="px-2.5 py-1 text-xs text-text-muted border border-border rounded hover:bg-red-500/10 hover:text-red-400 hover:border-red-500/20 transition-all ms-auto"
        >
          {t('action.dismiss', 'Dismiss')}
        </button>
      </div>
    </div>
  );
});
