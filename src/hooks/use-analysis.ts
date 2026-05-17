// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { listen } from '@tauri-apps/api/event';
import i18n from 'i18next';
import type { SourceRelevance, AnalysisProgress } from '../types';
import { getSourceLabel } from '../config/sources';
import { cmd } from '../lib/commands';
import { useAppStore } from '../store';
import { extractNearMisses, scrollToAndHighlightItem } from './analysis-utils';
import type { NarrationEvent } from './analysis-utils';
export type { NarrationEvent } from './analysis-utils';

/**
 * Analysis hook — thin wrapper around Zustand store.
 * Sets up Tauri event listeners that write directly to the store.
 * All state lives in the store; this hook adds no local state.
 */
export function useAnalysis(
  _addToast?: (type: 'success' | 'error' | 'warning' | 'info', message: string) => void,
  onBackgroundItems?: (itemIds: number[]) => void,
) {
  const state = useAppStore(s => s.appState);
  const expandedItem = useAppStore(s => s.expandedItem);
  const isBrowserMode = useAppStore(s => s.isBrowserMode);
  const setExpandedItem = useAppStore(s => s.setExpandedItem);
  const startAnalysis = useAppStore(s => s.startAnalysis);
  const loadContextFiles = useAppStore(s => s.loadContextFiles);
  const clearContext = useAppStore(s => s.clearContext);
  const indexContext = useAppStore(s => s.indexContext);

  // Live analysis narration events
  const [narrationEvents, setNarrationEvents] = useState<NarrationEvent[]>([]);

  const clearNarrationEvents = useCallback(() => {
    setNarrationEvents([]);
  }, []);

  // Set up event listeners for background analysis — all write to store
  useEffect(() => {
    let unlistens: UnlistenFn[] = [];

    const setupListeners = async () => {
      const results = await Promise.all([
        listen<AnalysisProgress>('analysis-progress', (event) => {
          const { stage, progress, message, items_processed, items_total } = event.payload;
          useAppStore.getState().setAppStateFull((s) => ({
            ...s,
            progress,
            progressMessage: message,
            progressStage: stage,
            status: items_total > 0
              ? `${message} (${items_processed}/${items_total})`
              : message,
          }));
        }),

        listen<SourceRelevance[]>('analysis-complete', (event) => {
          const results = event.payload;
          const relevantCount = results.filter((r) => r.relevant).length;
          const nearMisses = extractNearMisses(results, relevantCount);

          useAppStore.getState().setAppStateFull((s) => ({
            ...s,
            relevanceResults: results,
            nearMisses,
            status: i18n.t('analysis.statusRelevant', { relevant: relevantCount, total: results.length }),
            loading: false,
            analysisComplete: true,
            progress: 1,
            progressStage: 'complete',
            lastAnalyzedAt: new Date(),
          }));
          useAppStore.getState().addToast('success', i18n.t('analysis.complete', { count: relevantCount }));

          // Auto-enable monitoring after first successful analysis
          const { monitoring } = useAppStore.getState();
          if (monitoring && !monitoring.enabled && relevantCount > 0) {
            void cmd('set_monitoring_enabled', { enabled: true }).then(() => {
              void useAppStore.getState().loadMonitoringStatus();
            }).catch((e) => console.debug('[analysis] auto-enable monitoring:', e));
          }
        }),

        listen<string>('analysis-error', (event) => {
          const msg = event.payload;
          useAppStore.getState().setAppStateFull((s) => ({
            ...s,
            status: i18n.t('analysis.statusError', { message: msg }),
            loading: false,
            progress: 0,
            progressStage: 'error',
          }));
          const { addToast: toast } = useAppStore.getState();
          if (msg.includes('API') || msg.includes('key') || msg.includes('401')) {
            toast('error', i18n.t('analysis.apiError'));
          } else if (msg.includes('network') || msg.includes('timeout') || msg.includes('connect')) {
            toast('error', i18n.t('analysis.networkError'));
          } else {
            toast('error', i18n.t('analysis.failed', { message: msg }));
          }
        }),

        listen<{ source: string; error: string; retry_count: number }>('source-error', (event) => {
          const { source, error } = event.payload;
          useAppStore.getState().addToast('warning', i18n.t('analysis.sourceError', { source: getSourceLabel(source), error }));
        }),
        listen('network-offline', () => {
          useAppStore.getState().addToast('warning', i18n.t('analysis.offline'));
        }),

        listen<{ mode: string; reason?: string }>('embedding-mode', (event) => {
          const mode = event.payload.mode as 'semantic' | 'keyword-only';
          useAppStore.getState().setEmbeddingMode(mode);
          if (mode === 'keyword-only') {
            useAppStore.getState().addToast('warning', i18n.t('analysis.keywordOnly'));
          }
        }),

        listen('start-analysis-from-tray', () => {
          void useAppStore.getState().startAnalysis();
        }),

        // Custom notification clicked — navigate to briefing/signals view
        listen<{ item_id?: number }>('navigate-to-signals', (event) => {
          useAppStore.getState().setActiveView('briefing');
          if (event.payload?.item_id) {
            scrollToAndHighlightItem(event.payload.item_id);
          }
        }),

        listen<SourceRelevance[]>('background-results', (event) => {
          const newItems = event.payload;
          if (newItems.length === 0) return;
          const relevantNew = newItems.filter((r) => r.relevant).length;
          useAppStore.getState().setAppStateFull((s) => {
            const existingIds = new Set(newItems.map((n) => n.id));
            const kept = s.relevanceResults.filter((r) => !existingIds.has(r.id));
            const merged = [...kept, ...newItems].sort((a, b) => b.top_score - a.top_score);
            return {
              ...s,
              relevanceResults: merged,
              analysisComplete: true,
              lastAnalyzedAt: new Date(),
            };
          });
          if (relevantNew > 0) {
            useAppStore.getState().addToast('info', i18n.t('analysis.newRelevant', { count: relevantNew }));
            useAppStore.getState().setLastBackgroundResultsAt(new Date());
            onBackgroundItems?.(newItems.map(n => n.id));
          }
        }),

        listen<{ briefing: string; model?: string }>('briefing-auto-generated', (event) => {
          const data = event.payload;
          if (data.briefing) {
            useAppStore.getState().addToast('info', i18n.t('analysis.briefingAutoGenerated'));
          }
        }),

        listen<{ title: string; total_relevant: number; items: Array<{ title: string; source_type: string; score: number; signal_type?: string }>; data_freshness?: { newest_item_age_hours: number | null; items_last_24h: number; items_last_72h: number; newest_source_check_age_hours: number | null; source_checks_last_24h: number; source_checks_last_72h: number; failing_sources: number; stale_sources: number; is_stale: boolean } | null }>('morning-briefing-ready', (event) => {
          const { title, total_relevant, items, data_freshness } = event.payload;
          const isStale = data_freshness?.is_stale ?? false;
          if (total_relevant > 0 || isStale) {
            // Store morning briefing items for the main view's render waterfall.
            // The instant snapshot is NOT cleared here — it's naturally superseded
            // when aiBriefing.content or analysisComplete becomes true. Clearing it
            // prematurely caused the "black hole" startup bug (snapshot gone, no
            // replacement content yet → empty warmup state).
            useAppStore.getState().setMorningBriefData({
              title,
              totalRelevant: total_relevant,
              items: items.map(i => ({
                title: i.title,
                sourceType: i.source_type,
                score: i.score,
                signalType: i.signal_type ?? null,
              })),
              dataFreshness: data_freshness ? {
                newest_item_age_hours: data_freshness.newest_item_age_hours,
                items_last_24h: data_freshness.items_last_24h,
                items_last_72h: data_freshness.items_last_72h,
                newest_source_check_age_hours: data_freshness.newest_source_check_age_hours,
                source_checks_last_24h: data_freshness.source_checks_last_24h,
                source_checks_last_72h: data_freshness.source_checks_last_72h,
                failing_sources: data_freshness.failing_sources,
                stale_sources: data_freshness.stale_sources,
                is_stale: data_freshness.is_stale,
              } : null,
            });
            if (isStale) {
              useAppStore.getState().addToast('warning', i18n.t('analysis.staleData', 'Source data is stale — sources may need attention'));
            } else {
              useAppStore.getState().addToast('info', i18n.t('analysis.morningBriefingReady', { count: total_relevant }));
            }
          }
        }),

        listen<{ synthesis: string }>('morning-briefing-synthesis', (event) => {
          if (event.payload.synthesis) {
            useAppStore.getState().setMorningBriefSynthesis(event.payload.synthesis);
          }
        }),

        listen<{ type: string; severity: string; description: string }>('anomaly-detected', (event) => {
          const { severity, description } = event.payload;
          if (severity === 'High' || severity === 'Critical') {
            useAppStore.getState().addToast('warning', i18n.t('analysis.anomaly', { description }));
          }
        }),

        listen<{ item_count: number }>('digest-generated', (event) => {
          useAppStore.getState().addToast('info', i18n.t('analysis.digestGenerated', { count: event.payload.item_count }));
        }),

        listen<SourceRelevance[]>('partial-results', (event) => {
          const state = useAppStore.getState();
          if (state.appState.analysisComplete) return;
          const existingIds = new Set(state.appState.relevanceResults.map(r => r.id));
          const newItems = event.payload.filter(r => !existingIds.has(r.id));
          if (newItems.length === 0) return;
          const merged = [...state.appState.relevanceResults, ...newItems]
            .sort((a, b) => b.top_score - a.top_score);
          state.setAppStateFull(s => ({ ...s, relevanceResults: merged }));
        }),

        listen<string[]>('stacks-auto-detected', (event) => {
          const profileIds = event.payload;
          useAppStore.getState().addToast('info',
            i18n.t('analysis.stackDetected', { stack: profileIds.join(', ') }),
          );
        }),

        listen<{ task: string; reason: string }>('calibration-drift', (event) => {
          useAppStore.getState().addToast('info',
            i18n.t('analysis.calibrationDrift', { task: event.payload.task }),
          );
        }),

        listen<{
          narration_type: string;
          message: string;
          source: string | null;
          relevance: number | null;
        }>('analysis-narration', (event) => {
          setNarrationEvents(prev => [...prev.slice(-20), {
            type: event.payload.narration_type,
            message: event.payload.message,
            source: event.payload.source ?? undefined,
            relevance: event.payload.relevance ?? undefined,
            timestamp: Date.now(),
          }]);
        }),
      ]);

      unlistens = results;
    };

    void setupListeners();

    return () => {
      for (const unlisten of unlistens) unlisten();
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps -- stable callback ref
  }, []);

  useEffect(() => {
    void loadContextFiles();
  }, [loadContextFiles]);

  // Hydrate from backend cache — catches results from analysis that ran before mount
  useEffect(() => {
    const store = useAppStore.getState();
    if (store.appState.relevanceResults.length > 0) return; // Already have results
    void cmd('get_analysis_status').then((status) => {
      const s = status as { results?: SourceRelevance[]; completed?: boolean };
      if (s.results && s.results.length > 0) {
        useAppStore.getState().setAppStateFull((prev) => ({
          ...prev,
          relevanceResults: s.results!,
          analysisComplete: true,
          loading: false,
        }));
      }
    }).catch(() => { /* Silent — analysis may not have run yet */ });
  }, []);

  const setStatus = useCallback((status: string) => {
    useAppStore.getState().setAppState({ status });
  }, []);

  // setState wrapper for App.tsx compatibility (accepts updater function)
  const setState = useCallback((updater: ((s: typeof state) => typeof state) | Partial<typeof state>) => {
    if (typeof updater === 'function') {
      useAppStore.getState().setAppStateFull(updater);
    } else {
      useAppStore.getState().setAppState(updater);
    }
  }, []);

  return {
    state,
    setState,
    expandedItem,
    setExpandedItem,
    isBrowserMode,
    loadContextFiles,
    clearContext,
    indexContext,
    startAnalysis,
    setStatus,
    narrationEvents,
    clearNarrationEvents,
  };
}
