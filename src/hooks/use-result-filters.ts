// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useMemo, useCallback, useEffect } from 'react';
import { useAppStore } from '../store';
import { isProfileEmpty } from '../utils/profile-empty';
import type { SourceRelevance } from '../types';

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

/** Cap on sibling titles carried by an advisory stack's representative. */
const ADVISORY_STACK_TITLE_CAP = 8;

/**
 * Collapse same-package security advisories behind their highest-scoring
 * representative. An OSV backfill can land dozens of advisories for one
 * dependency in a single sync (34 axios advisories, 2026-07-09); as sibling
 * cards they carry one card of information. Grouping key: the advisory's
 * primary matched dependency. Items that aren't security alerts, or matched
 * no dependency, pass through untouched. Exported for tests.
 */
export function stackAdvisories(items: SourceRelevance[]): SourceRelevance[] {
  const groups = new Map<string, SourceRelevance[]>();
  const passthrough: SourceRelevance[] = [];
  for (const item of items) {
    const dep = item.score_breakdown?.matched_deps?.[0];
    if (item.signal_type === 'security_alert' && dep) {
      const group = groups.get(dep);
      if (group) group.push(item);
      else groups.set(dep, [item]);
    } else {
      passthrough.push(item);
    }
  }
  const out: SourceRelevance[] = [];
  for (const group of groups.values()) {
    if (group.length === 1) {
      out.push(group[0]!);
      continue;
    }
    // Representative = most urgent verdict first, then score: an "affected"
    // sibling must front the stack even if a stale one scored higher.
    const applicabilityRank = (r: SourceRelevance) =>
      r.applicability === 'affected' ? 0
      : r.applicability === 'likely_affected' ? 1
      : r.applicability === 'not_affected' ? 3
      : 2;
    group.sort((a, b) => (applicabilityRank(a) - applicabilityRank(b)) || (b.top_score - a.top_score));
    const rep = { ...group[0]! };
    rep.advisory_stack_count = group.length - 1;
    rep.advisory_stack_titles = group.slice(1, 1 + ADVISORY_STACK_TITLE_CAP).map(g => g.title);
    out.push(rep);
  }
  return [...out, ...passthrough];
}

/**
 * Result filters hook — reads all state from Zustand store.
 * Filter state lives in the store; filteredResults is derived here via useMemo.
 */
export const useResultFilters = () => {
  const relevanceResults = useAppStore(s => s.appState.relevanceResults);
  const detectedTech = useAppStore(s => s.discoveredContext?.tech);
  const interests = useAppStore(s => s.userContext?.interests);
  const feedbackGiven = useAppStore(s => s.feedbackGiven);
  const snoozedItemIds = useAppStore(s => s.snoozedItemIds);
  const loadSnoozedIds = useAppStore(s => s.loadSnoozedIds);
  const recordInteraction = useAppStore(s => s.recordInteraction);
  const setSettingsStatus = useAppStore(s => s.setSettingsStatus);

  // Active snoozes persist across sessions (Phase 96) — hydrate once so the
  // list respects them; expiry is implicit (backend only returns live ones).
  useEffect(() => {
    void loadSnoozedIds();
  }, [loadSnoozedIds]);

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

  // Cold-start signal — shared with the first-run celebration via profile-empty.ts
  // so the two never disagree. With no profile, 0 items are relevant, so the
  // default "show only relevant" filter would render an empty list and the
  // "Browse fresh picks" CTA leads nowhere; instead we relax that filter and
  // fall back to an honest recency/quality ranking (see the sort branch below).
  const profileEmpty = useMemo(
    () => isProfileEmpty(detectedTech?.length ?? 0, interests?.length ?? 0, relevanceResults.some(r => r.relevant)),
    [detectedTech, interests, relevanceResults],
  );

  const filteredResults = useMemo(() => {
    const query = searchQuery.toLowerCase().trim();

    // Step 1: Filter by snooze, source, relevance, saved, and search query
    const filtered = relevanceResults.filter(item => {
      // Snoozed = deferred, not rejected: hidden until the snooze expires.
      if (snoozedItemIds.has(item.id)) return false;
      const source = item.source_type || 'hackernews';
      if (!sourceFilters.has(source)) return false;
      if (showOnlyRelevant && !profileEmpty && !item.relevant) return false;
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
    const urlDeduped: typeof filtered = [];
    for (const group of urlGroups.values()) {
      // Sort by score desc, pick best
      group.sort((a, b) => b.top_score - a.top_score);
      const best = { ...group[0]! };
      if (group.length > 1) {
        best.seen_on = [...new Set(group.map(g => g.source_type || 'hackernews'))];
      }
      urlDeduped.push(best);
    }
    urlDeduped.push(...noUrl);

    // Step 2b: Advisory stacking — same-package advisories collapse behind
    // one representative row (see stackAdvisories).
    const deduped = stackAdvisories(urlDeduped);

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
      if (sortBy === 'score' && profileEmpty) {
        // Cold start: top_score is meaningless (the gate caps every item ~0.1
        // with no profile), so rank by a profile-free quality prior — content
        // quality × freshness, newest first as tiebreak. This is honest "fresh
        // picks", NOT a personalization claim, and only runs while profileEmpty.
        const prior = (r: typeof a) =>
          (r.score_breakdown?.content_quality_mult ?? 1) *
          (r.score_breakdown?.freshness_mult ?? 1);
        const dp = prior(b) - prior(a);
        if (Math.abs(dp) > 1e-6) return dp;
        const aFresh = a.created_at ? new Date(a.created_at).getTime() : 0;
        const bFresh = b.created_at ? new Date(b.created_at).getTime() : 0;
        return bFresh - aFresh;
      }
      if (sortBy === 'score') {
        const aN = a.score_breakdown?.necessity_score ?? 0;
        const bN = b.score_breakdown?.necessity_score ?? 0;
        const aComposite = a.top_score + aN * 0.4;
        const bComposite = b.top_score + bN * 0.4;
        return bComposite - aComposite;
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
  }, [relevanceResults, profileEmpty, sourceFilters, showOnlyRelevant, showSavedOnly, sortBy, searchQuery, feedbackGiven, snoozedItemIds]);

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
    profileEmpty,
    dismissAllBelow,
    saveAllAbove,
  };
};
