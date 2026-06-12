// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useRef, useMemo, useCallback, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { ResultItem } from './ResultItem';
import { LoadingOrEmptyState } from './LoadingOrEmptyState';
import { NoResultsState } from './NoResultsState';
import { ContextPanel } from './context-panel';
import { ResultFiltersBar } from './search/ResultFiltersBar';
import { useTranslatedContent } from './ContentTranslationProvider';
import { useAppStore } from '../store';
import { useResultFilters } from '../hooks';

interface ResultsViewProps {
  newItemIds: Set<number>;
  focusedIndex: number;
}

export function ResultsView({
  newItemIds,
  focusedIndex,
}: ResultsViewProps) {
  const { t } = useTranslation();
  const { getTranslated, requestTranslation } = useTranslatedContent();
  // Data selectors (may change, use useShallow)
  const { state, feedbackGiven, discoveredContext, expandedItem, searchFocusItemId } = useAppStore(
    useShallow((s) => ({
      state: s.appState,
      feedbackGiven: s.feedbackGiven,
      discoveredContext: s.discoveredContext,
      expandedItem: s.expandedItem,
      searchFocusItemId: s.searchFocusItemId,
    })),
  );

  // Action selectors (stable references)
  const setExpandedItem = useAppStore(s => s.setExpandedItem);
  const setSearchFocusItemId = useAppStore(s => s.setSearchFocusItemId);
  const startAnalysis = useAppStore(s => s.startAnalysis);
  const loadContextFiles = useAppStore(s => s.loadContextFiles);
  const clearContext = useAppStore(s => s.clearContext);
  const indexContext = useAppStore(s => s.indexContext);
  const recordInteraction = useAppStore(s => s.recordInteraction);

  const handleToggleExpand = useCallback((itemId: number) => {
    setExpandedItem(useAppStore.getState().expandedItem === itemId ? null : itemId);
  }, [setExpandedItem]);

  const {
    sourceFilters,
    sortBy,
    showOnlyRelevant,
    showSavedOnly,
    searchQuery,
    setSortBy,
    setShowOnlyRelevant,
    setShowSavedOnly,
    setSearchQuery,
    toggleSourceFilter,
    resetSourceFilters,
    filteredResults,
    profileEmpty,
    dismissAllBelow,
    saveAllAbove,
  } = useResultFilters();

  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: filteredResults.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 120,
    overscan: 5,
  });

  const relevantCount = useMemo(() => filteredResults.filter(r => r.relevant).length, [filteredResults]);
  const topPicksCount = useMemo(() => filteredResults.filter(r => r.top_score >= 0.72).length, [filteredResults]);
  const criticalCount = useMemo(() => filteredResults.filter(r => r.is_critical_alert).length, [filteredResults]);
  const totalCount = state.relevanceResults.length;

  // Topic cluster detection: find where 2+ consecutive items share a primary_topic
  const topicClusterStarts = useMemo(() => {
    if (sortBy !== 'score') return new Map<number, string>();
    const starts = new Map<number, string>();
    let i = 0;
    while (i < filteredResults.length) {
      const topic = filteredResults[i]!.primary_topic;
      if (topic) {
        let j = i + 1;
        while (j < filteredResults.length && filteredResults[j]!.primary_topic === topic) j++;
        if (j - i >= 2) starts.set(i, topic);
        i = j;
      } else {
        i++;
      }
    }
    return starts;
  }, [filteredResults, sortBy]);

  // Deep-link from the command search: scroll to + expand a specific item.
  useEffect(() => {
    if (searchFocusItemId == null) return;
    const idx = filteredResults.findIndex(r => r.id === searchFocusItemId);
    if (idx >= 0) {
      const id = searchFocusItemId;
      requestAnimationFrame(() => virtualizer.scrollToIndex(idx, { align: 'center' }));
      setExpandedItem(id);
      setSearchFocusItemId(null);
      return;
    }
    // Hidden by the relevance filter but present in the full set — reveal and re-run.
    if (showOnlyRelevant && state.relevanceResults.some(r => r.id === searchFocusItemId)) {
      setShowOnlyRelevant(false);
      return;
    }
    // Off-feed corpus item not in this list — clear; the user is already on Signal.
    setSearchFocusItemId(null);
  }, [searchFocusItemId, filteredResults, virtualizer, setExpandedItem, setSearchFocusItemId, showOnlyRelevant, setShowOnlyRelevant, state.relevanceResults]);

  useEffect(() => {
    const items = [
      ...filteredResults.map((r) => ({ id: String(r.id), text: r.title })),
      ...(state.nearMisses ?? []).map((r) => ({ id: String(r.id), text: r.title })),
    ];
    if (items.length > 0) requestTranslation(items);
  }, [filteredResults, state.nearMisses, requestTranslation]);
  const sourcesWithResults = useMemo(() => new Set(state.relevanceResults.map(r => r.source_type || 'hackernews')), [state.relevanceResults]);

  return (
    <div className="space-y-6">
      {/* Context Files Panel (collapsible) */}
      <details className="bg-bg-secondary rounded-lg border border-border">
        {/* eslint-disable i18next/no-literal-string */}
        <summary className="px-5 py-3 text-xs text-text-muted cursor-pointer hover:text-text-secondary">
          Context Files ({state.contextFiles.length} files)
        </summary>
        {/* eslint-enable i18next/no-literal-string */}
        <ContextPanel
          contextFiles={state.contextFiles}
          discoveredContext={discoveredContext}
          loading={state.loading}
          onReload={() => { void loadContextFiles(); }}
          onIndex={() => { void indexContext(); }}
          onClear={() => { void clearContext(); }}
        />
      </details>

      {/* Relevance Results Panel */}
      <section aria-label={t('results.title')} className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
        <div className="px-5 py-4 border-b border-border">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-3">
              <div aria-hidden="true" className={`w-2 h-2 rounded-full flex-shrink-0 ${
                state.analysisComplete ? (relevantCount > 0 ? 'bg-green-400' : 'bg-text-muted/50') : 'bg-orange-400 animate-pulse'
              }`} />
              <div>
                <div className="flex items-center gap-2">
                  <h2 className="font-medium text-text-primary">{t('results.title')}</h2>
                  {newItemIds.size > 0 && (
                    <span className="px-2 py-0.5 text-[10px] bg-blue-500/20 text-blue-400 rounded-full font-medium animate-pulse">
                      {t('results.new', { count: newItemIds.size })}
                    </span>
                  )}
                </div>
                <p className="text-xs text-text-muted" aria-live="polite">
                  {state.analysisComplete
                    ? (profileEmpty
                        ? <>{filteredResults.length} {t('results.freshPicksSubtext', 'fresh picks · ranked by recency & quality. Add a project folder or interests to personalize.')}</>
                        : <>
                            {showOnlyRelevant
                              ? t('results.countFiltered', { filtered: filteredResults.length, total: totalCount })
                              : t('results.countAll', { count: filteredResults.length })
                            }
                            {topPicksCount > 0 && ` · ${t('results.topPicks', { count: topPicksCount })}`}
                            {criticalCount > 0 && ` · ${t('results.criticalCount', { count: criticalCount })}`}
                          </>)
                    : t('results.clickAnalyze')}
                </p>
              </div>
            </div>
          </div>

          {/* Filter Bar */}
          {state.analysisComplete && (
            <ResultFiltersBar
              searchQuery={searchQuery}
              setSearchQuery={setSearchQuery}
              sourceFilters={sourceFilters}
              sourcesWithResults={sourcesWithResults}
              toggleSourceFilter={toggleSourceFilter}
              resetSourceFilters={resetSourceFilters}
              sortBy={sortBy}
              setSortBy={setSortBy}
              showOnlyRelevant={showOnlyRelevant}
              setShowOnlyRelevant={setShowOnlyRelevant}
              showSavedOnly={showSavedOnly}
              setShowSavedOnly={setShowSavedOnly}
              dismissAllBelow={dismissAllBelow}
              saveAllAbove={saveAllAbove}
            />
          )}
        </div>
        <div ref={parentRef} className="p-4 max-h-[calc(100vh-380px)] overflow-y-auto">
          {!state.analysisComplete ? (
            <LoadingOrEmptyState
              loading={state.loading}
              progressMessage={state.progressMessage}
              progress={state.progress}
              progressStage={state.progressStage}
              detectedStack={discoveredContext?.tech?.map(item => item.name) ?? []}
              onStartAnalysis={() => { void startAnalysis(); }}
            />
          ) : filteredResults.length === 0 ? (
            <NoResultsState
              totalAnalyzed={state.relevanceResults.length}
              showOnlyRelevant={showOnlyRelevant}
              sourceFilters={sourceFilters}
              nearMisses={state.nearMisses}
              setShowOnlyRelevant={setShowOnlyRelevant}
              resetSourceFilters={resetSourceFilters}
              getTranslated={getTranslated}
            />
          ) : (
            <div
              role="listbox"
              aria-label={t('results.title')}
              aria-activedescendant={focusedIndex >= 0 && filteredResults[focusedIndex] ? `result-item-${filteredResults[focusedIndex].id}` : undefined}
              tabIndex={-1}
              style={{ height: `${virtualizer.getTotalSize()}px`, width: '100%', position: 'relative' }}
            >
              {virtualizer.getVirtualItems().map((virtualRow) => {
                const item = filteredResults[virtualRow.index]!;
                const idx = virtualRow.index;
                // Score group headers (only when sorting by score)
                let groupHeader: string | null = null;
                if (sortBy === 'score' && profileEmpty) {
                  // Cold start: one honest header — these are fresh, not personalized.
                  // No "Top picks"/"Relevant" labels (would imply ranking we can't do yet).
                  if (idx === 0) groupHeader = t('results.freshPicksGroup', 'Fresh picks — not yet personalized');
                } else if (sortBy === 'score' && idx > 0) {
                  const prev = filteredResults[idx - 1]!;
                  if (prev.top_score >= 0.72 && item.top_score < 0.72) {
                    groupHeader = t('results.relevantGroup');
                  } else if (prev.top_score >= 0.50 && item.top_score < 0.50) {
                    groupHeader = t('results.belowThreshold');
                  }
                } else if (sortBy === 'score' && idx === 0 && item.top_score >= 0.72) {
                  groupHeader = t('results.topPicksGroup');
                } else if (sortBy === 'score' && idx === 0 && item.top_score >= 0.50) {
                  groupHeader = t('results.relevantGroup');
                }
                return (
                  <div
                    key={item.id}
                    style={{
                      position: 'absolute',
                      top: 0,
                      left: 0,
                      width: '100%',
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                    ref={virtualizer.measureElement}
                    data-index={virtualRow.index}
                  >
                    {groupHeader && (
                      <div className="flex items-center gap-3 mb-3 mt-2 first:mt-0">
                        <span className={`text-xs font-medium px-2 py-1 rounded-lg ${
                          groupHeader === t('results.topPicksGroup') ? 'bg-orange-500/10 text-orange-400' :
                          groupHeader === t('results.relevantGroup') ? 'bg-green-500/10 text-green-400' :
                          'bg-gray-500/10 text-text-muted'
                        }`}>
                          {groupHeader}
                        </span>
                        <div className="flex-1 h-px bg-border" />
                      </div>
                    )}
                    {topicClusterStarts.has(idx) && (
                      <div className="flex items-center gap-2 mb-2 mt-1">
                        <div className="flex-1 h-px bg-border/50" />
                        <span className="text-[10px] text-text-muted/70 uppercase tracking-wider font-medium px-1.5">
                          {topicClusterStarts.get(idx)}
                        </span>
                        <div className="flex-1 h-px bg-border/50" />
                      </div>
                    )}
                    <div className="pb-3">
                      <ResultItem
                        item={item}
                        isExpanded={expandedItem === item.id}
                        isFocused={focusedIndex === idx}
                        onToggleExpand={handleToggleExpand}
                        feedbackGiven={feedbackGiven}
                        onRecordInteraction={(itemId, actionType, item) => { void recordInteraction(itemId, actionType, item); }}
                        comparePool={expandedItem === item.id ? filteredResults : undefined}
                        itemIndex={idx}
                        totalItems={filteredResults.length}
                      />
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
