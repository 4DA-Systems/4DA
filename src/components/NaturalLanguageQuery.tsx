import { useState, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { ProGate } from './ProGate';

interface NLQItem {
  id: number;
  file_path: string | null;
  file_name: string | null;
  preview: string;
  relevance: number;
  source_type: string;
  timestamp: string | null;
  match_reason: string;
}

interface NLQResult {
  query: string;
  intent: string;
  items: NLQItem[];
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
}

const SOURCE_COLORS: Record<string, string> = {
  pdf: 'bg-red-500/15 text-red-400 border-red-500/20',
  docx: 'bg-blue-500/15 text-blue-400 border-blue-500/20',
  xlsx: 'bg-green-500/15 text-green-400 border-green-500/20',
  image: 'bg-purple-500/15 text-purple-400 border-purple-500/20',
  context: 'bg-cyan-500/15 text-cyan-400 border-cyan-500/20',
};

const DEFAULT_SOURCE = 'bg-gray-500/15 text-gray-400 border-gray-500/20';
export const NaturalLanguageQueryPanel = memo(function NaturalLanguageQueryPanel() {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<NLQResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = useCallback(async () => {
    const trimmed = query.trim();
    if (!trimmed || loading) return;
    setLoading(true);
    setError(null);
    try {
      const r = await invoke<NLQResult>('natural_language_query', { queryText: trimmed });
      setResult(r);
    } catch (err) {
      const msg = String(err);
      setError(msg.includes('No context') ? t('search.indexFirst') : msg);
    } finally {
      setLoading(false);
    }
  }, [query, loading, t]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleSubmit();
  }, [handleSubmit]);

  return (
    <ProGate feature={t('search.nlqFeature')}>
      <div className="mb-6 bg-bg-secondary rounded-lg border border-border overflow-hidden">
        {/* Header */}
        <div className="px-5 py-4 border-b border-border flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" className="text-gray-400">
              <circle cx="6" cy="6" r="5" stroke="currentColor" strokeWidth="1.5" />
              <path d="M10 10L13 13" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            </svg>
          </div>
          <div>
            <h2 className="font-medium text-white text-sm">{t('search.nlqTitle')}</h2>
            <p className="text-xs text-gray-500">{t('search.nlqSubtitle')}</p>
          </div>
        </div>

        {/* Search input */}
        <div className="px-4 pt-4 pb-3">
          <div className="flex gap-2">
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={t('search.nlqPlaceholder')}
              className="flex-1 px-3 py-2 text-sm bg-bg-primary border border-border rounded-lg text-white placeholder:text-gray-600 focus:outline-none focus:border-gray-500 transition-colors"
            />
            <button
              onClick={handleSubmit}
              disabled={loading || !query.trim()}
              className="px-4 py-2 text-sm bg-bg-tertiary border border-border text-gray-400 rounded-lg hover:text-white hover:border-gray-500 disabled:opacity-40 disabled:cursor-not-allowed transition-all"
            >
              {loading ? (
                <span className="w-4 h-4 border-2 border-gray-600 border-t-gray-300 rounded-full animate-spin inline-block" />
              ) : (
                t('search.query')
              )}
            </button>
          </div>

          {/* Error */}
          {error && (
            <div className="mt-2 px-3 py-2 bg-red-500/10 border border-red-500/20 rounded-lg">
              <p className="text-xs text-red-400">{error}</p>
            </div>
          )}
        </div>

        {/* Empty state */}
        {!result && !error && !loading && (
          <div className="px-4 pb-4">
            <p className="text-xs text-gray-600 text-center py-3">
              {t('search.nlqHint')}
            </p>
          </div>
        )}

        {/* Results */}
        {result && (
          <div className="px-4 pb-4 space-y-3">
            {/* Parsed query info */}
            <div className="flex flex-wrap items-center gap-1.5">
              <span className="text-[10px] text-gray-500 uppercase tracking-wider">{result.intent}</span>
              {result.parsed.keywords.map((kw) => (
                <span key={kw} className="px-1.5 py-0.5 text-[10px] bg-bg-tertiary text-gray-400 rounded border border-border">
                  {kw}
                </span>
              ))}
              {result.parsed.time_range?.relative && (
                <span className="px-1.5 py-0.5 text-[10px] bg-bg-tertiary text-gray-500 rounded border border-border">
                  {result.parsed.time_range.relative}
                </span>
              )}
              <span className="text-[10px] text-gray-600 ml-auto font-mono">
                {(result.parsed.confidence * 100).toFixed(0)}% conf
              </span>
            </div>

            {/* Summary callout */}
            {result.summary && (
              <div className="px-3 py-2.5 bg-bg-primary rounded-lg border border-border">
                <p className="text-xs text-gray-300 leading-relaxed">{result.summary}</p>
              </div>
            )}

            {/* Result items */}
            {result.items.length > 0 ? (
              <div className="space-y-1.5 max-h-72 overflow-y-auto">
                {result.items.map((item, i) => (
                  <div key={`${item.id}-${i}`} className="px-3 py-2.5 bg-bg-primary rounded-lg border border-border hover:border-gray-600 transition-colors">
                    <div className="flex items-start gap-2">
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          {item.file_name ? (
                            <span className="text-xs text-white font-medium truncate">{item.file_name}</span>
                          ) : (
                            <span className="text-xs text-gray-400 truncate">{t('search.untitled')}</span>
                          )}
                          <span className={`text-[10px] px-1.5 py-0.5 rounded border ${SOURCE_COLORS[item.source_type] || DEFAULT_SOURCE}`}>
                            {item.source_type}
                          </span>
                        </div>
                        <p className="text-[11px] text-gray-500 mt-1 line-clamp-2">{item.preview}</p>
                        {item.timestamp && (
                          <span className="text-[10px] text-gray-600 mt-1 inline-block">
                            {new Date(item.timestamp).toLocaleDateString()}
                          </span>
                        )}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="text-center py-4">
                <p className="text-xs text-gray-500">{t('search.noResults')}</p>
                <p className="text-[10px] text-gray-600 mt-0.5">{t('search.tryDifferent')}</p>
              </div>
            )}

            {/* Stats bar */}
            <div className="pt-2 border-t border-border">
              <p className="text-[10px] text-gray-600 text-center font-mono">
                {t('search.resultStats', { count: result.total_count, ms: result.execution_ms })}
              </p>
            </div>
          </div>
        )}
      </div>
    </ProGate>
  );
});
