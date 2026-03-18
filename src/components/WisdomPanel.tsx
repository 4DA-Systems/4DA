import { useState, memo, useCallback } from 'react';
import { cmd } from '../lib/commands';

interface TransmuteResult {
  wisdom: string;
  confidence: number;
  watch_for: string[];
  mode: string;
}

/**
 * WisdomPanel — real decision analysis inline in Insights view.
 *
 * Calls AWE's transmutation pipeline via Tauri command.
 * Shows actual wisdom output — not a placeholder.
 * Three modes: Perspective (voice), Analysis (structured), Challenge.
 */
export const WisdomPanel = memo(function WisdomPanel() {
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<TransmuteResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [mode, setMode] = useState<'voice' | 'challenge' | 'structured'>('voice');

  const handleTransmute = useCallback(async () => {
    const trimmed = query.trim();
    if (!trimmed || loading) return;

    setLoading(true);
    setResult(null);
    setError(null);

    try {
      const raw = await cmd('run_awe_transmute', { query: trimmed, mode });
      const parsed: TransmuteResult = JSON.parse(raw);
      setResult(parsed);
    } catch (e) {
      setError(typeof e === 'string' ? e : 'AWE analysis failed. Ensure the AWE binary is built.');
    } finally {
      setLoading(false);
    }
  }, [query, mode, loading]);

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">Decision Analysis</h3>
        <p className="text-xs text-text-muted mt-1">
          What are you deciding? Get perspective from your history.
        </p>
      </div>

      <div className="p-5 space-y-3">
        {/* Query input */}
        <div className="flex gap-2">
          <input
            type="text"
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleTransmute()}
            placeholder="Should we migrate from X to Y?"
            className="flex-1 bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-sm text-text-primary placeholder:text-text-muted/50 focus:outline-none focus:border-text-muted/50"
          />
          <button
            onClick={handleTransmute}
            disabled={!query.trim() || loading}
            className="px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-sm text-text-secondary hover:text-white hover:border-text-muted/50 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          >
            {loading ? 'Analyzing...' : 'Analyze'}
          </button>
        </div>

        {/* Mode selector */}
        <div className="flex gap-2">
          {([
            { key: 'voice' as const, label: 'Perspective' },
            { key: 'structured' as const, label: 'Analysis' },
            { key: 'challenge' as const, label: 'Challenge' },
          ]).map(({ key, label }) => (
            <button
              key={key}
              onClick={() => setMode(key)}
              className={`text-xs px-2.5 py-1 rounded transition-colors ${
                mode === key
                  ? 'bg-bg-tertiary text-white border border-border'
                  : 'text-text-muted hover:text-text-secondary'
              }`}
            >
              {label}
            </button>
          ))}
        </div>

        {/* Loading state */}
        {loading && (
          <div className="bg-bg-primary rounded-lg border border-border/50 p-4">
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-accent-gold animate-pulse" />
              <span className="text-xs text-text-muted">
                Running 7-stage analysis pipeline...
              </span>
            </div>
          </div>
        )}

        {/* Error state */}
        {error && (
          <div className="bg-error/10 rounded-lg border border-error/20 p-4">
            <p className="text-xs text-error">{error}</p>
          </div>
        )}

        {/* Result */}
        {result && (
          <div className="bg-bg-primary rounded-lg border border-border/50 p-4 space-y-3">
            {/* Wisdom text */}
            <div className="text-sm text-text-primary leading-relaxed whitespace-pre-wrap">
              {result.wisdom}
            </div>

            {/* Confidence */}
            <div className="flex items-center gap-3 text-xs text-text-muted">
              <span>
                {(result.confidence * 100).toFixed(0)}% confidence
              </span>
              <span className="text-text-muted/40">|</span>
              <span>
                {result.mode === 'voice' ? 'Perspective' : result.mode === 'challenge' ? 'Challenge' : 'Analysis'}
              </span>
            </div>

            {/* Watch for */}
            {result.watch_for.length > 0 && (
              <div className="pt-2 border-t border-border/30">
                <p className="text-xs text-text-muted mb-1">Watch for:</p>
                {result.watch_for.map((w, i) => (
                  <p key={i} className="text-xs text-text-secondary ml-2">
                    {w}
                  </p>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
});
