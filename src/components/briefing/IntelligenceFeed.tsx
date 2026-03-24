import { memo, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { formatScore, getScoreColor, formatRelativeAge } from '../../utils/score';
import { getSourceLabel, getSourceColorClass } from '../../config/sources';
import { isSafeUrl } from '../../utils/sanitize-html';
import type { SourceRelevance, FeedbackAction } from '../../types';

interface IntelligenceFeedProps {
  results: SourceRelevance[];
  feedbackGiven: Record<number, FeedbackAction>;
  signalIds: Set<number>;
  onSave: (item: SourceRelevance) => void;
  onDismiss: (item: SourceRelevance) => void;
  onRecordClick: (item: SourceRelevance) => void;
  onViewAll: () => void;
}

/**
 * Zone 3: The Feed — Clean, compact content items.
 * Shows top 15 relevant items (excluding signals already shown in Zone 2).
 * Ultra-compact: title + source + score. Expand on click.
 */
export const IntelligenceFeed = memo(function IntelligenceFeed({
  results,
  feedbackGiven,
  signalIds,
  onSave,
  onDismiss,
  onRecordClick,
  onViewAll,
}: IntelligenceFeedProps) {
  const { t } = useTranslation();

  // Top 15 relevant items, excluding signal items already in Zone 2
  const feedItems = useMemo(() => {
    return results
      .filter(r => r.relevant && !signalIds.has(r.id))
      .slice(0, 15);
  }, [results, signalIds]);

  const totalRelevant = useMemo(() => {
    return results.filter(r => r.relevant).length;
  }, [results]);

  if (feedItems.length === 0) return null;

  return (
    <div>
      <div className="flex items-center justify-between mb-3 px-1">
        <h3 className="text-xs font-medium text-text-muted uppercase tracking-wider">
          {t('feed.title', 'Signal Stream')}
        </h3>
        {totalRelevant > 15 && (
          <button
            onClick={onViewAll}
            className="text-xs text-text-muted hover:text-orange-400 transition-colors"
          >
            {t('feed.viewAll', 'View all {{count}}', { count: totalRelevant })}
          </button>
        )}
      </div>

      <div className="bg-bg-secondary rounded-lg border border-border divide-y divide-border/50 overflow-hidden">
        {feedItems.map(item => (
          <FeedItem
            key={item.id}
            item={item}
            feedback={feedbackGiven[item.id]}
            onSave={onSave}
            onDismiss={onDismiss}
            onRecordClick={onRecordClick}
          />
        ))}
      </div>

      {totalRelevant > 15 && (
        <div className="flex justify-center mt-4">
          <button
            onClick={onViewAll}
            className="px-5 py-2 text-sm text-orange-400 bg-bg-secondary border border-orange-500/20 rounded-lg hover:bg-orange-500/10 hover:border-orange-500/30 transition-all font-medium"
          >
            {t('briefing.viewAllResults', 'View all {{count}} results', { count: totalRelevant })}
          </button>
        </div>
      )}
    </div>
  );
});

interface FeedItemProps {
  item: SourceRelevance;
  feedback: FeedbackAction | undefined;
  onSave: (item: SourceRelevance) => void;
  onDismiss: (item: SourceRelevance) => void;
  onRecordClick: (item: SourceRelevance) => void;
}

const FeedItem = memo(function FeedItem({
  item,
  feedback,
  onSave,
  onDismiss,
  onRecordClick,
}: FeedItemProps) {
  const { t } = useTranslation();
  const source = item.source_type || 'hackernews';

  const handleClick = useCallback(() => {
    onRecordClick(item);
    if (item.url && isSafeUrl(item.url)) {
      window.open(item.url, '_blank', 'noopener,noreferrer');
    }
  }, [item, onRecordClick]);

  const hoverReason = useMemo(() => {
    const parts: string[] = [];
    if (item.explanation) return item.explanation;
    const b = item.score_breakdown;
    if (b) {
      if (b.context_score > 0.3) parts.push('Matches your context');
      if (b.matched_deps?.length) parts.push(`Affects ${b.matched_deps.slice(0, 2).join(', ')}`);
      if (b.interest_score > 0.3) parts.push('Interest match');
      if (b.ace_boost > 0.1) parts.push('Active in recent work');
    }
    if (item.source_type) parts.push(getSourceLabel(item.source_type));
    return parts.join(' \u00b7 ') || item.title;
  }, [item]);

  // Dim if already acted on
  const dimmed = feedback === 'dismiss' || feedback === 'mark_irrelevant';

  return (
    <div title={hoverReason} className={`group flex items-center gap-3 px-4 py-2.5 hover:bg-white/[0.02] transition-colors ${
      dimmed ? 'opacity-40' : ''
    }`}>
      {/* Score */}
      <span className={`flex-shrink-0 text-xs font-mono font-medium w-10 text-end ${getScoreColor(item.top_score)}`}>
        {formatScore(item.top_score)}
      </span>

      {/* Source badge */}
      <span className={`flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium ${getSourceColorClass(source)}`}>
        {getSourceLabel(source)}
      </span>

      {/* Title */}
      <button
        onClick={handleClick}
        className="flex-1 min-w-0 text-sm text-start text-text-primary truncate hover:text-orange-400 transition-colors"
      >
        {item.title}
      </button>

      {/* Age */}
      {item.created_at && (
        <span className="flex-shrink-0 text-[10px] text-text-muted/60 hidden sm:block">
          {formatRelativeAge(item.created_at)}
        </span>
      )}

      {/* Signal indicator */}
      {item.signal_type && (
        <span
          className={`flex-shrink-0 w-1.5 h-1.5 rounded-full ${
            item.signal_priority === 'critical' ? 'bg-red-400' :
            item.signal_priority === 'alert' ? 'bg-orange-400' :
            item.signal_priority === 'advisory' ? 'bg-amber-400' :
            'bg-blue-400'
          }`}
          role="img"
          aria-label={item.signal_type || 'Normal'}
          title={item.signal_type}
        />
      )}

      {/* Hover actions */}
      {!feedback && (
        <div className="flex-shrink-0 flex items-center gap-1 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity">
          <button
            onClick={(e) => { e.stopPropagation(); onSave(item); }}
            className="px-1.5 py-0.5 text-[10px] text-green-400 hover:bg-green-500/10 rounded transition-colors"
            title={t('action.save', 'Save')}
          >
            {t('action.save', 'Save')}
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); onDismiss(item); }}
            className="px-1.5 py-0.5 text-[10px] text-text-muted hover:text-red-400 hover:bg-red-500/10 rounded transition-colors"
            title={t('action.dismiss', 'Dismiss')}
          >
            {t('action.dismiss', 'Dismiss')}
          </button>
        </div>
      )}

      {/* Feedback indicator */}
      {feedback && (
        <span className={`flex-shrink-0 text-[10px] ${
          feedback === 'save' ? 'text-green-400' : 'text-text-muted'
        }`}>
          {feedback === 'save' ? '\u2713' : '\u2717'}
        </span>
      )}
    </div>
  );
});
