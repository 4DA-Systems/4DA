// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

  // Track the timestamp of the last auto-briefing trigger (not count)
  const lastBriefingTriggerRef = useRef(0);
  const generatingBriefingRef = useRef(false);
  const prevAnalysisCompleteRef = useRef(false);

  // Autonomous AI Briefing - triggers when analysisComplete transitions false→true
  useEffect(() => {
    const justCompleted = analysisComplete && !prevAnalysisCompleteRef.current;
    prevAnalysisCompleteRef.current = analysisComplete;

    if (
      autoBriefingEnabled &&
      justCompleted &&
      relevanceResults.length > 0 &&
      !aiBriefing.loading &&
      !generatingBriefingRef.current
    ) {
      // Debounce: don't re-trigger within 30 seconds
      const now = Date.now();
      if (now - lastBriefingTriggerRef.current < 30_000) return;
      lastBriefingTriggerRef.current = now;
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
  // eslint-disable-next-line react-hooks/exhaustive-deps -- trigger on analysis complete transition
  }, [analysisComplete, autoBriefingEnabled, aiBriefing.loading]);

  // Background auto-refresh: silently regenerate when briefing is >2h old
  // and new background items have arrived
  const lastBackgroundResultsAt = useAppStore(s => s.lastBackgroundResultsAt);
  useEffect(() => {
    if (
      !autoBriefingEnabled ||
      !lastBackgroundResultsAt ||
      !aiBriefing.lastGenerated ||
      aiBriefing.loading ||
      generatingBriefingRef.current
    ) return;

    const briefingAgeMs = Date.now() - aiBriefing.lastGenerated.getTime();
    const twoHoursMs = 2 * 60 * 60 * 1000;
    const hasNewItems = lastBackgroundResultsAt.getTime() > aiBriefing.lastGenerated.getTime();

    if (briefingAgeMs > twoHoursMs && hasNewItems) {
      generatingBriefingRef.current = true;
      generateBriefing().finally(() => {
        generatingBriefingRef.current = false;
      });
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- trigger on background results
  }, [lastBackgroundResultsAt]);

  return {
    aiBriefing,
    showBriefing,
    setShowBriefing,
    autoBriefingEnabled,
    setAutoBriefingEnabled,
    generateBriefing,
  };
}
