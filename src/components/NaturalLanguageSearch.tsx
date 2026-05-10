// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { listen } from '@tauri-apps/api/event';
import { useAppStore } from '../store';
import { useLicense } from '../hooks';
import { trackEvent } from '../hooks/use-telemetry';
import { useFourdaComponent } from '../hooks/use-fourda-component';
import { reportError } from '../lib/error-reporter';
import { StackHealthBar, type StackHealth } from './search/StackHealthBar';
import type { SynthesisResponse } from './search/SynthesisPanel';
import { StandingQueries } from './search/StandingQueries';
import { SearchResults } from './search/SearchResults';
import type { QueryResult } from './search/search-types';

interface NaturalLanguageSearchProps {
  onStatusChange?: (status: string) => void;
  defaultExpanded?: boolean;
}

export function NaturalLanguageSearch({ onStatusChange, defaultExpanded = true }: NaturalLanguageSearchProps) {
  const { t } = useTranslation();
  const { isPro } = useLicense();
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<QueryResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState(defaultExpanded);
  const [error, setError] = useState<string | null>(null);
  const [stackHealth, setStackHealth] = useState<StackHealth | null>(null);
  const [synthesis, setSynthesis] = useState<SynthesisResponse | null>(null);
  const [synthesisLoading, setSynthesisLoading] = useState(false);
  const [streamingText, setStreamingText] = useState('');
  const analysisComplete = useAppStore((s) => s.appState.analysisComplete);
  const lastAnalyzedAt = useAppStore((s) => s.appState.lastAnalyzedAt);
  const hasAnalysisRun = analysisComplete || lastAnalyzedAt !== null;
  const synthesisUnlistenRef = useRef<(() => void) | null>(null);

  const { containerRef: ambientRef, elementRef: ambientElement } = useFourdaComponent('fourda-briefing-atmosphere');

  useEffect(() => {
    const el = ambientElement.current;
    if (!el) return;
    el.setParam?.('quality', hasAnalysisRun ? 0.6 : 0.15);
    el.setParam?.('signal_heat', result ? Math.min(result.total_count / 50, 1) : 0);
    el.setParam?.('decision_pressure', loading ? 0.8 : 0);
  }, [hasAnalysisRun, result, loading, ambientElement]);

  // Load stack health on mount
  useEffect(() => {
    void cmd('get_stack_health')
      .then(r => r as unknown as StackHealth)
      .then(setStackHealth)
      .catch((err: unknown) => reportError('NaturalLanguageSearch.stackHealth', err));
  }, []);

  const fetchSynthesis = useCallback(async (queryText: string) => {
    setSynthesisLoading(true);
    setStreamingText('');

    if (synthesisUnlistenRef.current) {
      synthesisUnlistenRef.current();
      synthesisUnlistenRef.current = null;
    }

    const unlisten = await listen<string>('synthesis-token', (event) => {
      setStreamingText(prev => prev + event.payload);
    });
    synthesisUnlistenRef.current = unlisten;

    try {
      const resp = await cmd('synthesize_search', { queryText }) as unknown as SynthesisResponse;
      setSynthesis(resp);
      setStreamingText('');
    } catch (err) {
      reportError('NaturalLanguageSearch.synthesis', err);
      setSynthesis(null);
    } finally {
      unlisten();
      synthesisUnlistenRef.current = null;
      setSynthesisLoading(false);
    }
  }, []);

  useEffect(() => {
    return () => {
      if (synthesisUnlistenRef.current) {
        synthesisUnlistenRef.current();
      }
    };
  }, []);

  const handleSearch = async () => {
    if (!query.trim()) return;
    trackEvent('search_query', undefined, { query_length: query.trim().length });
    setLoading(true);
    setError(null);
    setSynthesis(null);
    try {
      const searchResult = await cmd('natural_language_query', { queryText: query }) as unknown as QueryResult;
      setResult(searchResult);
      onStatusChange?.(`Found ${searchResult.total_count} results in ${searchResult.execution_ms}ms`);
      if (searchResult.is_pro) {
        void fetchSynthesis(query);
      }
    } catch (err) {
      const msg = String(err);
      reportError('NaturalLanguageSearch.search', err);
      setError(msg.includes('No context') ? t('search.indexFirst') : msg);
      onStatusChange?.(`Search error: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => { if (e.key === 'Enter') void handleSearch(); };
  const handleSuggestedQuery = (sq: string) => setQuery(sq);
  const clearResults = () => { setResult(null); setQuery(''); setSynthesis(null); };
  const [watchCreated, setWatchCreated] = useState(false);
  const addToast = useAppStore(s => s.addToast);
  const handleWatch = async () => {
    if (!query.trim()) return;
    try {
      await cmd('create_standing_query', { queryText: query });
      setWatchCreated(true);
      setTimeout(() => setWatchCreated(false), 2000);
    } catch (err) {
      reportError('NaturalLanguageSearch.watchCreation', err);
      addToast('error', t('search.watchFailed', 'Failed to create watch. Please try again.'));
    }
  };

  return (
    <div className="relative rounded-lg p-5 border border-border overflow-hidden bg-bg-tertiary/50">
      {/* Ambient GAME field behind search panel */}
      <div
        ref={ambientRef}
        className="absolute inset-0 opacity-30 pointer-events-none"
        aria-hidden="true"
      />
      <div className="relative z-10">
      <button
        className="flex items-center justify-between cursor-pointer w-full text-start"
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
        aria-label={expanded ? t('search.collapsePanel') : t('search.expandPanel')}
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-cyan-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
            <span className="text-cyan-400 text-sm font-bold">{'❖'}</span>
          </div>
          <div>
            <h2 className="text-white font-medium text-base">{t('search.title')}</h2>
            <p className="text-text-muted text-sm">{t('search.subtitle')}</p>
          </div>
        </div>
        <span className="text-text-muted text-sm" aria-hidden="true">{expanded ? '▼' : '▶'}</span>
      </button>

      {expanded && !hasAnalysisRun && (
        <div className="mt-4">
          <div className="bg-bg-secondary rounded-lg border border-border p-5">
            <p className="text-sm text-white font-medium mb-3">{t('search.noAnalysisTitle')}</p>
            <div className="grid grid-cols-2 gap-2 mb-3">
              <div className="flex items-center gap-2 text-xs text-text-secondary">
                <span className="text-cyan-400/50">{'❖'}</span>
                {t('search.capabilityStack')}
              </div>
              <div className="flex items-center gap-2 text-xs text-text-secondary">
                <span className="text-cyan-400/50">{'⚖'}</span>
                {t('search.capabilityDecisions')}
              </div>
              <div className="flex items-center gap-2 text-xs text-text-secondary">
                <span className="text-cyan-400/50">{'◎'}</span>
                {t('search.capabilityGaps')}
              </div>
              <div className="flex items-center gap-2 text-xs text-text-secondary">
                <span className="text-cyan-400/50">{'✦'}</span>
                {t('search.capabilitySynthesis')}
              </div>
            </div>
            <p className="text-xs text-text-muted">{t('search.noAnalysisHint')}</p>
          </div>
        </div>
      )}

      {expanded && hasAnalysisRun && (
        <div className="mt-4 space-y-4">
          {/* Stack health bar */}
          <StackHealthBar health={stackHealth} onSuggestedQuery={handleSuggestedQuery} />

          {/* Search input */}
          <div className="flex gap-2" role="search">
            <input
              type="text"
              aria-label="Natural language search query"
              placeholder={t('search.placeholder')}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={handleKeyDown}
              className="flex-1 px-4 py-3 text-sm bg-bg-secondary border border-border rounded-lg text-white placeholder:text-text-muted focus:outline-none focus:border-cyan-500/50 transition-colors"
            />
            <button
              onClick={() => void handleSearch()}
              disabled={loading || !query.trim()}
              aria-label={loading ? t('search.searching') : t('action.search')}
              className="px-5 py-3 text-sm bg-cyan-500/20 border border-cyan-500/30 text-cyan-400 rounded-lg hover:bg-cyan-500/30 disabled:opacity-50 disabled:cursor-not-allowed transition-all font-medium"
            >
              {loading ? (
                <span className="flex items-center gap-1.5">
                  <span className="w-3 h-3 border-2 border-cyan-400/30 border-t-cyan-400 rounded-full animate-spin" />
                  {t('search.searching')}
                </span>
              ) : t('action.search')}
            </button>
            {isPro && result && (
              <button
                onClick={() => void handleWatch()}
                disabled={watchCreated}
                title={t('search.watchThis')}
                aria-label={watchCreated ? t('search.watchCreated', 'Watch created') : t('search.watchThis')}
                className={`px-3 py-3 text-sm border rounded-lg transition-all ${watchCreated ? 'bg-green-500/10 border-green-500/30 text-green-400' : 'bg-bg-secondary border-border text-text-secondary hover:text-cyan-400 hover:border-cyan-500/30'}`}
              >
                <span aria-hidden="true">{watchCreated ? '✓' : '⊚'}</span>
              </button>
            )}
          </div>

          {/* Error display */}
          {error && (
            <div role="alert" className="flex items-center gap-2 px-3 py-2 bg-red-900/20 border border-red-500/30 rounded-lg">
              <span className="text-red-400 text-xs" aria-hidden="true">{'⚠'}</span>
              <span className="text-xs text-red-300 flex-1">{error}</span>
              <button onClick={() => setError(null)} aria-label="Dismiss error" className="text-red-400/60 hover:text-red-400 text-xs">{'✕'}</button>
            </div>
          )}

          {/* Example queries — stack-aware when available */}
          {!result && !error && (
            <div className="space-y-2">
              <span className="text-xs text-text-secondary font-medium">
                {stackHealth?.suggested_queries?.length ? t('search.suggestedQueries') : t('search.tryThese')}
              </span>
              <div className="flex flex-wrap gap-2">
                {(stackHealth?.suggested_queries?.length
                  ? stackHealth.suggested_queries.slice(0, 4)
                  : ['show me files about authentication', 'pdfs from last month', 'summarize my notes on rust', 'what did I work on last week']
                ).map((example) => (
                  <button key={example} onClick={() => setQuery(example)} className="px-3 py-1.5 text-xs bg-bg-secondary rounded-lg border border-border text-text-secondary hover:text-cyan-400 hover:border-cyan-500/30 transition-all">
                    {example}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Results */}
          {result && (
            <SearchResults
              query={query}
              result={result}
              isPro={isPro}
              synthesis={synthesis}
              synthesisLoading={synthesisLoading}
              streamingText={streamingText}
              onRetrySynthesis={() => void fetchSynthesis(query)}
              onClear={clearResults}
            />
          )}

          {/* Standing queries (Pro) */}
          {isPro && <StandingQueries isPro={isPro} />}
        </div>
      )}
      </div>
    </div>
  );
}
