import { memo, useCallback, useMemo, useState } from 'react';
import type { SourceRelevance, FeedbackAction, FeedbackGiven } from '../types';
import { useItemSummary } from '../hooks/use-item-summary';
import { useViewTracking } from '../hooks/use-view-tracking';
import { useExpandTracking } from '../hooks/use-expand-tracking';
import { ResultItemCollapsed } from './result-item/ResultItemCollapsed';
import { ResultItemExpanded } from './result-item/ResultItemExpanded';
import { ScoreBreakdownDrawer } from './result-item/ScoreBreakdownDrawer';

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
  /** Other scored items available for comparison */
  comparePool?: SourceRelevance[];
}

export const ResultItem = memo(function ResultItem({
  item,
  isExpanded,
  isFocused,
  isNew,
  onToggleExpand,
  feedbackGiven,
  onRecordInteraction,
  comparePool,
}: ResultItemProps) {
  const [showBreakdown, setShowBreakdown] = useState(false);
  const toggleBreakdown = useCallback(() => setShowBreakdown(prev => !prev), []);

  const feedback = feedbackGiven[item.id];
  const isTopPick = item.top_score >= 0.72;
  const isHighConfidence = (item.confidence ?? 0) >= 0.7;
  const { summary, summaryLoading, summaryError, generateSummary } = useItemSummary(item.id, isExpanded);

  // Extract topics from title for behavior tracking
  const itemTopics = useMemo(() =>
    item.title.toLowerCase().split(/\s+/)
      .filter(w => w.length > 3)
      .slice(0, 5),
    [item.title],
  );

  const viewRef = useViewTracking({
    itemId: item.id,
    sourceType: item.source_type || 'unknown',
    enabled: !isExpanded, // Passive scroll tracking when collapsed
    hasExplicitFeedback: !!feedback,
    itemTopics,
  });

  // Track expand dwell time — emits click+dwell when collapsed/unmounted
  useExpandTracking(item.id, item.source_type || 'unknown', isExpanded, itemTopics);

  return (
    <div
      ref={viewRef}
      id={`result-item-${item.id}`}
      role="option"
      aria-selected={isFocused}
      tabIndex={isFocused ? 0 : -1}
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
        onToggleBreakdown={toggleBreakdown}
        showBreakdown={showBreakdown}
        feedback={feedback}
        fallbackReason={generateFallbackReason(item)}
      />

      {showBreakdown && item.score_breakdown && (
        <ScoreBreakdownDrawer
          breakdown={item.score_breakdown}
          finalScore={item.top_score}
          itemId={item.id}
          onClose={toggleBreakdown}
          comparePool={comparePool}
        />
      )}

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
