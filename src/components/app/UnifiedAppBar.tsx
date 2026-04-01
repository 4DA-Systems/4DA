// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useState, useCallback, Suspense, lazy } from 'react';
import { useTranslation } from 'react-i18next';
import { VoidEngine } from '../void-engine/VoidEngine';
import { OllamaStatus } from '../OllamaStatus';
import { SystemHealthDot } from '../SystemHealthDot';
import { cmd } from '../../lib/commands';
import type { AppState } from '../../store/types';

const ProValueBadge = lazy(() => import('../ProValueBadge').then(m => ({ default: m.ProValueBadge })));

// ============================================================================
// Types
// ============================================================================

interface UnifiedAppBarProps {
  state: AppState;
  monitoring: { enabled: boolean } | null;
  settingsFormProvider: string;
  isPro: boolean;
  tier: string;
  summaryBadges: { relevantCount: number; topCount: number; total: number } | null;
  aiBriefing: { error: string | null };
  onAnalyze: () => void;
  onOpenSettings: () => void;
  analysisPulse: boolean;
  embeddingStatus?: 'active' | 'degraded' | 'unavailable';
}

// ============================================================================
// Component
// ============================================================================

export const UnifiedAppBar = memo(function UnifiedAppBar({
  state,
  monitoring,
  settingsFormProvider,
  isPro,
  tier,
  summaryBadges,
  aiBriefing,
  onAnalyze,
  onOpenSettings,
  analysisPulse,
  embeddingStatus,
}: UnifiedAppBarProps) {
  const { t } = useTranslation();
  const [searchQuery, setSearchQuery] = useState('');
  const [searchLoading, setSearchLoading] = useState(false);

  const handleSearchSubmit = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();
    if (!searchQuery.trim() || searchLoading) return;
    setSearchLoading(true);
    try {
      await cmd('synthesize_search', { queryText: searchQuery.trim() });
    } catch {
      // Search errors handled by the NaturalLanguageSearch panel
    } finally {
      setSearchLoading(false);
    }
  }, [searchQuery, searchLoading]);

  const isLoading = state.loading;
  const isComplete = state.analysisComplete && !isLoading;

  // Status dot color
  const statusDotColor = isComplete
    ? 'bg-green-500'
    : isLoading
      ? 'bg-orange-400 animate-pulse'
      : 'bg-gray-500';

  // Compact status text
  const statusText = isLoading
    ? t('action.analyzing')
    : isComplete && summaryBadges
      ? t('action.analysisComplete')
      : t('action.ready');

  return (
    <div className="mb-3">
      {/* Main bar */}
      <div
        className="flex items-center gap-3 px-4 h-16 bg-bg-primary border-b border-border"
        role="banner"
        aria-label={t('app.title')}
      >
        {/* Left: VoidEngine + status */}
        <div className="flex items-center gap-3 flex-shrink-0">
          <div className="w-9 h-9 flex items-center justify-center rounded-lg overflow-hidden">
            <VoidEngine size={36} variant="pentachoron" />
          </div>
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full flex-shrink-0 ${statusDotColor}`} aria-hidden="true" />
            <span className="text-xs text-text-secondary whitespace-nowrap" role="status" aria-live="polite">
              {statusText}
            </span>
            {monitoring?.enabled && (
              <span className="text-[10px] text-green-400 font-medium uppercase tracking-wider">
                {t('header.live')}
              </span>
            )}
          </div>
        </div>

        {/* Divider */}
        <div className="w-px h-6 bg-border flex-shrink-0" aria-hidden="true" />

        {/* Center: Search input */}
        <form onSubmit={handleSearchSubmit} className="flex-1 min-w-0 flex items-center gap-1" role="search">
          <input
            type="text"
            data-search-input
            placeholder={t('search.placeholder')}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full px-3 py-1.5 text-sm bg-bg-secondary border border-border rounded-md text-white placeholder:text-text-muted focus:outline-none focus:border-orange-500/40 transition-colors"
            aria-label={t('search.placeholder')}
          />
          <button
            type="submit"
            disabled={!searchQuery.trim() || searchLoading}
            className={`px-2 py-1.5 rounded-md transition-colors ${
              searchQuery.trim() && !searchLoading
                ? 'text-accent-gold hover:bg-accent-gold/10'
                : 'text-text-muted/30 cursor-not-allowed'
            }`}
            aria-label="Search"
          >
            {searchLoading ? (
              <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
            ) : (
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14 5l7 7m0 0l-7 7m7-7H3" />
              </svg>
            )}
          </button>
        </form>

        {/* Divider */}
        <div className="w-px h-6 bg-border flex-shrink-0" aria-hidden="true" />

        {/* Right: badges + actions */}
        <div className="flex items-center gap-2 flex-shrink-0">
          {/* Summary badges */}
          {summaryBadges && isComplete && (
            <div className="flex items-center gap-1">
              <span className="px-1.5 py-0.5 text-[10px] bg-green-500/10 text-green-400 rounded font-mono">
                {summaryBadges.relevantCount} rel
              </span>
              {summaryBadges.topCount > 0 && (
                <span className="px-1.5 py-0.5 text-[10px] bg-orange-500/10 text-orange-400 rounded font-mono">
                  {summaryBadges.topCount} top
                </span>
              )}
            </div>
          )}

          {/* Analyze button */}
          <button
            onClick={onAnalyze}
            disabled={isLoading}
            aria-label={t('action.runAnalysis', 'Run analysis')}
            className={`w-8 h-8 flex items-center justify-center rounded-md transition-all ${
              isLoading
                ? 'bg-orange-500/20 text-orange-400 cursor-not-allowed'
                : 'bg-orange-500 text-white hover:bg-orange-600 hover:scale-105 active:scale-95'
            }`}
            style={analysisPulse ? {
              boxShadow: '0 0 12px rgba(249, 115, 22, 0.6)',
              transition: 'box-shadow 0.5s ease-out',
            } : {
              boxShadow: 'none',
              transition: 'box-shadow 0.5s ease-out',
            }}
          >
            {isLoading ? (
              <div className="w-4 h-4 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" aria-hidden="true" />
            ) : (
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
                <path d="M13 7A6 6 0 1 1 1 7a6 6 0 0 1 12 0Z" stroke="currentColor" strokeWidth="1.5" />
                <path d="M7 4v6M4 7h6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            )}
          </button>

          {/* Cancel button during analysis */}
          {isLoading && (
            <button
              onClick={() => cmd('cancel_analysis')}
              className="w-8 h-8 flex items-center justify-center rounded-md bg-bg-tertiary text-red-400 border border-red-500/30 hover:bg-red-500/10 transition-all"
              aria-label={t('action.cancelAnalysis', 'Cancel analysis')}
            >
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
                <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </button>
          )}

          {/* Persistent keyword-only mode indicator */}
          {embeddingStatus === 'unavailable' && (
            <button
              onClick={onOpenSettings}
              className="text-[10px] px-2 py-0.5 bg-amber-500/20 text-amber-400 rounded hover:bg-amber-500/30 transition-colors"
              title={t('status.keywordOnlyTooltip', 'Semantic scoring unavailable. Configure an AI provider in Settings for better results.')}
            >
              {t('status.keywordOnly', 'Keyword mode')}
            </button>
          )}
          {embeddingStatus === 'degraded' && (
            <button
              onClick={onOpenSettings}
              className="text-[10px] px-2 py-0.5 bg-amber-500/10 text-amber-300 rounded hover:bg-amber-500/20 transition-colors"
              title={t('status.degradedTooltip', 'Embeddings using fallback. Results may be less accurate.')}
            >
              {t('status.degraded', 'Limited')}
            </button>
          )}
          <OllamaStatus provider={settingsFormProvider} />
          <Suspense fallback={null}><ProValueBadge /></Suspense>

          {/* Tier badge */}
          <span className={`px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider rounded ${
            isPro
              ? 'bg-accent-gold/20 text-accent-gold border border-accent-gold/30'
              : 'bg-bg-tertiary text-gray-400 border border-border'
          }`}>
            {tier}
          </span>

          {/* System health — shows amber/red dot if issues detected */}
          <SystemHealthDot onClick={onOpenSettings} />

          {/* Settings gear */}
          <button
            data-settings-trigger
            onClick={onOpenSettings}
            className="w-8 h-8 flex items-center justify-center rounded-md bg-bg-secondary text-text-secondary border border-border hover:bg-bg-tertiary hover:border-orange-500/30 transition-all"
            aria-label={t('header.settings')}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
              <path d="M7 9a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z" stroke="currentColor" strokeWidth="1.2" />
              <path d="M11.4 8.6a1 1 0 0 0 .2 1.1l.04.04a1.2 1.2 0 1 1-1.7 1.7l-.04-.04a1 1 0 0 0-1.1-.2 1 1 0 0 0-.6.92v.12a1.2 1.2 0 1 1-2.4 0v-.06a1 1 0 0 0-.66-.92 1 1 0 0 0-1.1.2l-.04.04a1.2 1.2 0 1 1-1.7-1.7l.04-.04a1 1 0 0 0 .2-1.1 1 1 0 0 0-.92-.6h-.12a1.2 1.2 0 1 1 0-2.4h.06a1 1 0 0 0 .92-.66 1 1 0 0 0-.2-1.1l-.04-.04A1.2 1.2 0 1 1 4.08 2l.04.04a1 1 0 0 0 1.1.2h.04a1 1 0 0 0 .6-.92v-.12a1.2 1.2 0 1 1 2.4 0v.06a1 1 0 0 0 .6.92 1 1 0 0 0 1.1-.2l.04-.04a1.2 1.2 0 1 1 1.7 1.7l-.04.04a1 1 0 0 0-.2 1.1v.04a1 1 0 0 0 .92.6h.12a1.2 1.2 0 0 1 0 2.4h-.06a1 1 0 0 0-.92.6Z" stroke="currentColor" strokeWidth="1.2" />
            </svg>
          </button>
        </div>
      </div>

      {/* Progress line — 2px, full width, only during analysis */}
      {isLoading && state.progress > 0 && (
        <div className="h-0.5 w-full bg-bg-secondary">
          <div
            role="progressbar"
            aria-valuenow={Math.round(state.progress * 100)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={t('action.analyzing')}
            className="h-full bg-gradient-to-r from-orange-600 to-orange-400 transition-all duration-300 ease-out"
            style={{ width: `${state.progress * 100}%` }}
          />
        </div>
      )}

      {/* AI Briefing Error */}
      {aiBriefing.error && (
        <div role="alert" className="mx-4 mt-2 px-3 py-2 bg-red-900/20 border border-red-500/30 rounded-md text-red-300 text-xs flex items-center gap-2">
          <span aria-hidden="true">!</span>
          {aiBriefing.error}
        </div>
      )}
    </div>
  );
});
