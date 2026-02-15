import type { Dispatch, SetStateAction } from 'react';
import { ResultItem } from './ResultItem';
import { getStageLabel } from '../utils/score';
import { getSourceLabel } from '../config/sources';
import type { SourceRelevance, FeedbackAction, FeedbackGiven, ContextFile } from '../types';

interface ResultsViewProps {
  state: {
    contextFiles: ContextFile[];
    relevanceResults: SourceRelevance[];
    analysisComplete: boolean;
    loading: boolean;
    progressMessage: string;
    progress: number;
    progressStage: string;
  };
  filteredResults: SourceRelevance[];
  feedbackGiven: FeedbackGiven;
  discoveredContext: { tech: Array<{ name: string; category: string; confidence: number }>; topics: string[] };
  expandedItem: number | null;
  setExpandedItem: (id: number | null) => void;
  loadContextFiles: () => void;
  clearContext: () => void;
  indexContext: () => void;
  recordInteraction: (itemId: number, action: FeedbackAction, item: SourceRelevance) => void;
  newItemIds: Set<number>;
  focusedIndex: number;
  // Filter controls
  sourceFilters: Set<string>;
  sortBy: 'score' | 'date';
  showOnlyRelevant: boolean;
  showSavedOnly: boolean;
  searchQuery: string;
  setSortBy: (sort: 'score' | 'date') => void;
  setShowOnlyRelevant: (show: boolean) => void;
  setShowSavedOnly: (show: boolean) => void;
  setSearchQuery: (q: string) => void;
  toggleSourceFilter: (source: string) => void;
  dismissAllBelow: (threshold: number) => void;
  saveAllAbove: (threshold: number) => void;
  renderLimit: number;
  setRenderLimit: Dispatch<SetStateAction<number>>;
}

export function ResultsView({
  state,
  filteredResults,
  feedbackGiven,
  discoveredContext,
  expandedItem,
  setExpandedItem,
  loadContextFiles,
  clearContext,
  indexContext,
  recordInteraction,
  newItemIds,
  focusedIndex,
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
  dismissAllBelow,
  saveAllAbove,
  renderLimit,
  setRenderLimit,
}: ResultsViewProps) {
  const visibleResults = filteredResults.slice(0, renderLimit);

  return (
    <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
      {/* Context Files Panel */}
      <section className="bg-[#141414] rounded-lg border border-[#2A2A2A] overflow-hidden">
        <div className="px-5 py-4 border-b border-[#2A2A2A] flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 bg-[#1F1F1F] rounded-lg flex items-center justify-center">
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
              className="w-8 h-8 flex items-center justify-center text-sm bg-[#1F1F1F] text-gray-400 rounded-lg hover:bg-[#2A2A2A] hover:text-white transition-all"
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
              <div className="w-12 h-12 mx-auto mb-3 bg-[#1F1F1F] rounded-full flex items-center justify-center">
                <span className="text-2xl text-gray-500">F</span>
              </div>
              <p className="text-gray-400 text-sm mb-1">No context files yet</p>
              <p className="text-xs text-gray-600">Add files to your context directory to enable personalized analysis.</p>
            </div>
          ) : (
            <ul className="space-y-2">
              {state.contextFiles.map((file) => (
                <li
                  key={file.path}
                  className="px-3 py-2 bg-[#1F1F1F] rounded-lg border border-[#2A2A2A] hover:border-orange-500/30 transition-all"
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
            <div className="mt-4 pt-4 border-t border-[#2A2A2A]">
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
      <section className="lg:col-span-2 bg-[#141414] rounded-lg border border-[#2A2A2A] overflow-hidden">
        <div className="px-5 py-4 border-b border-[#2A2A2A]">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 bg-[#1F1F1F] rounded-lg flex items-center justify-center">
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
                {state.analysisComplete && filteredResults.length > renderLimit && (
                  <span className="text-xs text-[#666666]">
                    Showing {Math.min(renderLimit, filteredResults.length)} of {filteredResults.length}
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
            <div className="flex flex-wrap items-center gap-3 pt-3 border-t border-[#2A2A2A]" role="toolbar" aria-label="Filter and sort controls">
              {/* Search */}
              <div className="relative">
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search results..."
                  aria-label="Search results by keyword"
                  className="bg-[#1F1F1F] text-sm text-white placeholder-gray-500 rounded-lg pl-8 pr-3 py-1.5 w-48 border border-transparent focus:border-[#2A2A2A] focus:outline-none transition-all"
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
              <div className="flex items-center gap-2 bg-[#1F1F1F] px-3 py-1.5 rounded-lg flex-wrap" role="group" aria-label="Source filters">
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
              <div className="flex items-center gap-2 bg-[#1F1F1F] px-3 py-1.5 rounded-lg" role="group" aria-label="Sort order">
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
                    : 'bg-[#1F1F1F] text-gray-500 hover:text-gray-300'
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
                    : 'bg-[#1F1F1F] text-gray-500 hover:text-gray-300'
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
                  className="px-3 py-1.5 text-xs bg-[#1F1F1F] text-gray-500 rounded-lg hover:bg-red-500/10 hover:text-red-400 transition-all"
                  title="Dismiss all items below 30% relevance"
                >
                  x &lt;30%
                </button>
                <button
                  onClick={() => saveAllAbove(0.6)}
                  className="px-3 py-1.5 text-xs bg-[#1F1F1F] text-gray-500 rounded-lg hover:bg-green-500/10 hover:text-green-400 transition-all"
                  title="Save all items above 60% relevance"
                >
                  + &gt;60%
                </button>
              </div>
            </div>
          )}
        </div>
        <div className="p-4 max-h-[calc(100vh-380px)] overflow-y-auto">
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
                      <div className="w-full h-2 bg-[#1F1F1F] rounded-full overflow-hidden">
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
                  <div className="w-16 h-16 mx-auto mb-4 bg-[#1F1F1F] rounded-full flex items-center justify-center">
                    <span className="text-3xl text-gray-500">?</span>
                  </div>
                  <p className="text-lg text-white mb-2">Ready to search</p>
                  <p className="text-sm text-gray-500">
                    Click <span className="text-orange-400">Analyze</span> to find relevant content
                  </p>
                </>
              )}
            </div>
          ) : filteredResults.length === 0 ? (
            <div className="text-center py-16">
              <div className="w-16 h-16 mx-auto mb-4 bg-[#1F1F1F] rounded-full flex items-center justify-center">
                <span className="text-3xl text-gray-500">--</span>
              </div>
              <p className="text-lg text-white mb-2">No results match</p>
              <p className="text-sm text-gray-500">
                Try enabling more sources or showing all items
              </p>
            </div>
          ) : (
            <>
              <ul className="space-y-3">
                {visibleResults.map((item, idx) => {
                  // Score group headers (only when sorting by score)
                  let groupHeader: string | null = null;
                  if (sortBy === 'score' && idx > 0) {
                    const prev = visibleResults[idx - 1];
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
                    <li key={item.id}>
                      {groupHeader && (
                        <div className="flex items-center gap-3 mb-3 mt-2 first:mt-0">
                          <span className={`text-xs font-medium px-2 py-1 rounded-lg ${
                            groupHeader === 'Top Picks' ? 'bg-orange-500/10 text-orange-400' :
                            groupHeader === 'Relevant' ? 'bg-green-500/10 text-green-400' :
                            'bg-gray-500/10 text-gray-500'
                          }`}>
                            {groupHeader}
                          </span>
                          <div className="flex-1 h-px bg-[#2A2A2A]" />
                        </div>
                      )}
                      <ResultItem
                        item={item}
                        isExpanded={expandedItem === item.id}
                        isFocused={focusedIndex === idx}
                        isNew={newItemIds.has(item.id)}
                        onToggleExpand={() => setExpandedItem(expandedItem === item.id ? null : item.id)}
                        feedbackGiven={feedbackGiven}
                        onRecordInteraction={recordInteraction}
                      />
                    </li>
                  );
                })}
              </ul>
              {filteredResults.length > renderLimit && (
                <button
                  onClick={() => setRenderLimit(prev => prev + 50)}
                  className="w-full mt-3 py-2 text-sm text-orange-400 bg-[#1A1A1A] border border-[#2A2A2A] rounded-lg hover:bg-[#1F1F1F] transition-colors"
                >
                  Show more ({filteredResults.length - renderLimit} remaining)
                </button>
              )}
            </>
          )}
        </div>
      </section>
    </div>
  );
}
