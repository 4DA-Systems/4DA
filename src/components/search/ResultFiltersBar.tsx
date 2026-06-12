// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';
import { SourceCategoryFilter } from '../SourceCategoryFilter';

type SortMode = 'score' | 'date' | 'priority' | 'applicability' | 'freshness' | 'urgency';

export interface ResultFiltersBarProps {
  searchQuery: string;
  setSearchQuery: (q: string) => void;
  sourceFilters: Set<string>;
  sourcesWithResults: Set<string>;
  toggleSourceFilter: (id: string) => void;
  resetSourceFilters: () => void;
  sortBy: SortMode;
  setSortBy: (mode: SortMode) => void;
  showOnlyRelevant: boolean;
  setShowOnlyRelevant: (v: boolean) => void;
  showSavedOnly: boolean;
  setShowSavedOnly: (v: boolean) => void;
  dismissAllBelow: (threshold: number) => Promise<void>;
  saveAllAbove: (threshold: number) => Promise<void>;
}

export function ResultFiltersBar({
  searchQuery,
  setSearchQuery,
  sourceFilters,
  sourcesWithResults,
  toggleSourceFilter,
  resetSourceFilters,
  sortBy,
  setSortBy,
  showOnlyRelevant,
  setShowOnlyRelevant,
  showSavedOnly,
  setShowSavedOnly,
  dismissAllBelow,
  saveAllAbove,
}: ResultFiltersBarProps) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col sm:flex-row flex-wrap items-start sm:items-center gap-3 pt-3 border-t border-border" role="toolbar" aria-label="Filter and sort controls">
      {/* Search */}
      <div className="relative w-full sm:w-auto">
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder={t('results.searchPlaceholder')}
          aria-label="Search results by keyword"
          className="bg-bg-tertiary text-sm text-text-primary placeholder-gray-500 rounded-lg ps-8 pe-3 py-1.5 w-full sm:w-48 border border-transparent focus:border-border focus:outline-none transition-all"
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
        {([
          ['score', t('results.score')] as const,
          ['date', t('results.recent')] as const,
          ['urgency', t('results.sortUrgency', 'Urgency')] as const,
          ['priority', t('results.sortPriority', 'Priority')] as const,
          ['applicability', t('results.sortApplicability', 'Applicability')] as const,
          ['freshness', t('results.sortFreshness', 'Freshness')] as const,
        ]).map(([mode, label]) => (
          <button
            key={mode}
            onClick={() => setSortBy(mode)}
            aria-pressed={sortBy === mode}
            className={`px-2 py-1 text-xs rounded-lg transition-all ${
              sortBy === mode
                ? 'bg-text-primary/10 text-text-primary'
                : 'text-text-muted hover:text-text-secondary'
            }`}
          >
            {label}
          </button>
        ))}
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
      {/* eslint-disable i18next/no-literal-string */}
      <div className="flex items-center gap-2">
        <button
          onClick={() => { void dismissAllBelow(0.3); }}
          aria-label="Dismiss all items below 30% relevance"
          className="px-3 py-1.5 text-xs bg-bg-tertiary text-text-muted rounded-lg hover:bg-red-500/10 hover:text-red-400 transition-all"
          title="Dismiss all items below 30% relevance"
        >
          Hide low-signal
        </button>
        <button
          onClick={() => { void saveAllAbove(0.6); }}
          aria-label="Save all items above 60% relevance"
          className="px-3 py-1.5 text-xs bg-bg-tertiary text-text-muted rounded-lg hover:bg-green-500/10 hover:text-green-400 transition-all"
          title="Save all items above 60% relevance"
        >
          Save strong matches
        </button>
      </div>
      {/* eslint-enable i18next/no-literal-string */}
    </div>
  );
}
