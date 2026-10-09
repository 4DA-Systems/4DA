// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useRef, useMemo, useCallback, useEffect, useState, type SyntheticEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { LoadingOrEmptyState } from './LoadingOrEmptyState';
import { NoResultsState } from './NoResultsState';
import { ContextPanel } from './context-panel';
import { ResultFiltersBar } from './search/ResultFiltersBar';
import { useTranslatedContent } from './ContentTranslationProvider';
import { ResultLaneList } from './ResultLaneList';
import { SignalLanes } from './SignalLanes';
import { useAppStore } from '../store';
import { useResultFilters } from '../hooks';
import {
  STACK_LANE_CAP, flattenVisible, locateInLanes, partitionLanes, visibleLaneItems,
} from './signals/signal-lanes';
import { useSignalDisplayOrder } from './signals/signal-display-order';
import type { FeedbackAction, SourceRelevance } from '../types';

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

  // The file list is read lazily on first open (it reads every file under the
  // context dirs). Until then the count is UNKNOWN, not zero — the summary
  // used to print "(0 files)" for a freshly scanned D:\4DA because nothing had
  // asked yet (fresh-profile E2E 2026-10-09).
  const [contextFilesLoaded, setContextFilesLoaded] = useState(false);
  const handleContextPanelToggle = useCallback((event: SyntheticEvent<HTMLDetailsElement>) => {
    if (!event.currentTarget.open || contextFilesLoaded) return;
    setContextFilesLoaded(true);
    void loadContextFiles();
  }, [loadContextFiles, contextFilesLoaded]);

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

  // Split the feed into lanes by evidence pool (grounding), not score band, when
  // ranking by relevance: score can't separate signal from noise — a stack item
  // and pure noise both score ~0.9; grounding can (see signals/signal-lanes.ts).
  // Cold-start (profileEmpty) and the non-score sorts keep the flat list.
  const lanesActive = sortBy === 'score' && !profileEmpty;
  const stackExpanded = useSignalDisplayOrder(s => s.stackExpanded);
  const worthExpanded = useSignalDisplayOrder(s => s.worthExpanded);
  const moreExpanded = useSignalDisplayOrder(s => s.moreExpanded);
  const setStackExpanded = useSignalDisplayOrder(s => s.setStackExpanded);
  const setWorthExpanded = useSignalDisplayOrder(s => s.setWorthExpanded);
  const setMoreExpanded = useSignalDisplayOrder(s => s.setMoreExpanded);
  const setVisible = useSignalDisplayOrder(s => s.setVisible);
  const lanes = useMemo(() => (lanesActive ? partitionLanes(filteredResults) : null), [lanesActive, filteredResults]);
  const visibleLanes = useMemo(
    () => (lanes ? visibleLaneItems(lanes, { stackExpanded, worthExpanded, moreExpanded }) : null),
    [lanes, stackExpanded, worthExpanded, moreExpanded],
  );
  const displayResults = useMemo(
    () => (visibleLanes ? flattenVisible(visibleLanes) : filteredResults),
    [visibleLanes, filteredResults],
  );

  // Keyboard shortcuts index into what is on screen, in on-screen order.
  useEffect(() => { setVisible(displayResults); }, [displayResults, setVisible]);
  useEffect(() => () => setVisible(null), [setVisible]);

  const parentRef = useRef<HTMLDivElement>(null);
  const [scrollTarget, setScrollTarget] = useState<{ id: number } | null>(null);
  const onRecordInteraction = useCallback(
    (itemId: number, actionType: FeedbackAction, item: SourceRelevance) => { void recordInteraction(itemId, actionType, item); },
    [recordInteraction],
  );

  const relevantCount = useMemo(() => filteredResults.filter(r => r.relevant).length, [filteredResults]);
  const topPicksCount = useMemo(() => filteredResults.filter(r => r.top_score >= 0.72).length, [filteredResults]);
  const criticalCount = useMemo(() => filteredResults.filter(r => r.is_critical_alert).length, [filteredResults]);
  const totalCount = state.relevanceResults.length;

  // Topic cluster detection: find where 2+ consecutive items share a primary_topic.
  // Disabled while lanes are active — lanes are the primary partition.
  const topicClusterStarts = useMemo(() => {
    if (sortBy !== 'score' || lanesActive) return new Map<number, string>();
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
  }, [filteredResults, sortBy, lanesActive]);

  // Deep-link from the command search: scroll to + expand a specific item.
  useEffect(() => {
    if (searchFocusItemId == null) return;
    // In a collapsed part of a lane — open it first; this effect re-runs.
    const loc = lanes ? locateInLanes(lanes, searchFocusItemId) : null;
    if (loc?.lane === 'stack' && loc.index >= STACK_LANE_CAP && !stackExpanded) { setStackExpanded(true); return; }
    if (loc?.lane === 'worth' && !worthExpanded) { setWorthExpanded(true); return; }
    if (loc?.lane === 'more' && !moreExpanded) { setMoreExpanded(true); return; }
    if (displayResults.some(r => r.id === searchFocusItemId)) {
      const id = searchFocusItemId;
      setScrollTarget({ id });
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
  }, [searchFocusItemId, lanes, stackExpanded, worthExpanded, moreExpanded, setStackExpanded, setWorthExpanded, setMoreExpanded, displayResults, setExpandedItem, setSearchFocusItemId, showOnlyRelevant, setShowOnlyRelevant, state.relevanceResults]);

  useEffect(() => {
    const items = [
      ...filteredResults.map((r) => ({ id: String(r.id), text: r.title })),
      ...(state.nearMisses ?? []).map((r) => ({ id: String(r.id), text: r.title })),
    ];
    if (items.length > 0) requestTranslation(items);
  }, [filteredResults, state.nearMisses, requestTranslation]);
  const sourcesWithResults = useMemo(() => new Set(state.relevanceResults.map(r => r.source_type || 'hackernews')), [state.relevanceResults]);

  const listProps = {
    scrollElementRef: parentRef,
    focusedIndex,
    expandedItem,
    feedbackGiven,
    onToggleExpand: handleToggleExpand,
    onRecordInteraction,
    comparePool: filteredResults,
    scrollTarget,
  };

  // Flat list headers. Lanes cover every relevance-sorted view with a profile,
  // so the flat list is either cold start (one honest header + topic clusters)
  // or a non-score sort (no headers).
  const renderFlatPrefix = (_item: SourceRelevance, idx: number) => (
    <>
      {sortBy === 'score' && profileEmpty && idx === 0 && (
        <div className="flex items-center gap-3 mb-3 mt-2 first:mt-0">
          <span className="text-xs font-medium px-2 py-1 rounded-lg bg-gray-500/10 text-text-muted">
            {t('results.freshPicksGroup', 'Fresh picks — not yet personalized')}
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
    </>
  );

  return (
    <div className="space-y-6">
      {/* Context Files Panel (collapsible) */}
      <details
        className="bg-bg-secondary rounded-lg border border-border"
        onToggle={handleContextPanelToggle}
      >
        <summary className="px-5 py-3 text-xs text-text-muted cursor-pointer hover:text-text-secondary">
          {(contextFilesLoaded && !state.loading) || state.contextFiles.length > 0
            ? t('results.contextFilesCount', { count: state.contextFiles.length })
            : t('results.contextFiles')}
        </summary>
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
          ) : visibleLanes && lanes ? (
            <SignalLanes
              lanes={lanes}
              visible={visibleLanes}
              stackExpanded={stackExpanded}
              worthExpanded={worthExpanded}
              moreExpanded={moreExpanded}
              onToggleStack={() => setStackExpanded(!stackExpanded)}
              onToggleWorth={() => setWorthExpanded(!worthExpanded)}
              onToggleMore={() => setMoreExpanded(!moreExpanded)}
              {...listProps}
            />
          ) : (
            <ResultLaneList
              {...listProps}
              items={filteredResults}
              indexOffset={0}
              ariaLabel={t('results.title')}
              renderPrefix={renderFlatPrefix}
            />
          )}
        </div>
      </section>
    </div>
  );
}
