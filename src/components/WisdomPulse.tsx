import { useState, useEffect, memo, useCallback } from 'react';
import { cmd } from '../lib/commands';

interface WisdomState {
  topPrinciple: string | null;
  principleCount: number;
  decisionCount: number;
  pendingFeedback: number;
  loaded: boolean;
}

/**
 * WisdomPulse — your patterns, surfaced naturally.
 *
 * Shows validated patterns from your decision history.
 * Self-hides when the Wisdom Graph is empty (new users see nothing).
 * Gradually reveals as decisions accumulate.
 *
 * Language: "Your pattern" not "AWE detected."
 * Feel: memory, not surveillance.
 */
export const WisdomPulse = memo(function WisdomPulse() {
  const [state, setState] = useState<WisdomState>({
    topPrinciple: null,
    principleCount: 0,
    decisionCount: 0,
    pendingFeedback: 0,
    loaded: false,
  });
  const [expanded, setExpanded] = useState(false);

  const loadState = useCallback(async () => {
    try {
      // Sync wisdom (non-blocking, populates context chunks)
      const syncResult = await cmd('sync_awe_wisdom').catch(() => '');
      const statusText = typeof syncResult === 'string' ? syncResult : '';

      // Parse metrics
      const wisdomMatch = statusText.match(/(\d+) wisdom chunks/);
      const decisionsMatch = statusText.match(/(\d+) decisions/);
      const principleCount = wisdomMatch ? parseInt(wisdomMatch[1], 10) : 0;
      const decisionCount = decisionsMatch ? parseInt(decisionsMatch[1], 10) : 0;

      // Extract the top principle text from the sync
      // The sync command outputs principle text — extract the first one
      let topPrinciple: string | null = null;
      if (principleCount > 0) {
        // The principle content is embedded in context chunks
        // For display, we generate a natural-language summary
        if (decisionCount >= 5) {
          topPrinciple = `${decisionCount} decisions tracked, ${principleCount} patterns validated`;
        } else if (decisionCount > 0) {
          topPrinciple = `${decisionCount} decisions tracked — patterns emerging`;
        }
      }

      setState({
        topPrinciple,
        principleCount,
        decisionCount,
        pendingFeedback: 0,
        loaded: true,
      });
    } catch {
      setState(prev => ({ ...prev, loaded: true }));
    }
  }, []);

  useEffect(() => {
    loadState();
  }, [loadState]);

  // Don't show until the user has at least 1 decision
  if (!state.loaded || state.decisionCount === 0) {
    return null;
  }

  // Progressive disclosure based on decision count
  const hasPatterns = state.principleCount > 0;

  return (
    <button
      type="button"
      onClick={() => setExpanded(!expanded)}
      className="w-full text-left bg-bg-secondary rounded-lg border border-border/50 px-4 py-3 mb-4 hover:border-border transition-colors"
    >
      {/* Minimal header — feels like a quiet status line */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {hasPatterns ? (
            <div className="w-1.5 h-1.5 rounded-full bg-success" />
          ) : (
            <div className="w-1.5 h-1.5 rounded-full bg-text-muted/40" />
          )}
          <span className="text-xs text-text-secondary">
            {hasPatterns
              ? `Your pattern: ${state.principleCount} validated`
              : `${state.decisionCount} decisions — patterns forming`
            }
          </span>
        </div>
        {state.pendingFeedback > 0 && (
          <span className="text-xs text-accent-gold">
            {state.pendingFeedback} outcomes pending
          </span>
        )}
      </div>

      {/* Expanded detail — only when clicked */}
      {expanded && (
        <div className="mt-3 pt-3 border-t border-border/30 space-y-2">
          {state.topPrinciple && (
            <p className="text-xs text-text-secondary">
              {state.topPrinciple}
            </p>
          )}
          <p className="text-xs text-text-muted">
            Decisions compound into wisdom. Each outcome you record makes future recommendations more accurate.
          </p>
        </div>
      )}
    </button>
  );
});
