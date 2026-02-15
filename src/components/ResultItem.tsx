import { memo } from 'react';
import type { SourceRelevance, FeedbackAction, FeedbackGiven } from '../types';
import { useItemSummary } from '../hooks/use-item-summary';
import { ResultItemCollapsed } from './result-item/ResultItemCollapsed';
import { ResultItemExpanded } from './result-item/ResultItemExpanded';

function generateFallbackReason(item: SourceRelevance): string {
  const parts: string[] = [];
  const b = item.score_breakdown;
  if (b) {
    if (b.signal_count != null && b.signal_count >= 2) {
      parts.push(`${b.signal_count} signal${b.signal_count > 1 ? 's' : ''} confirmed`);
    }
    if (b.context_score > 0.3) parts.push('Strong project context match');
    if (b.interest_score > 0.3) parts.push('Matches declared interests');
    if (b.ace_boost > 0.1) parts.push('Active in your recent work');
    if (b.affinity_mult > 1.2) parts.push('Learned preference match');
    if (b.freshness_mult != null && b.freshness_mult > 1.1) parts.push('Recently published');
  }
  if (item.signal_type) {
    const labels: Record<string, string> = {
      security_alert: 'Security alert',
      breaking_change: 'Breaking change',
      tool_discovery: 'Tool discovery',
      tech_trend: 'Emerging trend',
      learning: 'Learning resource',
      competitive_intel: 'Competitive intel',
    };
    parts.unshift(labels[item.signal_type] || item.signal_type);
  }
  return parts.length > 0 ? parts.slice(0, 2).join(' \u00b7 ') : '';
}

interface ResultItemProps {
  item: SourceRelevance;
  isExpanded: boolean;
  isFocused?: boolean;
  isNew?: boolean;
  onToggleExpand: () => void;
  feedbackGiven: FeedbackGiven;
  onRecordInteraction: (
    itemId: number,
    actionType: FeedbackAction,
    item: SourceRelevance
  ) => void;
}

export const ResultItem = memo(function ResultItem({
  item,
  isExpanded,
  isFocused,
  isNew,
  onToggleExpand,
  feedbackGiven,
  onRecordInteraction,
}: ResultItemProps) {
  const feedback = feedbackGiven[item.id];
  const isTopPick = item.top_score >= 0.72;
  const isHighConfidence = (item.confidence ?? 0) >= 0.7;
  const { summary, summaryLoading, summaryError, generateSummary } = useItemSummary(item.id, isExpanded);

  return (
    <div
      id={`result-item-${item.id}`}
      className={`rounded border transition-colors ${
        isFocused
          ? 'ring-1 ring-orange-500/50'
          : ''
      } ${
        isTopPick
          ? 'bg-gradient-to-r from-orange-500/10 to-transparent border-orange-500/30'
          : item.relevant
          ? 'bg-bg-tertiary border-border'
          : 'bg-bg-primary border-border/50'
      }`}
    >
      <ResultItemCollapsed
        item={item}
        isExpanded={isExpanded}
        onToggleExpand={onToggleExpand}
        feedback={feedback}
        fallbackReason={generateFallbackReason(item)}
      />

      {isExpanded && (
        <ResultItemExpanded
          item={item}
          isNew={isNew}
          isTopPick={isTopPick}
          isHighConfidence={isHighConfidence}
          feedback={feedback}
          onRecordInteraction={onRecordInteraction}
          summary={summary}
          summaryLoading={summaryLoading}
          summaryError={summaryError}
          onGenerateSummary={generateSummary}
        />
      )}
    </div>
  );
});
