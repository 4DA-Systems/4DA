import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

// --- Types ---

interface FeedTestItem {
  title: string;
  url: string;
  published_at: string | null;
  content_preview: string;
}

interface FeedTestResult {
  feed_title: string | null;
  format: string;
  item_count: number;
  items: FeedTestItem[];
  fetch_duration_ms: number;
  errors: string[];
}

// --- Helpers ---

function isValidUrl(url: string): boolean {
  return url.startsWith('http://') || url.startsWith('https://');
}

function formatBadgeColor(format: string): string {
  const f = format.toLowerCase();
  if (f.includes('rss')) return 'bg-orange-400/15 text-orange-400';
  if (f.includes('atom')) return 'bg-blue-400/15 text-blue-400';
  return 'bg-[#1F1F1F] text-[#A0A0A0]';
}

function truncate(str: string, max: number): string {
  if (str.length <= max) return str;
  return str.slice(0, max) + '...';
}

// --- Component ---

export default function SourceDebugger() {
  const { t } = useTranslation();
  const [url, setUrl] = useState('');
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<FeedTestResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [validationError, setValidationError] = useState<string | null>(null);

  const testFeed = useCallback(async () => {
    const trimmed = url.trim();

    // Client-side validation
    if (!trimmed) {
      setValidationError('Enter a feed URL');
      return;
    }
    if (!isValidUrl(trimmed)) {
      setValidationError('URL must start with http:// or https://');
      return;
    }

    setValidationError(null);
    setLoading(true);
    setError(null);
    setResult(null);

    try {
      const res = await invoke<FeedTestResult>('toolkit_test_feed', { url: trimmed });
      setResult(res);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [url]);

  const clear = useCallback(() => {
    setUrl('');
    setResult(null);
    setError(null);
    setValidationError(null);
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !loading) {
      e.preventDefault();
      testFeed();
    }
  };

  return (
    <div className="space-y-4">
      {/* URL input row */}
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={url}
          onChange={(e) => {
            setUrl(e.target.value);
            if (validationError) setValidationError(null);
          }}
          onKeyDown={handleKeyDown}
          placeholder="https://example.com/feed.xml"
          spellCheck={false}
          className="flex-1 bg-[#1F1F1F] border border-[#2A2A2A] text-white text-sm font-mono rounded-lg px-3 py-2 outline-none focus:border-white/30 placeholder:text-[#666] transition-colors"
        />
        <button
          onClick={testFeed}
          disabled={loading || !url.trim()}
          className="flex items-center gap-2 px-5 py-2 text-sm font-medium bg-white text-[#0A0A0A] rounded-lg hover:bg-white/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
        >
          {loading ? (
            <>
              <div className="w-3.5 h-3.5 border-2 border-[#0A0A0A]/30 border-t-[#0A0A0A] rounded-full animate-spin" />
              {t('toolkit.sourceDebugger.testing')}
            </>
          ) : (
            <>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M4 11a9 9 0 019-9" />
                <path d="M4 4a16 16 0 0116 16" />
                <circle cx="5" cy="19" r="1" />
              </svg>
              {t('toolkit.sourceDebugger.testFeed')}
            </>
          )}
        </button>
        {(result || error) && (
          <button
            onClick={clear}
            className="px-3 py-2 text-xs text-[#A0A0A0] bg-[#141414] border border-[#2A2A2A] rounded-lg hover:text-white hover:border-white/20 transition-all shrink-0"
          >
            {t('toolkit.sourceDebugger.clear')}
          </button>
        )}
      </div>

      {/* Validation error */}
      {validationError && (
        <p className="text-xs text-[#EF4444]">{validationError}</p>
      )}

      {/* Fetch error */}
      {error && (
        <div className="flex items-center gap-3 px-4 py-3 bg-[#EF4444]/10 border border-[#EF4444]/30 rounded-lg">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#EF4444" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <span className="text-sm text-[#EF4444] flex-1">{error}</span>
        </div>
      )}

      {/* Empty state */}
      {!result && !error && !loading && (
        <div className="flex flex-col items-center justify-center py-16 text-[#666]">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="mb-3 opacity-50">
            <path d="M4 11a9 9 0 019-9" />
            <path d="M4 4a16 16 0 0116 16" />
            <circle cx="5" cy="19" r="1" />
          </svg>
          <p className="text-sm">{t('toolkit.sourceDebugger.empty')}</p>
          <p className="text-xs mt-1">{t('toolkit.sourceDebugger.emptyHint')}</p>
        </div>
      )}

      {/* Results */}
      {result && (
        <div className="space-y-4">
          {/* Result errors */}
          {result.errors.length > 0 && (
            <div className="space-y-1">
              {result.errors.map((err, i) => (
                <div key={i} className="px-3 py-2 bg-[#EF4444]/10 border border-[#EF4444]/30 rounded-lg text-xs text-[#EF4444] font-mono">
                  {err}
                </div>
              ))}
            </div>
          )}

          {/* Summary bar */}
          <div className="flex items-center gap-3 flex-wrap bg-[#141414] border border-[#2A2A2A] rounded-xl px-4 py-3">
            {/* Format badge */}
            <span className={`px-2 py-0.5 text-xs font-mono font-semibold rounded ${formatBadgeColor(result.format)}`}>
              {result.format}
            </span>

            {/* Feed title */}
            {result.feed_title && (
              <span className="text-sm text-white font-medium truncate">
                {result.feed_title}
              </span>
            )}

            <div className="flex items-center gap-4 ml-auto text-xs text-[#A0A0A0] font-mono shrink-0">
              <span>{result.item_count} item{result.item_count !== 1 ? 's' : ''}</span>
              <span>{result.fetch_duration_ms}ms</span>
            </div>
          </div>

          {/* Item list */}
          {result.items.length > 0 && (
            <div className="border border-[#2A2A2A] rounded-xl overflow-hidden max-h-[400px] overflow-y-auto">
              {result.items.map((item, i) => (
                <div
                  key={i}
                  className={`px-4 py-3 hover:bg-[#1F1F1F] transition-colors ${
                    i > 0 ? 'border-t border-[#2A2A2A]' : ''
                  }`}
                >
                  {/* Title as link */}
                  <div className="flex items-start gap-2 mb-1">
                    <span className="text-[10px] text-[#666] font-mono shrink-0 mt-0.5">
                      {i + 1}
                    </span>
                    {item.url ? (
                      <a
                        href={item.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-sm text-white hover:text-[#D4AF37] transition-colors line-clamp-1"
                        title={item.title}
                      >
                        {item.title || '(untitled)'}
                      </a>
                    ) : (
                      <span className="text-sm text-white line-clamp-1">
                        {item.title || '(untitled)'}
                      </span>
                    )}
                  </div>

                  {/* Published date */}
                  {item.published_at && (
                    <p className="text-[10px] text-[#666] font-mono ml-5 mb-1">
                      {item.published_at}
                    </p>
                  )}

                  {/* Content preview */}
                  {item.content_preview && (
                    <p className="text-xs text-[#A0A0A0] ml-5 line-clamp-2">
                      {truncate(item.content_preview, 200)}
                    </p>
                  )}
                </div>
              ))}
            </div>
          )}

          {result.items.length === 0 && result.errors.length === 0 && (
            <div className="text-center py-8 text-[#666] text-sm">
              {t('toolkit.sourceDebugger.noItems')}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
