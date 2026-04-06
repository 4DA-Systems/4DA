import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance, FeedbackAction } from '../../types';
import { formatScore, getScoreColor, formatRelativeAge, getScoreFactorKeys } from '../../utils/score';
import { getSourceLabel, getSourceColorClass } from '../../config/sources';
import { isSafeUrl } from '../../utils/sanitize-html';
import { formatLocalDateTime } from '../../utils/format-date';
import { useTranslatedContent } from '../ContentTranslationProvider';

interface ResultItemCollapsedProps {
  item: SourceRelevance;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onToggleBreakdown?: () => void;
  showBreakdown?: boolean;
  feedback: FeedbackAction | undefined;
  fallbackReason: string;
}

/**
 * Compact collapsed result item.
 * Default: score + source + title on one line.
 * Badges & explanations only appear on expand.
 */
export const ResultItemCollapsed = memo(function ResultItemCollapsed({
  item,
  isExpanded,
  onToggleExpand,
  onToggleBreakdown,
  showBreakdown,
  feedback,
  fallbackReason,
}: ResultItemCollapsedProps) {
  const { t } = useTranslation();
  const { getTranslated } = useTranslatedContent();
  const displayTitle = getTranslated(String(item.id), item.title);
  const scoreTooltip = useMemo(() => {
    const keys = getScoreFactorKeys(item);
    if (keys.length === 0) return undefined;
    return keys.map(k => t(k)).join('\n');
  }, [item.score_breakdown, t]);

  return (
    <div className="w-full px-4 py-2.5">
      {/* Primary row: score + source + title + age + expand */}
      <div className="flex items-center gap-3">
        {/* Score badge — click to toggle breakdown */}
        <button
          onClick={onToggleBreakdown && item.score_breakdown ? onToggleBreakdown : onToggleExpand}
          aria-expanded={showBreakdown}
          aria-label={item.score_breakdown ? `${t('scoreDrawer.toggle', 'Toggle score breakdown')}, score ${formatScore(item.top_score)}` : `Score: ${formatScore(item.top_score)}`}
          title={scoreTooltip}
          className={`flex-shrink-0 w-12 text-center py-0.5 rounded font-mono text-xs font-medium cursor-pointer transition-all ${getScoreColor(
            item.top_score,
          )} ${showBreakdown ? 'ring-1 ring-white/30' : ''} ${item.score_breakdown ? 'hover:ring-1 hover:ring-white/20' : ''}`}
        >
          {formatScore(item.top_score)}
        </button>

        {/* Source badge */}
        <span className={`flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium ${getSourceColorClass(item.source_type || '')}`}>
          {getSourceLabel(item.source_type || '') || item.source_type || t('results.unknownSource')}
        </span>

        {/* Signal dot */}
        {item.signal_type && (
          <span className={`flex-shrink-0 w-1.5 h-1.5 rounded-full ${
            item.signal_priority === 'critical' ? 'bg-red-400' :
            item.signal_priority === 'alert' ? 'bg-orange-400' :
            item.signal_priority === 'advisory' ? 'bg-amber-400' :
            'bg-blue-400'
          }`} title={item.signal_type} role="img" aria-label={`${item.signal_priority || 'normal'} priority: ${item.signal_type}`} />
        )}

        {/* Title */}
        <div className="flex-1 min-w-0">
          {item.url && isSafeUrl(item.url) ? (
            <a
              href={item.url}
              target="_blank"
              rel="noopener noreferrer"
              onClick={(e) => e.stopPropagation()}
              aria-label={`${displayTitle} (opens in new tab)`}
              className={`text-sm truncate block hover:underline decoration-gray-600 ${
                item.relevant ? 'text-text-primary' : 'text-text-secondary'
              }`}
            >
              {displayTitle}
            </a>
          ) : (
            <button
              onClick={onToggleExpand}
              aria-label={`Expand details: ${item.title}`}
              className={`text-sm truncate block text-start w-full ${
                item.relevant ? 'text-text-primary' : 'text-text-secondary'
              }`}
            >
              {displayTitle}
            </button>
          )}
        </div>

        {/* Age */}
        {item.created_at && (
          <span className="flex-shrink-0 text-[10px] text-text-muted/60" title={formatLocalDateTime(item.created_at)}>
            {formatRelativeAge(item.created_at)}
          </span>
        )}

        {/* Feedback indicator */}
        {feedback && (
          <span
            className={`flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded ${
              feedback === 'save'
                ? 'bg-success/20 text-success'
                : feedback === 'dismiss'
                ? 'bg-text-muted/20 text-text-muted'
                : 'bg-error/20 text-error'
            }`}
          >
            {feedback === 'save'
              ? `\u2713 ${t('feedback.saved')}`
              : feedback === 'dismiss'
              ? `\u2717 ${t('feedback.dismissed')}`
              : `\u2298 ${t('feedback.irrelevant')}`}
          </span>
        )}

        {/* Expand button */}
        <button
          onClick={onToggleExpand}
          aria-expanded={isExpanded}
          aria-controls={`result-detail-${item.id}`}
          aria-label={isExpanded ? t('results.collapseDetails') : t('results.expandDetails')}
          className="flex-shrink-0 text-text-muted text-xs hover:text-text-secondary transition-colors px-1"
        >
          {isExpanded ? '\u2212' : '+'}
        </button>
      </div>

      {/* Secondary row: explanation (only when expanded) */}
      {isExpanded && (
        <div className="mt-1.5 text-xs text-text-secondary ps-[3.75rem]">
          {item.explanation || fallbackReason}
        </div>
      )}

      {/* Similar items (collapsed by default, only when expanded) */}
      {isExpanded && (item.similar_count ?? 0) > 0 && (
        <details className="mt-1 ps-[3.75rem] group">
          <summary className="text-[10px] text-text-muted cursor-pointer hover:text-text-secondary select-none list-none flex items-center gap-1">
            <span className="text-[10px] text-text-muted group-open:rotate-90 transition-transform">&#9654;</span>
            {t('results.relatedArticles', { count: item.similar_count })}
          </summary>
          {item.similar_titles && item.similar_titles.length > 0 && (
            <ul className="mt-1 ms-3 space-y-0.5">
              {item.similar_titles.map((title, i) => (
                <li key={i} className="text-[10px] text-text-muted truncate">
                  {title}
                </li>
              ))}
            </ul>
          )}
        </details>
      )}
    </div>
  );
});
