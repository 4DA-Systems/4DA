import type { HNRelevance, FeedbackAction, FeedbackGiven } from '../types';
import { formatScore, getScoreColor } from '../utils/score';
import { ConfidenceIndicator } from './ConfidenceIndicator';
import { ScoreAutopsy } from './ScoreAutopsy';

interface ResultItemProps {
  item: HNRelevance;
  isExpanded: boolean;
  onToggleExpand: () => void;
  feedbackGiven: FeedbackGiven;
  onRecordInteraction: (
    itemId: number,
    actionType: FeedbackAction,
    item: HNRelevance
  ) => void;
}

export function ResultItem({
  item,
  isExpanded,
  onToggleExpand,
  feedbackGiven,
  onRecordInteraction,
}: ResultItemProps) {
  const feedback = feedbackGiven[item.id];

  const isTopPick = item.top_score >= 0.6;
  const isHighConfidence = (item.confidence ?? 0) >= 0.7;

  return (
    <li
      className={`rounded border transition-colors ${
        isTopPick
          ? 'bg-gradient-to-r from-orange-500/10 to-transparent border-orange-500/30'
          : item.relevant
          ? 'bg-bg-tertiary border-border'
          : 'bg-bg-primary border-border/50'
      }`}
    >
      {/* Main Row */}
      <button
        onClick={onToggleExpand}
        className="w-full px-4 py-3 text-left"
      >
        <div className="flex items-start gap-3">
          {/* Score Badge + Source */}
          <div className="flex-shrink-0 flex flex-col items-center gap-1">
            <div
              className={`w-14 text-center py-1 rounded font-mono text-sm font-medium ${getScoreColor(
                item.top_score,
              )}`}
            >
              {formatScore(item.top_score)}
            </div>
            <ConfidenceIndicator confidence={item.confidence} />
            {/* Source Badge */}
            <div className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${
              item.source_type === 'hackernews' ? 'bg-orange-500/20 text-orange-400' :
              item.source_type === 'arxiv' ? 'bg-purple-500/20 text-purple-400' :
              item.source_type === 'reddit' ? 'bg-blue-500/20 text-blue-400' :
              'bg-gray-500/20 text-gray-400'
            }`}>
              {item.source_type === 'hackernews' ? 'HN' :
               item.source_type === 'arxiv' ? 'arXiv' :
               item.source_type === 'reddit' ? 'Reddit' :
               item.source_type || 'Unknown'}
            </div>
          </div>

          {/* Title and URL */}
          <div className="flex-1 min-w-0">
            <div className="flex items-start gap-2">
              <div
                className={`text-sm flex-1 ${
                  item.relevant ? 'text-text-primary' : 'text-text-secondary'
                }`}
              >
                {item.title}
              </div>
              {/* Top Pick Badge */}
              {isTopPick && (
                <span className="flex-shrink-0 text-[10px] px-1.5 py-0.5 bg-orange-500/20 text-orange-400 rounded font-medium">
                  {isHighConfidence ? '⭐ Top Pick' : '🔥 Hot'}
                </span>
              )}
            </div>
            {item.url && (
              <div className="text-xs text-text-muted truncate font-mono mt-1">
                {item.url}
              </div>
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
                ? '✓ Saved'
                : feedback === 'dismiss'
                ? '✗ Dismissed'
                : '⊘ Irrelevant'}
            </div>
          )}

          {/* Expand Indicator */}
          <div className="text-text-muted text-xs">
            {isExpanded ? '−' : '+'}
          </div>
        </div>

        {/* Why This Matters - Preview (shown when not expanded, for ALL items) */}
        {!isExpanded && item.explanation && (
          <div className="mt-1.5 text-xs text-text-secondary italic pl-[4.25rem]">
            {item.explanation}
          </div>
        )}

        {/* Inline Score Breakdown (shown when not expanded) */}
        {!isExpanded && item.score_breakdown && (
          <div className="mt-1 pl-[4.25rem] flex gap-2 text-[10px]">
            {item.score_breakdown.context_score > 0.05 && (
              <span className="text-emerald-400/80">
                Context {Math.round(item.score_breakdown.context_score * 100)}%
              </span>
            )}
            {item.score_breakdown.interest_score > 0.05 && (
              <span className="text-cyan-400/80">
                Interest {Math.round(item.score_breakdown.interest_score * 100)}%
              </span>
            )}
            {item.score_breakdown.ace_boost > 0.05 && (
              <span className="text-amber-400/80">
                ACE +{Math.round(item.score_breakdown.ace_boost * 100)}%
              </span>
            )}
            {item.score_breakdown.affinity_mult > 1.05 && (
              <span className="text-pink-400/80">
                Affinity ×{item.score_breakdown.affinity_mult.toFixed(1)}
              </span>
            )}
            {item.score_breakdown.anti_penalty < 0.95 && (
              <span className="text-red-400/80">
                Penalty ×{item.score_breakdown.anti_penalty.toFixed(1)}
              </span>
            )}
            {item.score_breakdown.freshness_mult != null && item.score_breakdown.freshness_mult !== 1.0 && (
              <span className={item.score_breakdown.freshness_mult > 1.0 ? 'text-teal-400/80' : 'text-gray-500/80'}>
                Fresh {item.score_breakdown.freshness_mult > 1.0 ? '+' : ''}{Math.round((item.score_breakdown.freshness_mult - 1.0) * 100)}%
              </span>
            )}
          </div>
        )}
      </button>

      {/* Expanded Matches */}
      {isExpanded && (
        <div className="px-4 pb-3 border-t border-border/50 mt-2 pt-3">
          {/* Why This Matters - Full Display */}
          {item.explanation && (
            <div className="mb-3 p-2 bg-bg-primary/50 rounded border border-accent-gold/30">
              <div className="text-xs text-accent-gold font-medium mb-1">
                Why this matters:
              </div>
              <div className="text-xs text-text-secondary leading-relaxed">
                {item.explanation}
              </div>
            </div>
          )}

          {/* Feedback Buttons */}
          <div className="flex gap-2 mb-3">
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
                Open Link
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
              {feedback === 'save' ? '✓ Saved' : 'Save'}
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
              {feedback === 'dismiss' ? '✗ Dismissed' : 'Dismiss'}
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
              {feedback === 'mark_irrelevant' ? '⊘ Marked' : 'Not Relevant'}
            </button>
          </div>

          <div className="text-xs text-text-muted mb-2 font-medium">
            Top Matches:
          </div>
          <ul className="space-y-2">
            {item.matches.map((match, i) => (
              <li
                key={i}
                className="text-xs bg-bg-primary rounded p-2 border border-border/30"
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className={`font-mono ${getScoreColor(match.similarity)}`}>
                    {formatScore(match.similarity)}
                  </span>
                  <span className="text-text-muted">-&gt;</span>
                  <span className="text-accent-gold font-medium">
                    {match.source_file}
                  </span>
                </div>
                <div className="text-text-secondary pl-12 leading-relaxed">
                  "{match.matched_text}"
                </div>
              </li>
            ))}
          </ul>

          {/* Score Autopsy Component */}
          <ScoreAutopsy
            itemId={item.id}
            sourceType={item.source_type || 'hackernews'}
            currentScore={item.top_score}
          />
        </div>
      )}
    </li>
  );
}
