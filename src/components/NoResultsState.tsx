// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';

import { getRelevancePresentation } from '../utils/score';
import { ALL_SOURCE_IDS } from '../config/sources';

import type { SourceRelevance } from '../types';

interface NoResultsStateProps {
  totalAnalyzed: number;
  showOnlyRelevant: boolean;
  sourceFilters: Set<string>;
  nearMisses: SourceRelevance[] | null;
  setShowOnlyRelevant: (value: boolean) => void;
  resetSourceFilters: () => void;
  getTranslated: (id: string, fallback: string) => string;
}

export function NoResultsState({
  totalAnalyzed,
  showOnlyRelevant,
  sourceFilters,
  nearMisses,
  setShowOnlyRelevant,
  resetSourceFilters,
  getTranslated,
}: NoResultsStateProps) {
  const { t } = useTranslation();

  return (
    <div className="text-center py-16">
      <div className="w-16 h-16 mx-auto mb-4 bg-bg-tertiary rounded-full flex items-center justify-center">
        <svg className="w-7 h-7 text-text-muted" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
        </svg>
      </div>
      <p className="text-lg text-text-primary mb-2">{t('results.noMatch')}</p>
      <p className="text-sm text-text-muted mb-5">
        {t('results.analyzedNoMatch', { count: totalAnalyzed })}
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
        {/* eslint-disable-next-line i18next/no-literal-string */}
        {!showOnlyRelevant && sourceFilters.size === ALL_SOURCE_IDS.size && <p className="text-xs text-text-muted">Try a broader search query or add more interests in Settings</p>}
      </div>
      {nearMisses && nearMisses.length > 0 && (
        <section className="mt-8 mx-auto max-w-lg" aria-label={t('results.nearMissesTitle', 'Almost relevant')}>
          <p className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            {t('results.nearMissesTitle', 'Almost relevant')}
          </p>
          <div className="space-y-2">
            {nearMisses.map((item) => (
              <div
                key={item.id}
                className="flex items-center gap-3 px-3 py-2 bg-bg-secondary rounded-lg border border-border text-start"
              >
                <span className={`text-[10px] font-medium uppercase tracking-wider shrink-0 ${getRelevancePresentation(item.top_score).colorClass}`}>
                  {t(getRelevancePresentation(item.top_score).labelKey)}
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
        </section>
      )}
    </div>
  );
}
