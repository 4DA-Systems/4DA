import type { SourceRelevance, FeedbackAction } from '../../types';
import { formatScore, getScoreColor } from '../../utils/score';
import { getSourceLabel, getSourceColorClass } from '../../config/sources';
import { BadgeRow } from './BadgeRow';
import { ProInsightRow } from './ProInsightRow';

interface ResultItemCollapsedProps {
  item: SourceRelevance;
  isExpanded: boolean;
  onToggleExpand: () => void;
  feedback: FeedbackAction | undefined;
  fallbackReason: string;
}

export function ResultItemCollapsed({
  item,
  isExpanded,
  onToggleExpand,
  feedback,
  fallbackReason,
}: ResultItemCollapsedProps) {
  return (
    <div className="w-full px-4 py-3">
      <div className="flex items-start gap-3">
        {/* Score Badge + Source (click to expand) */}
        <button
          onClick={onToggleExpand}
          aria-expanded={isExpanded}
          aria-controls={`result-detail-${item.id}`}
          className="flex-shrink-0 flex flex-col items-center gap-1 cursor-pointer"
        >
          <div
            className={`w-14 text-center py-1 rounded font-mono text-sm font-medium ${getScoreColor(
              item.top_score,
            )}`}
          >
            {formatScore(item.top_score)}
          </div>
          {/* Source Badge */}
          <div className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${getSourceColorClass(item.source_type || '')}`}>
            {getSourceLabel(item.source_type || '') || item.source_type || 'Unknown'}
          </div>
        </button>

        {/* Title and URL — click title to open link */}
        <div className="flex-1 min-w-0">
          <div className="flex items-start gap-2">
            {item.url ? (
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
                className={`text-sm flex-1 text-left ${
                  item.relevant ? 'text-text-primary' : 'text-text-secondary'
                }`}
              >
                {item.title}
              </button>
            )}
            <BadgeRow item={item} />
          </div>
          {item.url && (
            <div className="text-xs text-text-muted truncate font-mono mt-1">
              {item.url}
            </div>
          )}
          {(item.similar_count ?? 0) > 0 && (
            <details className="mt-0.5 group">
              <summary className="text-[10px] text-gray-500 cursor-pointer hover:text-gray-400 select-none list-none flex items-center gap-1">
                <span className="text-[10px] text-gray-600 group-open:rotate-90 transition-transform">&#9654;</span>
                +{item.similar_count} related article{item.similar_count === 1 ? '' : 's'}
              </summary>
              {item.similar_titles && item.similar_titles.length > 0 && (
                <ul className="mt-1 ml-3 space-y-0.5">
                  {item.similar_titles.map((title, i) => (
                    <li key={i} className="text-[10px] text-gray-500 truncate">
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
              ? '\u2713 Saved'
              : feedback === 'dismiss'
              ? '\u2717 Dismissed'
              : '\u2298 Irrelevant'}
          </div>
        )}

        {/* Expand Button */}
        <button
          onClick={onToggleExpand}
          aria-expanded={isExpanded}
          aria-controls={`result-detail-${item.id}`}
          aria-label={isExpanded ? 'Collapse details' : 'Expand details'}
          className="text-text-muted text-xs hover:text-text-secondary transition-colors px-1"
        >
          {isExpanded ? '\u2212' : '+'}
        </button>
      </div>

      {/* Why This Matters - Preview (shown when not expanded) */}
      {!isExpanded && (
        <>
          <button onClick={onToggleExpand} className="w-full text-left">
            <div className="mt-1.5 text-xs text-text-secondary pl-[4.25rem]">
              {item.explanation || fallbackReason}
            </div>
          </button>
          <ProInsightRow item={item} />
        </>
      )}
    </div>
  );
}
