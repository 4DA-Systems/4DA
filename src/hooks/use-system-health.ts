import { useEffect } from 'react';
import { useAppStore } from '../store';

/**
 * System health hook — thin wrapper around Zustand store.
 * All state lives in the store; this hook adds the init-load effect.
 */
export function useSystemHealth(_onStatusChange?: (status: string) => void) {
  const systemHealth = useAppStore(s => s.systemHealth);
  const similarTopicQuery = useAppStore(s => s.similarTopicQuery);
  const setSimilarTopicQuery = useAppStore(s => s.setSimilarTopicQuery);
  const similarTopicResults = useAppStore(s => s.similarTopicResults);
  const loadSystemHealth = useAppStore(s => s.loadSystemHealth);
  const runAnomalyDetection = useAppStore(s => s.runAnomalyDetection);
  const resolveAnomaly = useAppStore(s => s.resolveAnomaly);
  const findSimilarTopics = useAppStore(s => s.findSimilarTopics);
  const saveWatcherState = useAppStore(s => s.saveWatcherState);

  useEffect(() => {
    loadSystemHealth();
  }, [loadSystemHealth]);

  return {
    systemHealth,
    similarTopicQuery,
    setSimilarTopicQuery,
    similarTopicResults,
    loadSystemHealth,
    runAnomalyDetection,
    resolveAnomaly,
    findSimilarTopics,
    saveWatcherState,
  };
}
