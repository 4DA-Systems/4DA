import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance, FeedbackAction } from '../../types';
import { formatScore, getScoreColor, formatRelativeAge, getScoreFactorKeys } from '../../utils/score';
import { getSourceLabel, getSourceColorClass } from '../../config/sources';
import { isSafeUrl } from '../../utils/sanitize-html';
import { BadgeRow } from './BadgeRow';
import { ProInsightRow } from './ProInsightRow';

interface ResultItemCollapsedProps {
  item: SourceRelevance;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onToggleBreakdown?: () => void;
  showBreakdown?: boolean;
  feedback: FeedbackAction | undefined;
  fallbackReason: string;
}

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
  const scoreTooltip = useMemo(() => {
    const keys = getScoreFactorKeys(item);
    if (keys.length === 0) return undefined;
    return keys.map(k => t(k)).join('\n');
  }, [item, t]);
  return (
    <div className="w-full px-4 py-3">
      <div className="flex items-start gap-3">
        {/* Score Badge + Source */}
        <div className="flex-shrink-0 flex flex-col items-center gap-1">
          {/* Score — click to toggle breakdown (if available) or expand */}
          <button
            onClick={onToggleBreakdown && item.score_breakdown ? onToggleBreakdown : onToggleExpand}
            aria-expanded={showBreakdown}
            aria-label={item.score_breakdown ? t('scoreDrawer.toggle', 'Toggle score breakdown') : undefined}
            title={scoreTooltip}
            className={`w-14 text-center py-1 rounded font-mono text-sm font-medium cursor-pointer transition-all ${getScoreColor(
              item.top_score,
            )} ${showBreakdown ? 'ring-1 ring-white/30' : ''} ${item.score_breakdown ? 'hover:ring-1 hover:ring-white/20' : ''}`}
          >
            {formatScore(item.top_score)}
          </button>
          {/* Source Badge — click to expand */}
          <button
            onClick={onToggleExpand}
            aria-expanded={isExpanded}
            aria-controls={`result-detail-${item.id}`}
            aria-label={`Source: ${getSourceLabel(item.source_type || '') || item.source_type || t('results.unknownSource')}`}
            className={`text-[10px] px-1.5 py-0.5 rounded font-medium cursor-pointer ${getSourceColorClass(item.source_type || '')}`}
          >
            {getSourceLabel(item.source_type || '') || item.source_type || t('results.unknownSource')}
          </button>
        </div>

        {/* Title and URL — click title to open link */}
        <div className="flex-1 min-w-0">
          <div className="flex items-start gap-2">
            {item.url && isSafeUrl(item.url) ? (
              <a
                href={item.url}
                target="_blank"
                rel="noopener noreferrer"
                onClick={(e) => e.stopPropagation()}
                className={`text-sm flex-1 hover:underline decoration-gray-600 ${
                  item.relevant ? 'text-text-primary' : 'text-text-secondary'
                }`}
              >
                {item.title}
              </a>
            ) : (
              <button
                onClick={onToggleExpand}
                aria-label={`Expand details: ${item.title}`}
                className={`text-sm flex-1 text-left ${
                  item.relevant ? 'text-text-primary' : 'text-text-secondary'
                }`}
              >
                {item.title}
              </button>
            )}
            <BadgeRow item={item} />
          </div>
          {(item.url || item.created_at) && (
            <div className="flex items-center gap-2 text-xs text-text-muted mt-1">
              {item.url && <span className="truncate font-mono">{item.url}</span>}
              {item.created_at && (
                <span className="flex-shrink-0 text-text-muted/70" title={new Date(item.created_at).toLocaleString()}>
                  {formatRelativeAge(item.created_at)}
                </span>
              )}
            </div>
          )}
          {(item.similar_count ?? 0) > 0 && (
            <details className="mt-0.5 group">
              <summary className="text-[10px] text-text-muted cursor-pointer hover:text-text-secondary select-none list-none flex items-center gap-1">
                <span className="text-[10px] text-text-muted group-open:rotate-90 transition-transform">&#9654;</span>
                {t('results.relatedArticles', { count: item.similar_count })}
              </summary>
              {item.similar_titles && item.similar_titles.length > 0 && (
                <ul className="mt-1 ml-3 space-y-0.5">
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

        {/* Feedback Indicators */}
        {feedback && (
          <div
            className={`text-xs px-2 py-0.5 rounded ${
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
          </div>
        )}

        {/* Expand Button */}
        <button
          onClick={onToggleExpand}
          aria-expanded={isExpanded}
          aria-controls={`result-detail-${item.id}`}
          aria-label={isExpanded ? t('results.collapseDetails') : t('results.expandDetails')}
          className="text-text-muted text-xs hover:text-text-secondary transition-colors px-1"
        >
          {isExpanded ? '\u2212' : '+'}
        </button>
      </div>

      {/* Why This Matters - Preview (shown when not expanded) */}
      {!isExpanded && (
        <>
          <button onClick={onToggleExpand} aria-label="Show full explanation" className="w-full text-left">
            <div className="mt-1.5 text-xs text-text-secondary pl-[4.25rem]">
              {item.explanation || fallbackReason}
            </div>
          </button>
          {/* Wisdom annotation — shows when context score is high (AWE wisdom chunks contributed) */}
          {item.score_breakdown && item.score_breakdown.context_score > 0.3 && item.relevant && (
            <div className="mt-1 text-[10px] text-text-muted pl-[4.25rem] flex items-center gap-1">
              <span className="w-1 h-1 rounded-full bg-success/60 inline-block" />
              Matches your experience
            </div>
          )}
          <ProInsightRow item={item} />
        </>
      )}
    </div>
  );
});
