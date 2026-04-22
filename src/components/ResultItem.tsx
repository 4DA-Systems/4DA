// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import type { SourceRelevance, FeedbackAction, FeedbackGiven } from '../types';
import { useItemSummary } from '../hooks/use-item-summary';
import { useViewTracking } from '../hooks/use-view-tracking';
import { useExpandTracking } from '../hooks/use-expand-tracking';
import { ResultItemCollapsed } from './result-item/ResultItemCollapsed';
import { ResultItemExpanded } from './result-item/ResultItemExpanded';
import { ScoreBreakdownDrawer } from './result-item/ScoreBreakdownDrawer';

function generateFallbackReason(item: SourceRelevance, t: TFunction): string {
  const parts: string[] = [];
  const b = item.score_breakdown;
  if (b) {
    if (b.signal_count != null && b.signal_count >= 2) {
      parts.push(t('result.signalsConfirmed', { count: b.signal_count }));
    }
    if (b.context_score > 0.3) parts.push(t('result.fallbackContext'));
    if (b.interest_score > 0.3) parts.push(t('result.fallbackInterests'));
    if (b.ace_boost > 0.1) parts.push(t('result.fallbackRecentWork'));
    if (b.affinity_mult > 1.2) parts.push(t('result.fallbackPreference'));
    if (b.freshness_mult != null && b.freshness_mult > 1.1) parts.push(t('result.fallbackFresh'));
  }
  if (item.signal_type) {
    const labels: Record<string, string> = {
      security_alert: t('result.signalSecurity'),
      breaking_change: t('result.signalBreaking'),
      tool_discovery: t('result.signalTool'),
      tech_trend: t('result.signalTrend'),
      learning: t('result.signalLearning'),
      competitive_intel: t('result.signalCompetitive'),
    };
    parts.unshift(labels[item.signal_type] || item.signal_type);
  }
  return parts.length > 0 ? parts.slice(0, 2).join(' \u00b7 ') : '';
}

interface ResultItemProps {
  item: SourceRelevance;
  isExpanded: boolean;
  isFocused?: boolean;
  onToggleExpand: (itemId: number) => void;
  feedbackGiven: FeedbackGiven;
  onRecordInteraction: (
    itemId: number,
    actionType: FeedbackAction,
    item: SourceRelevance
  ) => void;
  /** Other scored items available for comparison */
  comparePool?: SourceRelevance[];
  /** Zero-based index in the results list (for scroll depth tracking) */
  itemIndex?: number;
  /** Total items in the results list (for scroll depth tracking) */
  totalItems?: number;
}

export const ResultItem = memo(function ResultItem({
  item,
  isExpanded,
  isFocused,
  onToggleExpand,
  feedbackGiven,
  onRecordInteraction,
  comparePool,
  itemIndex,
  totalItems,
}: ResultItemProps) {
  const { t } = useTranslation();
  const [showBreakdown, setShowBreakdown] = useState(false);
  const toggleBreakdown = useCallback(() => setShowBreakdown(prev => !prev), []);

  const feedback = feedbackGiven[item.id];
  const isTopPick = item.top_score >= 0.72;
  const autoGenerate = itemIndex != null && itemIndex < 3;
  const { summary, summaryLoading, summaryError, generateSummary } = useItemSummary(item.id, isExpanded, { autoGenerate });

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
    itemIndex,
    totalItems,
  });

  // Track expand dwell time — emits click+dwell when collapsed/unmounted
  useExpandTracking(item.id, item.source_type || 'unknown', isExpanded, itemTopics);

  const categoryBorder = useMemo(() => {
    if (item.score_breakdown?.necessity_category === 'security_vulnerability') {
      return 'border-l-2 border-l-red-500/50';
    }
    if (item.signal_type === 'breaking_change') {
      return 'border-l-2 border-l-amber-500/50';
    }
    if (item.signal_type === 'tool_discovery' || item.score_breakdown?.content_type === 'release_announcement') {
      return 'border-l-2 border-l-blue-500/50';
    }
    return '';
  }, [item.score_breakdown?.necessity_category, item.score_breakdown?.content_type, item.signal_type]);

  const scoreGlow = useMemo(() => {
    if (!isTopPick) return undefined;
    const intensity = Math.min((item.top_score - 0.72) * 10, 1);
    return {
      boxShadow: `0 0 ${8 + intensity * 12}px rgba(212, 175, 55, ${0.06 + intensity * 0.1}), inset 0 1px 0 rgba(212, 175, 55, ${0.05 + intensity * 0.08})`,
    };
  }, [isTopPick, item.top_score]);

  return (
    <div
      ref={viewRef}
      id={`result-item-${item.id}`}
      role="option"
      aria-selected={isFocused}
      tabIndex={isFocused ? 0 : -1}
      style={scoreGlow}
      className={`rounded border transition-all duration-500 ${categoryBorder} ${
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
        onToggleExpand={() => onToggleExpand(item.id)}
        onToggleBreakdown={toggleBreakdown}
        showBreakdown={showBreakdown}
        feedback={feedback}
        fallbackReason={generateFallbackReason(item, t)}
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
