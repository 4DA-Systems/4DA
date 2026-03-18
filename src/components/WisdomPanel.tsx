import { useState, memo, useCallback } from 'react';

/**
 * WisdomPanel — inline decision analysis in the Insights view.
 *
 * Lets users transmute decisions directly within 4DA.
 * Shows results as natural prose, not clinical analysis.
 * Lives alongside TechRadar and DecisionMemory.
 */
export const WisdomPanel = memo(function WisdomPanel() {
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [mode, setMode] = useState<'voice' | 'challenge' | 'structured'>('voice');

  const handleTransmute = useCallback(async () => {
    if (!query.trim() || loading) return;

    setLoading(true);
    setResult(null);

    try {
      // Call AWE via the sync_awe_wisdom command pattern
      // For full transmutation, we'd need a dedicated Tauri command
      // For now, surface a helpful message about using AWE CLI
      const trimmed = query.trim();
      setResult(
        `To transmute this decision, run:\n\n` +
        `awe transmute "${trimmed}" --${mode} -d software-engineering\n\n` +
        `Or ask Claude: "Use awe_transmute to analyze: ${trimmed}"`
      );
    } catch {
      setResult('AWE is not available. Ensure the AWE binary is built.');
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
            className="px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-sm text-text-secondary hover:text-white hover:border-text-muted/50 transition-colors disabled:opacity-30"
          >
            {loading ? '...' : 'Analyze'}
          </button>
        </div>

        {/* Mode selector */}
        <div className="flex gap-2">
          {(['voice', 'structured', 'challenge'] as const).map(m => (
            <button
              key={m}
              onClick={() => setMode(m)}
              className={`text-xs px-2.5 py-1 rounded transition-colors ${
                mode === m
                  ? 'bg-bg-tertiary text-white border border-border'
                  : 'text-text-muted hover:text-text-secondary'
              }`}
            >
              {m === 'voice' ? 'Perspective' : m === 'challenge' ? 'Challenge' : 'Analysis'}
            </button>
          ))}
        </div>

        {/* Result */}
        {result && (
          <div className="bg-bg-primary rounded-lg border border-border/50 p-4 mt-3">
            <pre className="text-xs text-text-secondary whitespace-pre-wrap font-sans leading-relaxed">
              {result}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
});
