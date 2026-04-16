import { useTranslation } from 'react-i18next';
import type { SourceRelevance, FeedbackAction } from '../../types';
import { formatScore, getScoreColor } from '../../utils/score';
import { ArticleReader } from '../ArticleReader';
import { ScoreAutopsy } from '../ScoreAutopsy';
import { ScoreBreakdownRow } from './ScoreBreakdownRow';
import { FeedbackButtons } from './FeedbackButtons';
import { StreetsEngineLink } from './StreetsEngineLink';

interface ResultItemExpandedProps {
  item: SourceRelevance;
  isNew?: boolean;
  isTopPick: boolean;
  isHighConfidence: boolean;
  feedback: FeedbackAction | undefined;
  onRecordInteraction: (itemId: number, actionType: FeedbackAction, item: SourceRelevance) => void;
  summary: string | null;
  summaryLoading: boolean;
  summaryError: string | null;
  onGenerateSummary: () => void;
}

export function ResultItemExpanded({
  item,
  isNew,
  isTopPick,
  isHighConfidence,
  feedback,
  onRecordInteraction,
  summary,
  summaryLoading,
  summaryError,
  onGenerateSummary,
}: ResultItemExpandedProps) {
  const { t } = useTranslation();
  return (
    <div id={`result-detail-${item.id}`} className="px-4 pb-3 border-t border-border/50 mt-2 pt-3">
      {/* Why This Matters - Full Display */}
      {item.explanation && (
        <div className="mb-3 p-2 bg-bg-primary/50 rounded border border-accent-gold/30">
          <div className="text-xs text-accent-gold font-medium mb-1">
            {t('results.whyThisMatters')}
          </div>
          <div className="text-xs text-text-secondary leading-relaxed">
            {item.explanation}
          </div>
        </div>
      )}

      {/* AI Summary */}
      <div className="mb-3">
        {summary ? (
          <div className="p-2 bg-bg-primary/50 rounded border border-cyan-500/20">
            <div className="text-[10px] text-cyan-400 font-medium mb-1">{t('results.aiSummary')}</div>
            <div className="text-xs text-text-secondary leading-relaxed">{summary}</div>
          </div>
        ) : (
          <button
            onClick={onGenerateSummary}
            disabled={summaryLoading}
            aria-label={summaryLoading ? t('briefing.generating') : `${t('results.generateAiSummary')} for ${item.title}`}
            className="text-[11px] px-2.5 py-1.5 rounded border border-cyan-500/20 text-cyan-400 hover:bg-cyan-500/10 transition-colors disabled:opacity-50"
          >
            {summaryLoading ? t('briefing.generating') : t('results.generateAiSummary')}
          </button>
        )}
        {summaryError && (
          <div className="mt-1 text-[10px] text-red-400">{summaryError}</div>
        )}
      </div>

      {/* STREETS Revenue Engine Connection */}
      <StreetsEngineLink item={item} />

      {/* Article Reader */}
      <ArticleReader
        itemId={item.id}
        url={item.url ?? undefined}
        contentType={item.score_breakdown?.content_type}
      />

      {/* Quality Indicators */}
      <ScoreBreakdownRow
        item={item}
        isNew={isNew}
        isTopPick={isTopPick}
        isHighConfidence={isHighConfidence}
      />

      {/* Feedback Buttons */}
      <FeedbackButtons
        item={item}
        feedback={feedback}
        onRecordInteraction={onRecordInteraction}
      />

      <div className="text-xs text-text-muted mb-2 font-medium">
        {t('results.topMatches')}
      </div>
      <ul className="space-y-2">
        {item.matches.map((match, i) => (
          <li
            key={i}
            className="text-xs bg-bg-primary rounded p-2 border border-border/30"
          >
            <div className="flex items-center gap-2 mb-1">
              <span className={`font-mono ${getScoreColor(match.similarity)}`} aria-label={`Match score: ${formatScore(match.similarity)}`}>
                {formatScore(match.similarity)}
              </span>
              <span className="text-text-muted">-&gt;</span>
              <span className="text-accent-gold font-medium">
                {match.source_file}
              </span>
            </div>
            <div className="text-text-secondary ps-12 leading-relaxed">
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
  );
}
