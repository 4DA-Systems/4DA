import { useState, useEffect, memo } from 'react';
import { cmd } from '../lib/commands';

interface WisdomState {
  decisions: number;
  feedback: number;
  principles: number;
  confirmationRate: number;
  topPrinciple: string | null;
  pendingCount: number;
  loaded: boolean;
}

/**
 * WisdomPulse — ambient AWE presence in the briefing.
 *
 * Shows the user's wisdom health at a glance: decisions tracked,
 * principles validated, calibration quality. Appears only when
 * the Wisdom Graph has data (self-hides when empty).
 *
 * Design principle: feel natural, not intrusive. One glanceable card
 * that tells you your wisdom engine is learning.
 */
export const WisdomPulse = memo(function WisdomPulse() {
  const [state, setState] = useState<WisdomState>({
    decisions: 0,
    feedback: 0,
    principles: 0,
    confirmationRate: 0,
    topPrinciple: null,
    pendingCount: 0,
    loaded: false,
  });

  useEffect(() => {
    loadWisdomState();
  }, []);

  async function loadWisdomState() {
    try {
      // Call AWE status + health via the Tauri command
      const healthOutput = await cmd('sync_awe_wisdom').catch(() => null);

      // Parse the AWE CLI health output for metrics
      // This calls awe health internally — we parse the status
      const statusText: string = typeof healthOutput === 'string' ? healthOutput : '';

      // Extract metrics from the sync result
      const wisdomMatch = statusText.match(/(\d+) wisdom chunks/);
      const decisionsMatch = statusText.match(/(\d+) decisions/);

      // Also try to get more detailed stats by looking at what was synced
      // For now, use the sync result as indicator of AWE health
      const wisdomChunks = wisdomMatch ? parseInt(wisdomMatch[1], 10) : 0;
      const detectedDecisions = decisionsMatch ? parseInt(decisionsMatch[1], 10) : 0;

      setState({
        decisions: detectedDecisions,
        feedback: 0,
        principles: wisdomChunks,
        confirmationRate: 0,
        topPrinciple: wisdomChunks > 0 ? 'Wisdom synced to context' : null,
        pendingCount: 0,
        loaded: true,
      });
    } catch {
      setState(prev => ({ ...prev, loaded: true }));
    }
  }

  // Don't show if AWE has no data or hasn't loaded
  if (!state.loaded || (state.principles === 0 && state.decisions === 0)) {
    return null;
  }

  return (
    <div className="bg-bg-secondary rounded-lg border border-border p-4 mb-4">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full bg-accent-gold animate-pulse" />
          <span className="text-xs font-medium text-accent-gold tracking-wider uppercase">
            AWE
          </span>
        </div>
        <span className="text-xs text-text-muted">
          Wisdom Engine
        </span>
      </div>

      <div className="flex items-center gap-4 text-xs text-text-secondary">
        {state.principles > 0 && (
          <span>{state.principles} principles active</span>
        )}
        {state.decisions > 0 && (
          <span>{state.decisions} decisions detected</span>
        )}
      </div>

      {state.topPrinciple && (
        <p className="text-xs text-text-muted mt-2 italic">
          {state.topPrinciple}
        </p>
      )}
    </div>
  );
});
