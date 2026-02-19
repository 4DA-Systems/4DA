import { useRef } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useShallow } from 'zustand/react/shallow';
import { ResultItem } from './ResultItem';
import { getStageLabel } from '../utils/score';
import { getSourceLabel } from '../config/sources';
import { useAppStore } from '../store';
import { useResultFilters } from '../hooks';

interface ResultsViewProps {
  newItemIds: Set<number>;
  focusedIndex: number;
  renderLimit: number;
  setRenderLimit: Dispatch<SetStateAction<number>>;
}

export function ResultsView({
  newItemIds,
  focusedIndex,
  renderLimit: _renderLimit,
  setRenderLimit: _setRenderLimit,
}: ResultsViewProps) {
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
  const loadContextFiles = useAppStore(s => s.loadContextFiles);
  const clearContext = useAppStore(s => s.clearContext);
  const indexContext = useAppStore(s => s.indexContext);
  const recordInteraction = useAppStore(s => s.recordInteraction);

  // Filters (zero-param — reads from store internally)
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

  return (
    <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
      {/* Context Files Panel */}
      <section className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
        <div className="px-5 py-4 border-b border-border flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
              <span className="text-gray-500">F</span>
            </div>
            <div>
              <h2 className="font-medium text-white">Context</h2>
              <p className="text-xs text-gray-500">{state.contextFiles.length} files indexed</p>
            </div>
          </div>
          <div className="flex gap-2">
            <button
              onClick={loadContextFiles}
              className="w-8 h-8 flex items-center justify-center text-sm bg-bg-tertiary text-gray-400 rounded-lg hover:bg-border hover:text-white transition-all"
              title="Reload files"
            >
              R
            </button>
            {state.contextFiles.length > 0 && (
              <>
                <button
                  onClick={indexContext}
                  disabled={state.loading}
                  className="px-3 py-1.5 text-xs bg-green-500/10 text-green-400 border border-green-500/30 rounded-lg hover:bg-green-500/20 transition-all disabled:opacity-50"
                  title="Index files"
                >
                  Index
                </button>
                <button
                  onClick={clearContext}
                  className="px-3 py-1.5 text-xs bg-red-500/10 text-red-400 border border-red-500/30 rounded-lg hover:bg-red-500/20 transition-all"
                  title="Clear"
                >
                  Clear
                </button>
              </>
            )}
          </div>
        </div>
        <div className="p-4 max-h-[calc(100vh-320px)] overflow-y-auto">
          {state.contextFiles.length === 0 ? (
            <div className="text-center py-8 px-4">
              <div className="w-12 h-12 mx-auto mb-3 bg-bg-tertiary rounded-full flex items-center justify-center">
                <svg className="w-5 h-5 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                </svg>
              </div>
              <p className="text-gray-400 text-sm mb-1">Context auto-discovered</p>
              <p className="text-xs text-gray-600">4DA scans your projects automatically. Customize scan paths in Settings.</p>
            </div>
          ) : (
            <ul className="space-y-2">
              {state.contextFiles.map((file) => (
                <li
                  key={file.path}
                  className="px-3 py-2 bg-bg-tertiary rounded-lg border border-border hover:border-orange-500/30 transition-all"
                >
                  <div className="font-mono text-white text-sm truncate">
                    {file.path.split('/').pop()?.split('\\').pop()}
                  </div>
                  <div className="text-xs text-gray-500 mt-1">{file.lines} lines</div>
                </li>
              ))}
            </ul>
          )}

          {/* ACE Discovered Context */}
          {(discoveredContext.tech.length > 0 || discoveredContext.topics.length > 0) && (
            <div className="mt-4 pt-4 border-t border-border">
              <div className="text-xs text-gray-500 mb-3 flex items-center gap-2">
                <span>Auto-Discovered</span>
                <span className="px-1.5 py-0.5 text-[10px] bg-orange-500/20 text-orange-400 rounded" title="Auto Context Engine - score boost from your local project context">ACE</span>
              </div>
              {discoveredContext.tech.length > 0 && (
                <div className="mb-3">
                  <div className="flex flex-wrap gap-1.5">
                    {discoveredContext.tech.slice(0, 6).map((tech) => (
                      <span
                        key={tech.name}
                        className="px-2 py-1 text-[11px] bg-green-500/10 text-green-400 rounded-lg border border-green-500/20"
                        title={`${tech.category} - ${Math.round(tech.confidence * 100)}%`}
                      >
                        {tech.name}
                      </span>
                    ))}
                    {discoveredContext.tech.length > 6 && (
                      <span className="text-[11px] text-gray-500 self-center">+{discoveredContext.tech.length - 6}</span>
                    )}
                  </div>
                </div>
              )}
              {discoveredContext.topics.length > 0 && (
                <div className="flex flex-wrap gap-1.5">
                  {discoveredContext.topics.slice(0, 4).map((topic) => (
                    <span
                      key={topic}
                      className="px-2 py-1 text-[11px] bg-orange-500/10 text-orange-400 rounded-lg border border-orange-500/20"
                    >
                      {topic}
                    </span>
                  ))}
                  {discoveredContext.topics.length > 4 && (
                    <span className="text-[11px] text-gray-500 self-center">+{discoveredContext.topics.length - 4}</span>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </section>

      {/* Relevance Results Panel */}
      <section className="lg:col-span-2 bg-bg-secondary rounded-lg border border-border overflow-hidden">
        <div className="px-5 py-4 border-b border-border">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
                <span className="text-gray-500">R</span>
              </div>
              <div>
                <div className="flex items-center gap-2">
                  <h2 className="font-medium text-white">Results</h2>
                  {newItemIds.size > 0 && (
                    <span className="px-2 py-0.5 text-[10px] bg-blue-500/20 text-blue-400 rounded-full font-medium animate-pulse">
                      {newItemIds.size} new
                    </span>
                  )}
                </div>
                <p className="text-xs text-gray-500">
                  {state.analysisComplete
                    ? `${filteredResults.length} items - ${filteredResults.filter((r) => r.relevant).length} relevant`
                    : 'Click Analyze to find relevant content'}
                </p>
                {state.analysisComplete && filteredResults.length > 0 && (
                  <span className="text-xs text-text-muted">
                    {filteredResults.length} items
                  </span>
                )}
              </div>
            </div>
            {state.analysisComplete && (
              <div className="flex items-center gap-2">
                {filteredResults.filter((r) => r.top_score >= 0.72).length > 0 && (
                  <span className="text-xs px-2 py-1 bg-orange-500/10 text-orange-400 rounded-lg">
                    {filteredResults.filter((r) => r.top_score >= 0.72).length} top picks
                  </span>
                )}
                <span className="text-xs px-2 py-1 bg-green-500/10 text-green-400 rounded-lg">
                  {filteredResults.filter((r) => r.relevant).length} relevant
                </span>
              </div>
            )}
          </div>

          {/* Filter Bar */}
          {state.analysisComplete && (
            <div className="flex flex-wrap items-center gap-3 pt-3 border-t border-border" role="toolbar" aria-label="Filter and sort controls">
              {/* Search */}
              <div className="relative">
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search results..."
                  aria-label="Search results by keyword"
                  className="bg-bg-tertiary text-sm text-white placeholder-gray-500 rounded-lg pl-8 pr-3 py-1.5 w-48 border border-transparent focus:border-border focus:outline-none transition-all"
                />
                <svg className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
                {searchQuery && (
                  <button
                    onClick={() => setSearchQuery('')}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300"
                    aria-label="Clear search"
                  >
                    <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                    </svg>
                  </button>
                )}
              </div>

              {/* Source Filters */}
              <div className="flex items-center gap-2 bg-bg-tertiary px-3 py-1.5 rounded-lg flex-wrap" role="group" aria-label="Source filters">
                <span className="text-xs text-gray-500">Sources:</span>
                {[...new Set(state.relevanceResults.map(r => r.source_type || 'hackernews'))]
                  .sort((a, b) => getSourceLabel(a).localeCompare(getSourceLabel(b)))
                  .map(id => (
                    <button
                      key={id}
                      onClick={() => toggleSourceFilter(id)}
                      aria-pressed={sourceFilters.has(id)}
                      aria-label={`Filter ${getSourceLabel(id)} source`}
                      className={`px-2 py-1 text-xs rounded-lg transition-all ${
                        sourceFilters.has(id)
                          ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30'
                          : 'text-gray-500 hover:text-gray-300'
                      }`}
                    >
                      {getSourceLabel(id)}
                    </button>
                  ))}
              </div>

              {/* Sort */}
              <div className="flex items-center gap-2 bg-bg-tertiary px-3 py-1.5 rounded-lg" role="group" aria-label="Sort order">
                <span className="text-xs text-gray-500">Sort:</span>
                <button
                  onClick={() => setSortBy('score')}
                  aria-pressed={sortBy === 'score'}
                  className={`px-2 py-1 text-xs rounded-lg transition-all ${
                    sortBy === 'score'
                      ? 'bg-white/10 text-white'
                      : 'text-gray-500 hover:text-gray-300'
                  }`}
                >
                  Score
                </button>
                <button
                  onClick={() => setSortBy('date')}
                  aria-pressed={sortBy === 'date'}
                  className={`px-2 py-1 text-xs rounded-lg transition-all ${
                    sortBy === 'date'
                      ? 'bg-white/10 text-white'
                      : 'text-gray-500 hover:text-gray-300'
                  }`}
                >
                  Recent
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
                    : 'bg-bg-tertiary text-gray-500 hover:text-gray-300'
                }`}
              >
                {showOnlyRelevant ? 'Relevant only' : 'Show all'}
              </button>

              {/* Saved Items Toggle */}
              <button
                onClick={() => { setShowSavedOnly(!showSavedOnly); if (!showSavedOnly) setShowOnlyRelevant(false); }}
                aria-pressed={showSavedOnly}
                aria-label="Show saved items only"
                className={`px-3 py-1.5 text-xs rounded-lg transition-all ${
                  showSavedOnly
                    ? 'bg-blue-500/20 text-blue-400 border border-blue-500/30'
                    : 'bg-bg-tertiary text-gray-500 hover:text-gray-300'
                }`}
              >
                {showSavedOnly ? 'Saved' : 'Saved'}
              </button>

              {/* Spacer */}
              <div className="flex-1" />

              {/* Batch Operations */}
              <div className="flex items-center gap-2">
                <button
                  onClick={() => dismissAllBelow(0.3)}
                  className="px-3 py-1.5 text-xs bg-bg-tertiary text-gray-500 rounded-lg hover:bg-red-500/10 hover:text-red-400 transition-all"
                  title="Dismiss all items below 30% relevance"
                >
                  x &lt;30%
                </button>
                <button
                  onClick={() => saveAllAbove(0.6)}
                  className="px-3 py-1.5 text-xs bg-bg-tertiary text-gray-500 rounded-lg hover:bg-green-500/10 hover:text-green-400 transition-all"
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
            <div className="text-center py-16">
              {state.loading ? (
                <>
                  <div className="w-16 h-16 mx-auto mb-4 bg-orange-500/20 rounded-full flex items-center justify-center">
                    <div className="w-8 h-8 border-3 border-orange-500 border-t-transparent rounded-full animate-spin" />
                  </div>
                  <p className="text-lg text-white mb-2">Analyzing...</p>
                  <p className="text-sm text-gray-500">{state.progressMessage}</p>
                  {state.progress > 0 && (
                    <div className="mt-4 max-w-xs mx-auto">
                      <div className="flex justify-between text-xs text-gray-500 mb-1">
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
                  <div className="w-16 h-16 mx-auto mb-4 bg-bg-tertiary rounded-full flex items-center justify-center">
                    <svg className="w-7 h-7 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                    </svg>
                  </div>
                  <p className="text-lg text-white mb-2">No results yet</p>
                  <p className="text-sm text-gray-500 mb-5">
                    Run an analysis to discover relevant content from 7+ sources
                  </p>
                  <button
                    onClick={startAnalysis}
                    className="px-6 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-lg hover:bg-orange-600 transition-colors"
                  >
                    Analyze Now
                  </button>
                  <p className="text-xs text-gray-600 mt-3">
                    or press <kbd className="px-1.5 py-0.5 bg-bg-tertiary rounded text-gray-500">R</kbd>
                  </p>
                </>
              )}
            </div>
          ) : filteredResults.length === 0 ? (
            <div className="text-center py-16">
              <div className="w-16 h-16 mx-auto mb-4 bg-bg-tertiary rounded-full flex items-center justify-center">
                <svg className="w-7 h-7 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
                </svg>
              </div>
              <p className="text-lg text-white mb-2">No results match your filters</p>
              <p className="text-sm text-gray-500 mb-5">
                {state.relevanceResults.length} items analyzed, but none match current filters
              </p>
              <div className="flex items-center justify-center gap-3">
                {showOnlyRelevant && (
                  <button
                    onClick={() => setShowOnlyRelevant(false)}
                    className="px-4 py-2 text-sm bg-bg-tertiary text-gray-300 rounded-lg border border-border hover:border-orange-500/30 transition-all"
                  >
                    Show all items
                  </button>
                )}
                {sourceFilters.size > 0 && (
                  <button
                    onClick={() => sourceFilters.forEach(s => toggleSourceFilter(s))}
                    className="px-4 py-2 text-sm bg-bg-tertiary text-gray-300 rounded-lg border border-border hover:border-orange-500/30 transition-all"
                  >
                    Clear source filters
                  </button>
                )}
                {!showOnlyRelevant && sourceFilters.size === 0 && <p className="text-xs text-gray-600">Try a broader search query or add more interests in Settings</p>}
              </div>
            </div>
          ) : (
            <div style={{ height: `${virtualizer.getTotalSize()}px`, width: '100%', position: 'relative' }}>
              {virtualizer.getVirtualItems().map((virtualRow) => {
                const item = filteredResults[virtualRow.index];
                const idx = virtualRow.index;
                // Score group headers (only when sorting by score)
                let groupHeader: string | null = null;
                if (sortBy === 'score' && idx > 0) {
                  const prev = filteredResults[idx - 1];
                  if (prev.top_score >= 0.72 && item.top_score < 0.72) {
                    groupHeader = 'Relevant';
                  } else if (prev.top_score >= 0.50 && item.top_score < 0.50) {
                    groupHeader = 'Below Threshold';
                  }
                } else if (sortBy === 'score' && idx === 0 && item.top_score >= 0.72) {
                  groupHeader = 'Top Picks';
                } else if (sortBy === 'score' && idx === 0 && item.top_score >= 0.50) {
                  groupHeader = 'Relevant';
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
                          groupHeader === 'Top Picks' ? 'bg-orange-500/10 text-orange-400' :
                          groupHeader === 'Relevant' ? 'bg-green-500/10 text-green-400' :
                          'bg-gray-500/10 text-gray-500'
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
                        onToggleExpand={() => setExpandedItem(expandedItem === item.id ? null : item.id)}
                        feedbackGiven={feedbackGiven}
                        onRecordInteraction={recordInteraction}
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
