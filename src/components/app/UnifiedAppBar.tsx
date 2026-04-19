// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, Suspense, lazy } from 'react';
import { useTranslation } from 'react-i18next';
import { BrandMark } from '../void-engine/BrandMark';
import { useVoidSignals } from '../../hooks/use-void-signals';
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
  const voidSignal = useVoidSignals();

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
        className="flex flex-wrap items-center gap-3 px-4 min-h-[4rem] py-2 bg-bg-primary border-b border-border"
        role="banner"
        aria-label={t('app.title')}
      >
        {/* Left: VoidEngine + status */}
        <div className="flex items-center gap-3 flex-shrink-0">
          <div className="w-9 h-9 flex items-center justify-center rounded-lg overflow-hidden" aria-hidden="true">
            <BrandMark signal={voidSignal} size={36} />
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

        {/* Spacer */}
        <div className="flex-1" />

        {/* Right: badges + actions */}
        <div className="flex items-center gap-2 flex-shrink-0">
          {/* Summary badge — single pill, no more rel/top redundancy */}
          {summaryBadges && isComplete && (
            <span
              className="inline-flex items-center gap-1.5 px-2 py-0.5 text-[10px] bg-bg-tertiary/60 text-text-secondary rounded font-mono border border-border"
              title={t('header.summaryTooltip', '{{rel}} relevant items this session · {{top}} top-scored', {
                rel: summaryBadges.relevantCount,
                top: summaryBadges.topCount,
              })}
            >
              <span className="text-green-400">{summaryBadges.relevantCount}</span>
              <span className="text-text-muted">relevant</span>
              {summaryBadges.topCount > 0 && (
                <>
                  <span className="text-text-muted">·</span>
                  <span className="text-orange-400">{summaryBadges.topCount}</span>
                  <span className="text-text-muted">top</span>
                </>
              )}
            </span>
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
              aria-label={t('status.keywordOnlyTooltip', 'Semantic scoring unavailable. Configure an AI provider in Settings for better results.')}
            >
              {t('status.keywordOnly', 'Keyword mode')}
            </button>
          )}
          {embeddingStatus === 'degraded' && (
            <button
              onClick={onOpenSettings}
              className="text-[10px] px-2 py-0.5 bg-amber-500/10 text-amber-300 rounded hover:bg-amber-500/20 transition-colors"
              title={t('status.degradedTooltip', 'Embeddings using fallback. Results may be less accurate.')}
              aria-label={t('status.degradedTooltip', 'Embeddings using fallback. Results may be less accurate.')}
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
