import { useState, useMemo, useCallback } from 'react';
import type { HNRelevance, FeedbackAction, FeedbackGiven } from '../types';

const ALL_SOURCES = new Set([
  'hackernews', 'arxiv', 'reddit', 'github',
  'rss', 'youtube', 'twitter', 'producthunt',
]);

export const useResultFilters = (
  relevanceResults: HNRelevance[],
  feedbackGiven: FeedbackGiven,
  recordInteraction: (id: number, action: FeedbackAction, item: HNRelevance) => Promise<void>,
  setSettingsStatus: (msg: string) => void,
) => {
  const [sourceFilters, setSourceFilters] = useState<Set<string>>(new Set(ALL_SOURCES));
  const [sortBy, setSortBy] = useState<'score' | 'date'>('score');
  const [showOnlyRelevant, setShowOnlyRelevant] = useState(false);

  const toggleSourceFilter = useCallback((source: string) => {
    setSourceFilters(prev => {
      const next = new Set(prev);
      if (next.has(source)) {
        if (next.size > 1) next.delete(source);
      } else {
        next.add(source);
      }
      return next;
    });
  }, []);

  const filteredResults = useMemo(() =>
    relevanceResults
      .filter(item => {
        const source = item.source_type || 'hackernews';
        if (!sourceFilters.has(source)) return false;
        if (showOnlyRelevant && !item.relevant) return false;
        return true;
      })
      .sort((a, b) => {
        if (sortBy === 'score') {
          return b.top_score - a.top_score;
        }
        return b.id - a.id;
      }),
    [relevanceResults, sourceFilters, showOnlyRelevant, sortBy],
  );

  const dismissAllBelow = useCallback(async (threshold: number) => {
    const itemsToDismiss = filteredResults.filter(
      item => item.top_score < threshold && !feedbackGiven[item.id],
    );
    for (const item of itemsToDismiss) {
      await recordInteraction(item.id, 'dismiss', item);
    }
    setSettingsStatus(`Dismissed ${itemsToDismiss.length} items below ${Math.round(threshold * 100)}%`);
    setTimeout(() => setSettingsStatus(''), 3000);
  }, [filteredResults, feedbackGiven, recordInteraction, setSettingsStatus]);

  const saveAllAbove = useCallback(async (threshold: number) => {
    const itemsToSave = filteredResults.filter(
      item => item.top_score >= threshold && !feedbackGiven[item.id],
    );
    for (const item of itemsToSave) {
      await recordInteraction(item.id, 'save', item);
    }
    setSettingsStatus(`Saved ${itemsToSave.length} items above ${Math.round(threshold * 100)}%`);
    setTimeout(() => setSettingsStatus(''), 3000);
  }, [filteredResults, feedbackGiven, recordInteraction, setSettingsStatus]);

  return {
    sourceFilters,
    sortBy,
    setSortBy,
    showOnlyRelevant,
    setShowOnlyRelevant,
    toggleSourceFilter,
    filteredResults,
    dismissAllBelow,
    saveAllAbove,
  };
};
