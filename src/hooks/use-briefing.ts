import { useEffect, useRef } from 'react';
import type { SourceRelevance } from '../types';
import { useAppStore } from '../store';
import type { BriefingState } from '../store';

interface UseBriefingResult {
  aiBriefing: BriefingState;
  showBriefing: boolean;
  setShowBriefing: (show: boolean) => void;
  autoBriefingEnabled: boolean;
  setAutoBriefingEnabled: (enabled: boolean) => void;
  generateBriefing: () => Promise<void>;
}

/**
 * Briefing hook — thin wrapper around Zustand store.
 * All state lives in the store; this hook adds the auto-briefing trigger effect.
 */
export function useBriefing(
  relevanceResults: SourceRelevance[],
  analysisComplete: boolean,
): UseBriefingResult {
  const aiBriefing = useAppStore(s => s.aiBriefing);
  const showBriefing = useAppStore(s => s.showBriefing);
  const setShowBriefing = useAppStore(s => s.setShowBriefing);
  const autoBriefingEnabled = useAppStore(s => s.autoBriefingEnabled);
  const setAutoBriefingEnabled = useAppStore(s => s.setAutoBriefingEnabled);
  const generateBriefing = useAppStore(s => s.generateBriefing);

  // Track last briefing count locally (not externally consumed, just for auto-trigger dedup)
  const lastBriefingCountRef = useRef(0);
  const generatingBriefingRef = useRef(false);

  // Autonomous AI Briefing - triggers when analysis completes with new relevant items
  useEffect(() => {
    const totalCount = relevanceResults.length;

    if (
      autoBriefingEnabled &&
      analysisComplete &&
      totalCount > 0 &&
      !aiBriefing.loading &&
      !generatingBriefingRef.current &&
      totalCount !== lastBriefingCountRef.current
    ) {
      // Auto-generate briefing after analysis completes
      lastBriefingCountRef.current = totalCount;
      generatingBriefingRef.current = true;

      const briefingTimer = setTimeout(() => {
        generateBriefing().finally(() => {
          generatingBriefingRef.current = false;
        });
      }, 500);

      return () => {
        clearTimeout(briefingTimer);
        generatingBriefingRef.current = false;
      };
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- trigger on analysis complete and count change
  }, [analysisComplete, relevanceResults.length, autoBriefingEnabled, aiBriefing.loading]);

  return {
    aiBriefing,
    showBriefing,
    setShowBriefing,
    autoBriefingEnabled,
    setAutoBriefingEnabled,
    generateBriefing,
  };
}
