// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useRef } from 'react';
import type { SourceRelevance } from '../types';
import { useAppStore } from '../store';
import type { BriefingState } from '../store';

interface UseBriefingResult {
  aiBriefing: BriefingState;
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
  const autoBriefingEnabled = useAppStore(s => s.autoBriefingEnabled);
  const setAutoBriefingEnabled = useAppStore(s => s.setAutoBriefingEnabled);
  const generateBriefing = useAppStore(s => s.generateBriefing);

  // Track the timestamp of the last auto-briefing trigger (not count)
  const lastBriefingTriggerRef = useRef(0);
  const generatingBriefingRef = useRef(false);
  const prevAnalysisCompleteRef = useRef(false);
  const lastBackgroundAskRef = useRef(0);

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
        void generateBriefing({ auto: true }).finally(() => {
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

  // Background auto-refresh: when the brief is >2h old and new background
  // items have arrived, ask again as AUTO — the backend reuses today's brief
  // unless the facts it reports changed (or the day did).
  const lastBackgroundResultsAt = useAppStore(s => s.lastBackgroundResultsAt);
  useEffect(() => {
    if (
      !autoBriefingEnabled ||
      !lastBackgroundResultsAt ||
      !aiBriefing.lastGenerated ||
      aiBriefing.loading ||
      generatingBriefingRef.current
    ) return;

    const now = Date.now();
    const briefingAgeMs = now - aiBriefing.lastGenerated.getTime();
    const twoHoursMs = 2 * 60 * 60 * 1000;
    const hasNewItems = lastBackgroundResultsAt.getTime() > aiBriefing.lastGenerated.getTime();
    // A reused brief keeps its real (old) timestamp, so without this every
    // background cycle (~10 min) would re-ask; ask at most every two hours.
    const askedRecently = now - lastBackgroundAskRef.current < twoHoursMs;

    if (briefingAgeMs > twoHoursMs && hasNewItems && !askedRecently) {
      lastBackgroundAskRef.current = now;
      generatingBriefingRef.current = true;
      void generateBriefing({ auto: true }).finally(() => {
        generatingBriefingRef.current = false;
      });
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- trigger on background results
  }, [lastBackgroundResultsAt]);

  return {
    aiBriefing,
    autoBriefingEnabled,
    setAutoBriefingEnabled,
    generateBriefing,
  };
}
