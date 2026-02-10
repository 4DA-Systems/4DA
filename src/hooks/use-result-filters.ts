import { useState, useMemo, useCallback } from 'react';
import type { SourceRelevance, FeedbackAction, FeedbackGiven } from '../types';

const ALL_SOURCES = new Set([
  'hackernews', 'arxiv', 'reddit', 'github',
  'rss', 'youtube', 'twitter', 'producthunt',
]);

/** Normalize URL for dedup: strip protocol, www, trailing slash, query params */
function normalizeUrl(url: string | null | undefined): string | null {
  if (!url) return null;
  try {
    let u = url.toLowerCase().trim();
    u = u.replace(/^https?:\/\//, '').replace(/^www\./, '');
    // Remove query params and fragment
    u = u.split('?')[0].split('#')[0];
    // Remove trailing slash
    u = u.replace(/\/+$/, '');
    return u;
  } catch {
    return url;
  }
}

export const useResultFilters = (
  relevanceResults: SourceRelevance[],
  feedbackGiven: FeedbackGiven,
  recordInteraction: (id: number, action: FeedbackAction, item: SourceRelevance) => Promise<void>,
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

  const filteredResults = useMemo(() => {
    // Step 1: Filter by source and relevance
    const filtered = relevanceResults.filter(item => {
      const source = item.source_type || 'hackernews';
      if (!sourceFilters.has(source)) return false;
      if (showOnlyRelevant && !item.relevant) return false;
      return true;
    });

    // Step 2: Cross-source deduplication by normalized URL
    const urlGroups = new Map<string, SourceRelevance[]>();
    const noUrl: SourceRelevance[] = [];

    for (const item of filtered) {
      const normalized = normalizeUrl(item.url);
      if (normalized) {
        const group = urlGroups.get(normalized);
        if (group) {
          group.push(item);
        } else {
          urlGroups.set(normalized, [item]);
        }
      } else {
        noUrl.push(item);
      }
    }

    // Keep highest-scoring item per URL group, tag with seen_on
    const deduped: SourceRelevance[] = [];
    for (const group of urlGroups.values()) {
      // Sort by score desc, pick best
      group.sort((a, b) => b.top_score - a.top_score);
      const best = { ...group[0] };
      if (group.length > 1) {
        best.seen_on = [...new Set(group.map(g => g.source_type || 'hackernews'))];
      }
      deduped.push(best);
    }
    deduped.push(...noUrl);

    // Step 3: Sort
    deduped.sort((a, b) => {
      if (sortBy === 'score') {
        return b.top_score - a.top_score;
      }
      return b.id - a.id;
    });

    return deduped;
  }, [relevanceResults, sourceFilters, showOnlyRelevant, sortBy]);

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
