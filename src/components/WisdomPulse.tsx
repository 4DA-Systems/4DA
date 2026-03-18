import { useState, useEffect, memo, useCallback } from 'react';
import { cmd } from '../lib/commands';

interface AweSummary {
  available: boolean;
  decisions: number;
  principles: number;
  pending: number;
  top_principle: string | null;
  health: string | null;
}

/**
 * WisdomPulse — your patterns, surfaced naturally.
 *
 * Shows validated patterns from your decision history.
 * Self-hides when the Wisdom Graph is empty.
 * Uses lightweight get_awe_summary (read-only, no sync).
 *
 * Language: "Your pattern" not "AWE detected."
 * Feel: memory, not surveillance.
 */
const CalibrationDetail = memo(function CalibrationDetail() {
  const [data, setData] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    cmd('run_awe_calibration', { domain: 'software-engineering' })
      .then(raw => setData(raw))
      .catch(() => setData(null))
      .finally(() => setLoaded(true));
  }, []);

  if (!loaded || !data) return null;

  return (
    <div className="text-xs text-text-muted/80 whitespace-pre-wrap font-mono">
      {data.split('\n').slice(0, 6).join('\n')}
    </div>
  );
});

export const WisdomPulse = memo(function WisdomPulse() {
  const [summary, setSummary] = useState<AweSummary | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [loaded, setLoaded] = useState(false);

  const loadSummary = useCallback(async () => {
    try {
      const raw = await cmd('get_awe_summary');
      const parsed: AweSummary = JSON.parse(raw);
      setSummary(parsed);
    } catch {
      setSummary(null);
    } finally {
      setLoaded(true);
    }
  }, []);

  useEffect(() => {
    loadSummary();
  }, [loadSummary]);

  // Don't render until loaded
  if (!loaded) return null;

  // Don't show if AWE unavailable or empty
  if (!summary?.available || summary.decisions === 0) return null;

  const hasPatterns = summary.principles > 0;
  const hasPending = summary.pending > 0;

  return (
    <button
      type="button"
      onClick={() => setExpanded(!expanded)}
      className="w-full text-left bg-bg-secondary rounded-lg border border-border/50 px-4 py-3 mb-4 hover:border-border transition-colors"
    >
      {/* Compact header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className={`w-1.5 h-1.5 rounded-full ${hasPatterns ? 'bg-success' : 'bg-text-muted/40'}`} />
          <span className="text-xs text-text-secondary">
            {hasPatterns
              ? `Your patterns: ${summary.principles} validated from ${summary.decisions} decisions`
              : `${summary.decisions} decisions tracked — patterns forming`
            }
          </span>
        </div>
        {hasPending && (
          <span className="text-xs text-accent-gold">
            {summary.pending} pending
          </span>
        )}
      </div>

      {/* Top principle — the most valuable line */}
      {hasPatterns && summary.top_principle && !expanded && (
        <p className="mt-1.5 text-xs text-text-muted pl-3.5 italic truncate">
          {summary.top_principle}
        </p>
      )}

      {/* Expanded detail */}
      {expanded && (
        <div className="mt-3 pt-3 border-t border-border/30 space-y-2">
          {summary.top_principle && (
            <div className="text-xs text-text-secondary">
              <span className="text-text-muted">Top pattern: </span>
              {summary.top_principle}
            </div>
          )}
          {summary.health && (
            <div className="text-xs text-text-muted">
              {summary.health}
            </div>
          )}
          <CalibrationDetail />
          {hasPending && (
            <p className="text-xs text-accent-gold/80">
              {summary.pending} decisions need outcome recording to improve accuracy.
            </p>
          )}
          <p className="text-xs text-text-muted/60">
            Each recorded outcome makes future recommendations more precise.
          </p>
        </div>
      )}
    </button>
  );
});
