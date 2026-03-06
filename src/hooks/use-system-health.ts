import { useAppStore } from '../store';

/**
 * System health hook — thin wrapper around Zustand store.
 * Data is loaded lazily when the Health tab is first visited in SettingsModal.
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
