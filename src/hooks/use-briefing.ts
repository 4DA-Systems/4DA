import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SourceRelevance } from '../types';

interface BriefingState {
  content: string | null;
  loading: boolean;
  error: string | null;
  model: string | null;
  lastGenerated: Date | null;
}

const initialBriefingState: BriefingState = {
  content: null,
  loading: false,
  error: null,
  model: null,
  lastGenerated: null,
};

interface UseBriefingResult {
  aiBriefing: BriefingState;
  showBriefing: boolean;
  setShowBriefing: React.Dispatch<React.SetStateAction<boolean>>;
  autoBriefingEnabled: boolean;
  setAutoBriefingEnabled: React.Dispatch<React.SetStateAction<boolean>>;
  generateBriefing: () => Promise<void>;
}

export function useBriefing(
  relevanceResults: SourceRelevance[],
  analysisComplete: boolean,
): UseBriefingResult {
  const [aiBriefing, setAiBriefing] = useState<BriefingState>(initialBriefingState);
  const [showBriefing, setShowBriefing] = useState(false);
  const [autoBriefingEnabled, setAutoBriefingEnabled] = useState(true);
  const [lastBriefingCount, setLastBriefingCount] = useState(0);
  const generatingBriefingRef = useRef(false);

  const generateBriefing = useCallback(async () => {
    setAiBriefing(prev => ({ ...prev, loading: true, error: null }));
    try {
      const result = await invoke<{
        success: boolean;
        briefing: string | null;
        error?: string;
        model?: string;
        item_count?: number;
        latency_ms?: number;
      }>('generate_ai_briefing');

      if (result.success && result.briefing) {
        setAiBriefing({
          content: result.briefing,
          loading: false,
          error: null,
          model: result.model || null,
          lastGenerated: new Date(),
        });
        setShowBriefing(true);
      } else {
        setAiBriefing(prev => ({
          ...prev,
          loading: false,
          error: result.error || 'Failed to generate briefing',
        }));
      }
    } catch (error) {
      setAiBriefing(prev => ({
        ...prev,
        loading: false,
        error: `Error: ${error}`,
      }));
    }
  }, []);

  // Autonomous AI Briefing - triggers when analysis completes with new relevant items
  useEffect(() => {
    const totalCount = relevanceResults.length;

    if (
      autoBriefingEnabled &&
      analysisComplete &&
      totalCount > 0 &&
      !aiBriefing.loading &&
      !generatingBriefingRef.current &&
      totalCount !== lastBriefingCount
    ) {
      // Auto-generate briefing after analysis completes
      setLastBriefingCount(totalCount);
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
  // eslint-disable-next-line react-hooks/exhaustive-deps -- trigger on analysis complete and count change, not on full results array
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
