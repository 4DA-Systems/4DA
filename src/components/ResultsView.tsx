import { useRef, useMemo, useCallback, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { ResultItem } from './ResultItem';
import { SmartEmptyState } from './SmartEmptyState';
import { ContextPanel } from './context-panel';
import { useTranslatedContent } from './ContentTranslationProvider';
import { getStageLabel } from '../utils/score';
import { ALL_SOURCE_IDS } from '../config/sources';
import { SourceCategoryFilter } from './SourceCategoryFilter';
import { useAppStore } from '../store';
import { useResultFilters } from '../hooks';
import { registerGameComponent } from '../lib/game-components';

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
  useEffect(() => { registerGameComponent('game-tetrahedron'); }, []);
  // Data selectors (may change, use useShallow)
  const { state, feedbackGiven, discoveredContext, expandedItem } = useAppStore(
    useShallow((s) => ({
      state: s.appState,
      feedbackGiven: s.feedbackGiven,
      discoveredContext: s.discoveredContext,
      expandedItem: s.expandedItem,
    })),
  );

  // Action selectors (stable references, no need for useShallow)
  const setExpandedItem = useAppStore(s => s.setExpandedItem);
  const startAnalysis = useAppStore(s => s.startAnalysis);

  // Stable toggle handler — avoids inline arrow defeating memo on every virtualized row
  const handleToggleExpand = useCallback((itemId: number) => {
    setExpandedItem(useAppStore.getState().expandedItem === itemId ? null : itemId);
  }, [setExpandedItem]);
  const loadContextFiles = useAppStore(s => s.loadContextFiles);
  const clearContext = useAppStore(s => s.clearContext);
  const indexContext = useAppStore(s => s.indexContext);
  const recordInteraction = useAppStore(s => s.recordInteraction);

  // Filters (zero-param -- reads from store internally)
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
    dismissAllBelow,
    saveAllAbove,
  } = useResultFilters();

  // Virtual scrolling setup
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: filteredResults.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 120,
    overscan: 5,
  });

  // 5b. Memoize computed filter counts and unique sources
  const relevantCount = useMemo(() => filteredResults.filter(r => r.relevant).length, [filteredResults]);
  const topPicksCount = useMemo(() => filteredResults.filter(r => r.top_score >= 0.72).length, [filteredResults]);

  // Request content translation for visible results + near misses
  useEffect(() => {
    const items = [
      ...filteredResults.map((r) => ({ id: String(r.id), text: r.title })),
      ...(state.nearMisses ?? []).map((r) => ({ id: String(r.id), text: r.title })),
    ];
    if (items.length > 0) requestTranslation(items);
  }, [filteredResults, state.nearMisses, requestTranslation]);
  // Show ALL registered sources (not just those with results) so users see full coverage
  const sourcesWithResults = useMemo(
    () => new Set(state.relevanceResults.map(r => r.source_type || 'hackernews')),
    [state.relevanceResults],
  );

  return (
    <div className="space-y-6">
      {/* Context Files Panel (collapsible) */}
      <details className="bg-bg-secondary rounded-lg border border-border">
        <summary className="px-5 py-3 text-xs text-text-muted cursor-pointer hover:text-text-secondary">
          Context Files ({state.contextFiles.length} files)
        </summary>
        <ContextPanel
          contextFiles={state.contextFiles}
          discoveredContext={discoveredContext}
          loading={state.loading}
          onReload={loadContextFiles}
          onIndex={indexContext}
          onClear={clearContext}
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
                  <h2 className="font-medium text-white">{t('results.title')}</h2>
                  {newItemIds.size > 0 && (
                    <span className="px-2 py-0.5 text-[10px] bg-blue-500/20 text-blue-400 rounded-full font-medium animate-pulse">
                      {t('results.new', { count: newItemIds.size })}
                    </span>
                  )}
                </div>
                <p className="text-xs text-text-muted" aria-live="polite">
                  {state.analysisComplete
                    ? `${filteredResults.length} items · ${relevantCount} relevant · ${topPicksCount} high confidence`
                    : t('results.clickAnalyze')}
                </p>
              </div>
            </div>
          </div>

          {/* Filter Bar */}
          {state.analysisComplete && (
            <div className="flex flex-col sm:flex-row flex-wrap items-start sm:items-center gap-3 pt-3 border-t border-border" role="toolbar" aria-label="Filter and sort controls">
              {/* Search */}
              <div className="relative w-full sm:w-auto">
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder={t('results.searchPlaceholder')}
                  aria-label="Search results by keyword"
                  className="bg-bg-tertiary text-sm text-white placeholder-gray-500 rounded-lg ps-8 pe-3 py-1.5 w-full sm:w-48 border border-transparent focus:border-border focus:outline-none transition-all"
                />
                <svg className="absolute start-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text-muted" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
                {searchQuery && (
                  <button
                    onClick={() => setSearchQuery('')}
                    className="absolute end-2 top-1/2 -translate-y-1/2 text-text-muted hover:text-text-secondary"
                    aria-label="Clear search"
                  >
                    <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                    </svg>
                  </button>
                )}
              </div>

              {/* Source Filters — grouped by category */}
              <SourceCategoryFilter
                sourceFilters={sourceFilters}
                sourcesWithResults={sourcesWithResults}
                onToggle={toggleSourceFilter}
                onReset={resetSourceFilters}
              />

              {/* Sort */}
              <div className="flex items-center gap-2 bg-bg-tertiary px-3 py-1.5 rounded-lg" role="group" aria-label="Sort order">
                <span className="text-xs text-text-muted">{t('results.sort')}</span>
                <button
                  onClick={() => setSortBy('score')}
                  aria-pressed={sortBy === 'score'}
                  className={`px-2 py-1 text-xs rounded-lg transition-all ${
                    sortBy === 'score'
                      ? 'bg-white/10 text-white'
                      : 'text-text-muted hover:text-text-secondary'
                  }`}
                >
                  {t('results.score')}
                </button>
                <button
                  onClick={() => setSortBy('date')}
                  aria-pressed={sortBy === 'date'}
                  className={`px-2 py-1 text-xs rounded-lg transition-all ${
                    sortBy === 'date'
                      ? 'bg-white/10 text-white'
                      : 'text-text-muted hover:text-text-secondary'
                  }`}
                >
                  {t('results.recent')}
                </button>
              </div>

              {/* Relevance Toggle */}
              <button
                onClick={() => { setShowOnlyRelevant(!showOnlyRelevant); if (!showOnlyRelevant) setShowSavedOnly(false); }}
                aria-pressed={showOnlyRelevant}
                aria-label="Toggle relevant items only"
                className={`px-3 py-1.5 text-xs rounded-lg transition-all ${
                  showOnlyRelevant
                    ? 'bg-green-500/20 text-green-400 border border-green-500/30'
                    : 'bg-bg-tertiary text-text-muted hover:text-text-secondary'
                }`}
              >
                {showOnlyRelevant ? t('results.relevantOnly') : t('results.showAllItems')}
              </button>

              {/* Saved Items Toggle */}
              <button
                onClick={() => { setShowSavedOnly(!showSavedOnly); if (!showSavedOnly) setShowOnlyRelevant(false); }}
                aria-pressed={showSavedOnly}
                aria-label="Show saved items only"
                className={`px-3 py-1.5 text-xs rounded-lg transition-all ${
                  showSavedOnly
                    ? 'bg-blue-500/20 text-blue-400 border border-blue-500/30'
                    : 'bg-bg-tertiary text-text-muted hover:text-text-secondary'
                }`}
              >
                {t('results.saved')}
              </button>

              {/* Spacer */}
              <div className="flex-1" />

              {/* Batch Operations */}
              <div className="flex items-center gap-2">
                <button
                  onClick={() => dismissAllBelow(0.3)}
                  aria-label="Dismiss all items below 30% relevance"
                  className="px-3 py-1.5 text-xs bg-bg-tertiary text-text-muted rounded-lg hover:bg-red-500/10 hover:text-red-400 transition-all"
                  title="Dismiss all items below 30% relevance"
                >
                  x &lt;30%
                </button>
                <button
                  onClick={() => saveAllAbove(0.6)}
                  aria-label="Save all items above 60% relevance"
                  className="px-3 py-1.5 text-xs bg-bg-tertiary text-text-muted rounded-lg hover:bg-green-500/10 hover:text-green-400 transition-all"
                  title="Save all items above 60% relevance"
                >
                  + &gt;60%
                </button>
              </div>
            </div>
          )}
        </div>
        <div ref={parentRef} className="p-4 max-h-[calc(100vh-380px)] overflow-y-auto">
          {!state.analysisComplete ? (
            <div className="text-center py-16" role="status" aria-busy={state.loading}>
              {state.loading ? (
                <>
                  <div className="w-16 h-16 mx-auto mb-4 bg-orange-500/20 rounded-full flex items-center justify-center">
                    <div className="w-8 h-8 border-3 border-orange-500 border-t-transparent rounded-full animate-spin" />
                  </div>
                  <p className="text-lg text-white mb-2">{t('action.analyzing')}</p>
                  <p className="text-sm text-text-muted">{state.progressMessage}</p>
                  {state.progress > 0 && (
                    <div className="mt-4 max-w-xs mx-auto">
                      <div className="flex justify-between text-xs text-text-muted mb-1">
                        <span>{getStageLabel(state.progressStage)}</span>
                        <span>{Math.round(state.progress * 100)}%</span>
                      </div>
                      <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
                        <div
                          className="h-full bg-gradient-to-r from-orange-600 to-orange-400 transition-all duration-300 ease-out rounded-full"
                          style={{ width: `${state.progress * 100}%` }}
                        />
                      </div>
                    </div>
                  )}
                </>
              ) : (
                <>
                  <div className="w-16 h-16 mx-auto mb-4 rounded-xl border border-border/30 overflow-hidden" role="img" aria-label="4DA">
                    <game-tetrahedron style={{ width: '64px', height: '64px', display: 'block' }} />
                  </div>
                  <p className="text-lg text-white mb-2">{t('results.noResults')}</p>
                  <p className="text-sm text-text-muted mb-3">
                    {t('results.startAnalysis')}
                  </p>
                  <p className="text-xs text-text-muted/70 mb-5 max-w-md mx-auto leading-relaxed">
                    {t('results.howItWorks')}
                  </p>
                  <SmartEmptyState detectedStack={discoveredContext?.tech?.map(item => item.name) ?? []} />
                  <button
                    onClick={startAnalysis}
                    className="mt-5 px-6 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-lg hover:bg-orange-600 transition-colors"
                  >
                    {t('results.analyzeNow')}
                  </button>
                  <p className="text-xs text-text-muted mt-3">
                    or press <kbd className="px-1.5 py-0.5 bg-bg-tertiary rounded text-text-muted">R</kbd>
                  </p>
                  <p className="text-[10px] text-text-muted/50 mt-2">
                    {t('results.analyzeHint')}
                  </p>
                </>
              )}
            </div>
          ) : filteredResults.length === 0 ? (
            <div className="text-center py-16">
              <div className="w-16 h-16 mx-auto mb-4 bg-bg-tertiary rounded-full flex items-center justify-center">
                <svg className="w-7 h-7 text-text-muted" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
                </svg>
              </div>
              <p className="text-lg text-white mb-2">{t('results.noMatch')}</p>
              <p className="text-sm text-text-muted mb-5">
                {t('results.analyzedNoMatch', { count: state.relevanceResults.length })}
              </p>
              <div className="flex items-center justify-center gap-3">
                {showOnlyRelevant && (
                  <button
                    onClick={() => setShowOnlyRelevant(false)}
                    className="px-4 py-2 text-sm bg-bg-tertiary text-text-secondary rounded-lg border border-border hover:border-orange-500/30 transition-all"
                  >
                    {t('results.showAll')}
                  </button>
                )}
                {sourceFilters.size < ALL_SOURCE_IDS.size && (
                  <button
                    onClick={() => resetSourceFilters()}
                    className="px-4 py-2 text-sm bg-bg-tertiary text-text-secondary rounded-lg border border-border hover:border-orange-500/30 transition-all"
                  >
                    {t('results.clearSourceFilters')}
                  </button>
                )}
                {!showOnlyRelevant && sourceFilters.size === ALL_SOURCE_IDS.size && <p className="text-xs text-text-muted">Try a broader search query or add more interests in Settings</p>}
              </div>
              {state.nearMisses && state.nearMisses.length > 0 && (
                <div className="mt-8 mx-auto max-w-lg">
                  <p className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
                    {t('results.nearMissesTitle', 'Almost relevant')}
                  </p>
                  <div className="space-y-2">
                    {state.nearMisses.map((item) => (
                      <div
                        key={item.id}
                        className="flex items-center gap-3 px-3 py-2 bg-bg-secondary rounded-lg border border-border text-start"
                      >
                        <span className="text-xs font-mono text-text-muted shrink-0">
                          {Math.round(item.top_score * 100)}%
                        </span>
                        <span className="text-sm text-text-secondary truncate">
                          {getTranslated(String(item.id), item.title)}
                        </span>
                        <span className="text-[10px] text-text-muted shrink-0 ms-auto">
                          {item.source_type}
                        </span>
                      </div>
                    ))}
                  </div>
                  <p className="text-[10px] text-text-muted mt-2">
                    {t('results.nearMissesHint', 'These items scored close to your relevance threshold. Adjust interests in Settings to include them.')}
                  </p>
                </div>
              )}
            </div>
          ) : (
            <div
              role="listbox"
              aria-label={t('results.title')}
              aria-activedescendant={focusedIndex >= 0 && filteredResults[focusedIndex] ? `result-item-${filteredResults[focusedIndex]!.id}` : undefined}
              tabIndex={-1}
              style={{ height: `${virtualizer.getTotalSize()}px`, width: '100%', position: 'relative' }}
            >
              {virtualizer.getVirtualItems().map((virtualRow) => {
                const item = filteredResults[virtualRow.index]!;
                const idx = virtualRow.index;
                // Score group headers (only when sorting by score)
                let groupHeader: string | null = null;
                if (sortBy === 'score' && idx > 0) {
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
                    <div className="pb-3">
                      <ResultItem
                        item={item}
                        isExpanded={expandedItem === item.id}
                        isFocused={focusedIndex === idx}
                        isNew={newItemIds.has(item.id)}
                        onToggleExpand={handleToggleExpand}
                        feedbackGiven={feedbackGiven}
                        onRecordInteraction={recordInteraction}
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
