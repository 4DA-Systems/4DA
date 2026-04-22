// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useMemo, useCallback } from 'react';
import { useAppStore } from '../store';

/** Run promise-returning tasks with bounded concurrency (prevents IPC queue saturation) */
async function pLimit<T>(tasks: (() => Promise<T>)[], concurrency: number): Promise<PromiseSettledResult<T>[]> {
  const results: PromiseSettledResult<T>[] = [];
  let index = 0;

  async function runNext(): Promise<void> {
    while (index < tasks.length) {
      const i = index++;
      try {
        results[i] = { status: 'fulfilled', value: await tasks[i]!() };
      } catch (reason) {
        results[i] = { status: 'rejected', reason };
      }
    }
  }

  const workers = Array.from({ length: Math.min(concurrency, tasks.length) }, () => runNext());
  await Promise.all(workers);
  return results;
}

/** Normalize URL for dedup: strip protocol, www, trailing slash, query params */
function normalizeUrl(url: string | null | undefined): string | null {
  if (!url) return null;
  try {
    let u = url.toLowerCase().trim();
    u = u.replace(/^https?:\/\//, '').replace(/^www\./, '');
    // Remove query params and fragment
    u = u.split('?')[0]!.split('#')[0]!;
    // Remove trailing slash
    u = u.replace(/\/+$/, '');
    return u;
  } catch {
    return url;
  }
}

/**
 * Result filters hook — reads all state from Zustand store.
 * Filter state lives in the store; filteredResults is derived here via useMemo.
 */
export const useResultFilters = () => {
  const relevanceResults = useAppStore(s => s.appState.relevanceResults);
  const feedbackGiven = useAppStore(s => s.feedbackGiven);
  const recordInteraction = useAppStore(s => s.recordInteraction);
  const setSettingsStatus = useAppStore(s => s.setSettingsStatus);

  const sourceFilters = useAppStore(s => s.sourceFilters);
  const sortBy = useAppStore(s => s.sortBy);
  const showOnlyRelevant = useAppStore(s => s.showOnlyRelevant);
  const showSavedOnly = useAppStore(s => s.showSavedOnly);
  const searchQuery = useAppStore(s => s.searchQuery);
  const toggleSourceFilter = useAppStore(s => s.toggleSourceFilter);
  const resetSourceFilters = useAppStore(s => s.resetSourceFilters);
  const setSortBy = useAppStore(s => s.setSortBy);
  const setShowOnlyRelevant = useAppStore(s => s.setShowOnlyRelevant);
  const setShowSavedOnly = useAppStore(s => s.setShowSavedOnly);
  const setSearchQuery = useAppStore(s => s.setSearchQuery);

  const filteredResults = useMemo(() => {
    const query = searchQuery.toLowerCase().trim();

    // Step 1: Filter by source, relevance, saved, and search query
    const filtered = relevanceResults.filter(item => {
      const source = item.source_type || 'hackernews';
      if (!sourceFilters.has(source)) return false;
      if (showOnlyRelevant && !item.relevant) return false;
      if (showSavedOnly && feedbackGiven[item.id] !== 'save') return false;
      // Search filter: match against title, explanation, source type
      if (query) {
        const title = (item.title || '').toLowerCase();
        const explanation = (item.explanation || '').toLowerCase();
        const sourceLabel = (item.source_type || '').toLowerCase();
        if (!title.includes(query) && !explanation.includes(query) && !sourceLabel.includes(query)) {
          return false;
        }
      }
      return true;
    });

    // Step 2: Cross-source deduplication by normalized URL
    const urlGroups = new Map<string, typeof filtered>();
    const noUrl: typeof filtered = [];

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
    const deduped: typeof filtered = [];
    for (const group of urlGroups.values()) {
      // Sort by score desc, pick best
      group.sort((a, b) => b.top_score - a.top_score);
      const best = { ...group[0]! };
      if (group.length > 1) {
        best.seen_on = [...new Set(group.map(g => g.source_type || 'hackernews'))];
      }
      deduped.push(best);
    }
    deduped.push(...noUrl);

    // Step 3: Sort
    const priorityOrder: Record<string, number> = {
      critical: 0, alert: 1, advisory: 2, watch: 3,
    };
    const applicabilityOrder: Record<string, number> = {
      affected: 0, likely_affected: 1, needs_verification: 2, not_affected: 3,
    };
    const urgencyOrder: Record<string, number> = {
      immediate: 0, this_week: 1, awareness: 2, none: 3,
    };

    deduped.sort((a, b) => {
      if (sortBy === 'score') {
        return b.top_score - a.top_score;
      }
      if (sortBy === 'priority') {
        const aPrio = priorityOrder[a.signal_priority ?? 'watch'] ?? 4;
        const bPrio = priorityOrder[b.signal_priority ?? 'watch'] ?? 4;
        return aPrio - bPrio || b.top_score - a.top_score;
      }
      if (sortBy === 'applicability') {
        const aAppl = applicabilityOrder[a.applicability ?? 'not_affected'] ?? 4;
        const bAppl = applicabilityOrder[b.applicability ?? 'not_affected'] ?? 4;
        return aAppl - bAppl || b.top_score - a.top_score;
      }
      if (sortBy === 'urgency') {
        const aUrg = urgencyOrder[a.score_breakdown?.necessity_urgency ?? 'none'] ?? 4;
        const bUrg = urgencyOrder[b.score_breakdown?.necessity_urgency ?? 'none'] ?? 4;
        return aUrg - bUrg || b.top_score - a.top_score;
      }
      if (sortBy === 'freshness') {
        const aDate = a.created_at ? new Date(a.created_at).getTime() : 0;
        const bDate = b.created_at ? new Date(b.created_at).getTime() : 0;
        return bDate - aDate;
      }
      return b.id - a.id;
    });

    return deduped;
  }, [relevanceResults, sourceFilters, showOnlyRelevant, showSavedOnly, sortBy, searchQuery, feedbackGiven]);

  const dismissAllBelow = useCallback(async (threshold: number) => {
    const itemsToDismiss = filteredResults.filter(
      item => item.top_score < threshold && !feedbackGiven[item.id],
    );
    const results = await pLimit(
      itemsToDismiss.map(item => () => recordInteraction(item.id, 'dismiss', item)),
      10,
    );
    const failed = results.filter(r => r.status === 'rejected').length;
    const succeeded = results.length - failed;
    const msg = failed > 0
      ? `Dismissed ${succeeded} of ${results.length}. ${failed} failed.`
      : `Dismissed ${succeeded} items below ${Math.round(threshold * 100)}%`;
    setSettingsStatus(msg);
    setTimeout(() => setSettingsStatus(''), 4000);
  }, [filteredResults, feedbackGiven, recordInteraction, setSettingsStatus]);

  const saveAllAbove = useCallback(async (threshold: number) => {
    const itemsToSave = filteredResults.filter(
      item => item.top_score >= threshold && !feedbackGiven[item.id],
    );
    const results = await pLimit(
      itemsToSave.map(item => () => recordInteraction(item.id, 'save', item)),
      10,
    );
    const failed = results.filter(r => r.status === 'rejected').length;
    const succeeded = results.length - failed;
    const msg = failed > 0
      ? `Saved ${succeeded} of ${results.length}. ${failed} failed.`
      : `Saved ${succeeded} items above ${Math.round(threshold * 100)}%`;
    setSettingsStatus(msg);
    setTimeout(() => setSettingsStatus(''), 4000);
  }, [filteredResults, feedbackGiven, recordInteraction, setSettingsStatus]);

  return {
    sourceFilters,
    sortBy,
    setSortBy,
    showOnlyRelevant,
    setShowOnlyRelevant,
    showSavedOnly,
    setShowSavedOnly,
    searchQuery,
    setSearchQuery,
    toggleSourceFilter,
    resetSourceFilters,
    filteredResults,
    dismissAllBelow,
    saveAllAbove,
  };
};
