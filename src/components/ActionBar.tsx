import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { AudioBriefing } from './AudioBriefing';
import { ContextHandoff } from './ContextHandoff';
import { getStageLabel } from '../utils/score';
import { useAppStore } from '../store';
import { registerGameComponent } from '../lib/game-components';
import type { Settings, SourceRelevance } from '../types';

interface ActionBarProps {
  state: {
    loading: boolean;
    analysisComplete: boolean;
    status: string;
    lastAnalyzedAt: Date | null;
    progress: number;
    progressStage: string;
    relevanceResults: SourceRelevance[];
  };
  settings: Settings | null;
  aiBriefing: { loading: boolean; error: string | null };
  autoBriefingEnabled: boolean;
  summaryBadges: { relevantCount: number; topCount: number; total: number } | null;
  onAnalyze: () => void;
  onGenerateBriefing: () => void;
  onToggleAutoBriefing: () => void;
  onToast: (type: 'success' | 'error', message: string) => void;
}

function getRefreshLabel(state: ActionBarProps['state'], briefingLoading: boolean, t: (key: string) => string): string {
  if (state.loading) return t('action.analyzing');
  if (briefingLoading) return t('action.briefing');
  if (state.lastAnalyzedAt && Date.now() - state.lastAnalyzedAt.getTime() < 600_000) {
    return t('action.upToDate');
  }
  return t('action.refresh');
}

export function ActionBar({
  state,
  settings,
  aiBriefing,
  autoBriefingEnabled,
  summaryBadges,
  onAnalyze,
  onGenerateBriefing,
  onToggleAutoBriefing,
  onToast,
}: ActionBarProps) {
  const { t } = useTranslation();
  const [overflowOpen, setOverflowOpen] = useState(false);
  const overflowRef = useRef<HTMLDivElement>(null);

  // Register GPU scan-ring component
  useEffect(() => { registerGameComponent('game-scan-ring'); }, []);

  // Close overflow on outside click
  useEffect(() => {
    if (!overflowOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (overflowRef.current && !overflowRef.current.contains(e.target as HTMLElement)) {
        setOverflowOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [overflowOpen]);

  const embeddingMode = useAppStore(s => s.embeddingMode);
  const isRefreshing = state.loading || aiBriefing.loading;
  const refreshLabel = getRefreshLabel(state, aiBriefing.loading, t);
  const isUpToDate = !state.loading && !aiBriefing.loading && state.lastAnalyzedAt != null && Date.now() - state.lastAnalyzedAt.getTime() < 600_000;

  return (
    <div className="mb-6 bg-bg-secondary rounded-lg border border-border overflow-hidden" role="region" aria-label="Analysis controls">
      {/* Main Action Row */}
      <div className="px-5 py-4 flex items-center gap-4">
        {/* Status */}
        <div className="flex items-center gap-3 flex-1 min-w-0" role="status" aria-live="polite">
          {state.loading ? (
            <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center flex-shrink-0 overflow-hidden" aria-busy="true">
              <game-scan-ring style={{ width: '20px', height: '20px' }} aria-label="Analysis in progress" />
            </div>
          ) : (
            <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center flex-shrink-0">
              <span className="text-gray-500">*</span>
            </div>
          )}
          <div className="min-w-0">
            <p className="text-sm text-white font-medium truncate">
              {state.loading ? t('action.analyzing') : state.analysisComplete ? t('action.analysisComplete') : t('action.ready')}
            </p>
            <p className="text-xs text-gray-500 truncate">
              {state.status}
              {state.lastAnalyzedAt && !state.loading && (
                <span className="ml-2 text-gray-600">
                  · {state.lastAnalyzedAt.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                </span>
              )}
            </p>
          </div>
        </div>

        {/* LLM Badge */}
        {settings?.rerank.enabled && settings?.llm.has_api_key && (
          <div className="px-3 py-1.5 bg-orange-500/10 text-orange-400 text-xs rounded-lg border border-orange-500/20">
            LLM
          </div>
        )}

        {/* Keyword-only Mode Badge */}
        {embeddingMode === 'keyword-only' && (
          <button
            className="px-3 py-1.5 bg-amber-500/10 text-amber-400 text-xs rounded-lg border border-amber-500/20 cursor-pointer hover:bg-amber-500/20 transition-colors"
            title="No embedding model available. Click to configure AI provider for semantic matching."
            onClick={() => useAppStore.getState().setShowSettings(true)}
          >
            {t('action.keywordOnly')}
          </button>
        )}

        {/* Summary Badges */}
        {summaryBadges && (
          <div className="flex items-center gap-1.5">
            <span className="px-2 py-1 text-[11px] bg-bg-tertiary text-gray-400 rounded-lg font-mono">
              {summaryBadges.total}
            </span>
            <span className="px-2 py-1 text-[11px] bg-green-500/10 text-green-400 rounded-lg font-mono">
              {summaryBadges.relevantCount} rel
            </span>
            {summaryBadges.topCount > 0 && (
              <span className="px-2 py-1 text-[11px] bg-orange-500/10 text-orange-400 rounded-lg font-mono">
                {summaryBadges.topCount} top
              </span>
            )}
          </div>
        )}

        {/* Actions */}
        <div className="flex items-center gap-2">
          {/* Smart Refresh Button */}
          <button
            onClick={onAnalyze}
            disabled={isRefreshing}
            className={`px-5 py-2.5 text-sm font-medium rounded-lg transition-all flex items-center gap-2 ${
              isUpToDate
                ? 'bg-green-500/10 text-green-400 border border-green-500/20'
                : 'bg-orange-500 text-white hover:bg-orange-600 hover:scale-105 active:scale-95'
            } disabled:opacity-50 disabled:cursor-not-allowed`}
          >
            {isRefreshing && (
              <div className="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin" />
            )}
            {isUpToDate && !isRefreshing && (
              <div className="w-2 h-2 bg-green-400 rounded-full" />
            )}
            {refreshLabel}
          </button>
          {state.loading && (
            <button
              onClick={() => invoke('cancel_analysis')}
              className="px-3 py-2.5 text-sm bg-bg-tertiary text-red-400 border border-red-500/30 font-medium rounded-lg hover:bg-red-500/10 transition-all"
            >
              {t('action.cancel')}
            </button>
          )}

          {/* Overflow Menu */}
          <div className="relative" ref={overflowRef}>
            <button
              onClick={() => setOverflowOpen(!overflowOpen)}
              aria-label="More actions"
              aria-expanded={overflowOpen}
              aria-haspopup="true"
              className="w-10 h-10 rounded-lg flex items-center justify-center bg-bg-tertiary text-gray-400 border border-border hover:text-gray-200 hover:border-[#3A3A3A] transition-all"
              title="More actions"
            >
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <circle cx="8" cy="3" r="1.5" fill="currentColor" />
                <circle cx="8" cy="8" r="1.5" fill="currentColor" />
                <circle cx="8" cy="13" r="1.5" fill="currentColor" />
              </svg>
            </button>
            {overflowOpen && (
              <div className="absolute right-0 top-12 z-50 w-56 bg-[#1A1A1A] border border-border rounded-lg shadow-xl py-1" role="menu">
                <button
                  role="menuitem"
                  onClick={() => { onGenerateBriefing(); setOverflowOpen(false); }}
                  disabled={aiBriefing.loading || state.relevanceResults.length === 0}
                  className="w-full px-4 py-2.5 text-sm text-left text-gray-300 hover:bg-border disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
                >
                  {t('action.regenerateBriefing')}
                </button>
                <button
                  role="menuitem"
                  onClick={() => { onToggleAutoBriefing(); setOverflowOpen(false); }}
                  className="w-full px-4 py-2.5 text-sm text-left text-gray-300 hover:bg-border transition-colors flex items-center justify-between"
                >
                  {t('action.autoBriefing')}
                  <span className={`text-xs px-2 py-0.5 rounded ${autoBriefingEnabled ? 'bg-orange-500/20 text-orange-400' : 'bg-border text-gray-500'}`}>
                    {autoBriefingEnabled ? 'ON' : 'OFF'}
                  </span>
                </button>
                <div className="border-t border-border my-1" />
                <div className="px-4 py-2 flex items-center gap-2">
                  <AudioBriefing />
                  <ContextHandoff onStatus={(msg) => { onToast(msg.includes('fail') ? 'error' : 'success', msg); setOverflowOpen(false); }} />
                </div>
                {state.analysisComplete && (
                  <>
                    <div className="border-t border-border my-1" />
                    <button
                      role="menuitem"
                      onClick={async () => {
                        try {
                          const md = await invoke<string>('export_results', { format: 'markdown' });
                          await window.navigator.clipboard.writeText(md);
                          onToast('success', 'Results copied to clipboard');
                        } catch (e) {
                          onToast('error', `Export failed: ${e}`);
                        }
                        setOverflowOpen(false);
                      }}
                      className="w-full px-4 py-2.5 text-sm text-left text-gray-300 hover:bg-border transition-colors"
                    >
                      {t('action.exportMarkdown')}
                    </button>
                    <button
                      role="menuitem"
                      onClick={async () => {
                        try {
                          const digest = await invoke<string>('export_results', { format: 'digest' });
                          await window.navigator.clipboard.writeText(digest);
                          onToast('success', 'Shareable digest copied');
                        } catch (e) {
                          onToast('error', `Digest export failed: ${e}`);
                        }
                        setOverflowOpen(false);
                      }}
                      className="w-full px-4 py-2.5 text-sm text-left text-gray-300 hover:bg-border transition-colors"
                    >
                      {t('action.exportDigest')}
                    </button>
                  </>
                )}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Progress Bar */}
      {state.loading && state.progress > 0 && (
        <div className="px-5 pb-4">
          <div className="flex justify-between text-xs text-gray-500 mb-2">
            <span>{getStageLabel(state.progressStage)}</span>
            <span>{Math.round(state.progress * 100)}%</span>
          </div>
          <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
            <div
              role="progressbar"
              aria-valuenow={Math.round(state.progress * 100)}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label={`Analysis progress: ${Math.round(state.progress * 100)}%`}
              className="h-full bg-gradient-to-r from-orange-600 to-orange-400 transition-all duration-300 ease-out rounded-full"
              style={{ width: `${state.progress * 100}%` }}
            />
          </div>
        </div>
      )}

      {/* AI Briefing Error */}
      {aiBriefing.error && (
        <div role="alert" className="mx-5 mb-4 p-3 bg-red-900/20 border border-red-500/30 rounded-lg text-red-300 text-sm flex items-center gap-2">
          <span>!</span>
          {aiBriefing.error}
        </div>
      )}
    </div>
  );
}
