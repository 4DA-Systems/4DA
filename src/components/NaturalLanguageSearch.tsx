import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { listen } from '@tauri-apps/api/event';
import { useAppStore } from '../store';
import { useLicense } from '../hooks';
import { trackEvent } from '../hooks/use-telemetry';
import { useGameComponent } from '../hooks/use-game-component';
import { reportError } from '../lib/error-reporter';
import { StackHealthBar, type StackHealth } from './search/StackHealthBar';
import { SynthesisPanel, type SynthesisResponse } from './search/SynthesisPanel';
import { GhostPreview, type GhostPreviewData } from './search/GhostPreview';
import { StandingQueries } from './search/StandingQueries';

interface QueryResultItem {
  id: number;
  file_path: string | null;
  file_name: string | null;
  preview: string;
  relevance: number;
  source_type: string;
  timestamp: string | null;
  match_reason: string;
}

interface QueryResult {
  query: string;
  intent: string;
  items: QueryResultItem[];
  total_count: number;
  execution_ms: number;
  summary: string | null;
  parsed: {
    keywords: string[];
    entities: string[];
    time_range: { start: string; end: string; relative: string | null } | null;
    file_types: string[];
    sentiment: string | null;
    confidence: number;
  };
  stack_context: { name: string; category: string; relevant: boolean }[];
  related_decisions: { id: number; subject: string; decision: string; relation: string }[];
  knowledge_gaps: { technology: string; days_stale: number; severity: string }[];
  ghost_preview: GhostPreviewData | null;
  is_pro: boolean;
}

const intentLabels: Record<string, string> = { Find: 'Find', Summarize: 'Summarize', Compare: 'Compare', Timeline: 'Timeline', Count: 'Count' };
const sourceLabels: Record<string, string> = { pdf: 'PDF', docx: 'DOC', xlsx: 'XLS', image: 'IMG', context: 'CTX' };

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

  const { containerRef: ambientRef, elementRef: ambientElement } = useGameComponent('game-briefing-atmosphere');

  useEffect(() => {
    const el = ambientElement.current;
    if (!el) return;
    el.setParam?.('quality', hasAnalysisRun ? 0.6 : 0.15);
    el.setParam?.('signal_heat', result ? Math.min(result.total_count / 50, 1) : 0);
    el.setParam?.('decision_pressure', loading ? 0.8 : 0);
  }, [hasAnalysisRun, result, loading, ambientElement]);

  // Load stack health on mount
  useEffect(() => {
    cmd('get_stack_health')
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
        fetchSynthesis(query);
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

  const handleKeyDown = (e: React.KeyboardEvent) => { if (e.key === 'Enter') handleSearch(); };
  const handleSuggestedQuery = (sq: string) => setQuery(sq);
  const clearResults = () => { setResult(null); setQuery(''); setSynthesis(null); };
  const [watchCreated, setWatchCreated] = useState(false);
  const handleWatch = async () => {
    if (!query.trim()) return;
    try {
      await cmd('create_standing_query', { queryText: query });
      setWatchCreated(true);
      setTimeout(() => setWatchCreated(false), 2000);
    } catch (err) { reportError('NaturalLanguageSearch.watchCreation', err); }
  };
  const relevantStack = result?.stack_context?.filter((s) => s.relevant) ?? [];

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
        className="flex items-center justify-between cursor-pointer w-full text-left"
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
        aria-label={expanded ? t('search.collapsePanel') : t('search.expandPanel')}
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-cyan-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
            <span className="text-cyan-400 text-sm font-bold">{'\u2756'}</span>
          </div>
          <div>
            <h2 className="text-white font-medium text-base">{t('search.title')}</h2>
            <p className="text-text-muted text-sm">{t('search.subtitle')}</p>
          </div>
        </div>
        <span className="text-text-muted text-sm" aria-hidden="true">{expanded ? '\u25BC' : '\u25B6'}</span>
      </button>

      {expanded && !hasAnalysisRun && (
        <div className="mt-4">
          <div className="bg-bg-secondary rounded-lg border border-border p-5">
            <p className="text-sm text-white font-medium mb-3">{t('search.noAnalysisTitle')}</p>
            <div className="grid grid-cols-2 gap-2 mb-3">
              <div className="flex items-center gap-2 text-xs text-text-secondary">
                <span className="text-cyan-400/50">{'\u2756'}</span>
                {t('search.capabilityStack')}
              </div>
              <div className="flex items-center gap-2 text-xs text-text-secondary">
                <span className="text-cyan-400/50">{'\u2696'}</span>
                {t('search.capabilityDecisions')}
              </div>
              <div className="flex items-center gap-2 text-xs text-text-secondary">
                <span className="text-cyan-400/50">{'\u25CE'}</span>
                {t('search.capabilityGaps')}
              </div>
              <div className="flex items-center gap-2 text-xs text-text-secondary">
                <span className="text-cyan-400/50">{'\u2726'}</span>
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
              onClick={handleSearch}
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
                onClick={handleWatch}
                disabled={watchCreated}
                title={t('search.watchThis')}
                aria-label={watchCreated ? t('search.watchCreated', 'Watch created') : t('search.watchThis')}
                className={`px-3 py-3 text-sm border rounded-lg transition-all ${watchCreated ? 'bg-green-500/10 border-green-500/30 text-green-400' : 'bg-bg-secondary border-border text-text-secondary hover:text-cyan-400 hover:border-cyan-500/30'}`}
              >
                <span aria-hidden="true">{watchCreated ? '\u2713' : '\u229A'}</span>
              </button>
            )}
          </div>

          {/* Error display */}
          {error && (
            <div role="alert" className="flex items-center gap-2 px-3 py-2 bg-red-900/20 border border-red-500/30 rounded-lg">
              <span className="text-red-400 text-xs" aria-hidden="true">{'\u26A0'}</span>
              <span className="text-xs text-red-300 flex-1">{error}</span>
              <button onClick={() => setError(null)} aria-label="Dismiss error" className="text-red-400/60 hover:text-red-400 text-xs">{'\u2715'}</button>
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
            <div className="space-y-4">
              {/* Synthesis panel (Pro only) */}
              <SynthesisPanel query={query} isPro={isPro} synthesis={synthesis} loading={synthesisLoading} streamingText={streamingText} onRetry={() => fetchSynthesis(query)} />

              {/* Query parsing info */}
              <div className="flex items-center gap-2 p-3 bg-bg-secondary rounded-lg border border-border flex-wrap">
                <span className="text-xs font-medium text-text-muted">{intentLabels[result.intent] || result.intent}</span>
                <span className="text-sm text-white">{'\u2022'}</span>
                <span className="text-sm text-cyan-400">{result.parsed.keywords.join(', ')}</span>
                {result.parsed.time_range && (
                  <span className="px-2 py-1 text-xs bg-bg-tertiary rounded-md text-text-secondary border border-border">
                    {result.parsed.time_range.relative || t('search.customRange')}
                  </span>
                )}
                {result.parsed.file_types.length > 0 && (
                  <span className="px-2 py-1 text-xs bg-bg-tertiary rounded-md text-text-secondary border border-border">
                    {result.parsed.file_types.join(', ')}
                  </span>
                )}
                <button onClick={clearResults} aria-label="Clear search results" className="ml-auto text-text-muted hover:text-white transition-colors">{'\u2715'}</button>
              </div>

              {/* Stack context */}
              {relevantStack.length > 0 && (
                <div className="flex items-center gap-2 text-xs text-text-secondary">
                  <span className="text-text-muted">{t('search.yourStack')}:</span>
                  {relevantStack.map((s) => (
                    <span key={s.name} className="px-2 py-0.5 bg-bg-secondary rounded border border-border text-text-secondary">{s.name}</span>
                  ))}
                </div>
              )}

              {/* Summary */}
              {result.summary && (
                <div className="text-sm text-text-secondary bg-bg-secondary rounded-lg p-4 border border-border">{result.summary}</div>
              )}

              {/* Result items */}
              <div className="space-y-2 max-h-64 overflow-y-auto">
                {result.items.map((item, index) => (
                  <div key={`${item.id}-${index}`} className="p-3 bg-bg-secondary rounded-lg border border-border hover:border-cyan-500/30 transition-colors">
                    <div className="flex items-start gap-3">
                      <span className="text-[10px] text-text-muted uppercase font-mono bg-bg-tertiary px-1.5 py-0.5 rounded">{sourceLabels[item.source_type] || 'SRC'}</span>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="text-sm text-white font-medium truncate">{item.file_name || t('search.unknownFile')}</span>
                          <span className={`text-xs px-2 py-0.5 rounded-md ${item.relevance > 0.7 ? 'bg-green-500/20 text-green-400' : item.relevance > 0.4 ? 'bg-yellow-500/20 text-yellow-400' : 'bg-gray-500/20 text-text-secondary'}`}>
                            {(item.relevance * 100).toFixed(0)}%
                          </span>
                        </div>
                        <p className="text-xs text-text-muted mt-1 line-clamp-2">{item.preview}</p>
                        <div className="flex items-center gap-2 mt-2 text-[10px] text-text-muted">
                          <span className="text-cyan-400/70">{item.match_reason}</span>
                          {item.timestamp && (
                            <>
                              <span>{'\u2022'}</span>
                              <span>{new Date(item.timestamp).toLocaleDateString()}</span>
                            </>
                          )}
                        </div>
                      </div>
                    </div>
                  </div>
                ))}
                {result.items.length === 0 && (
                  <div className="text-center py-6 bg-bg-secondary rounded-lg border border-border">
                    <div className="text-sm text-text-secondary">{t('search.noResults')}</div>
                    <div className="text-xs text-text-muted mt-1">{t('search.tryDifferent')}</div>
                  </div>
                )}
              </div>

              {/* Related decisions (Pro) */}
              {isPro && result.related_decisions.length > 0 && (
                <div className="space-y-1.5">
                  <h4 className="text-xs text-text-secondary uppercase tracking-wider">{t('search.relatedDecisions')}</h4>
                  {result.related_decisions.map((d) => (
                    <div key={d.id} className="px-3 py-2 bg-bg-secondary rounded-lg border border-border text-xs">
                      <span className="text-text-secondary">{d.subject}</span>
                      <span className="text-text-muted mx-1.5">{'\u2014'}</span>
                      <span className="text-text-secondary">{d.decision}</span>
                    </div>
                  ))}
                </div>
              )}

              {/* Knowledge gaps (Pro) */}
              {isPro && result.knowledge_gaps.length > 0 && (
                <div className="space-y-1.5">
                  <h4 className="text-xs text-text-secondary uppercase tracking-wider">{t('search.knowledgeGaps')}</h4>
                  {result.knowledge_gaps.map((gap, i) => (
                    <div key={i} className="flex items-center gap-2 px-3 py-2 bg-bg-secondary rounded-lg border border-border text-xs">
                      <span className={gap.severity === 'critical' ? 'text-red-400' : gap.severity === 'high' ? 'text-yellow-400' : 'text-text-secondary'}>{'\u25CF'}</span>
                      <span className="text-text-secondary">{gap.technology}</span>
                      <span className="text-text-muted">{t('search.staleForDays', { days: gap.days_stale })}</span>
                    </div>
                  ))}
                </div>
              )}

              {/* Ghost preview (non-Pro insight) */}
              {result.ghost_preview && !result.is_pro && (
                <GhostPreview preview={result.ghost_preview} />
              )}

              {/* Stats footer */}
              <div className="text-xs text-text-muted text-center pt-2 border-t border-border">
                {t('search.stats', { count: result.total_count, ms: result.execution_ms, confidence: (result.parsed.confidence * 100).toFixed(0) })}
              </div>
            </div>
          )}

          {/* Standing queries (Pro) */}
          {isPro && <StandingQueries isPro={isPro} />}
        </div>
      )}
      </div>
    </div>
  );
}
