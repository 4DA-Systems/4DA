// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore, SystemHealthSlice } from './types';

export const createSystemHealthSlice: StateCreator<AppStore, [], [], SystemHealthSlice> = (set, get) => ({
  systemHealth: null,
  similarTopicQuery: '',
  similarTopicResults: [],

  setSimilarTopicQuery: (q) => set({ similarTopicQuery: q }),

  loadSystemHealth: async () => {
    const [anomalyResult, embeddingResult, rateLimitResult, accuracyResult] =
      await Promise.allSettled([
        cmd('ace_get_unresolved_anomalies'),
        cmd('ace_embedding_status'),
        cmd('ace_get_rate_limit_status', { source: 'global' }),
        cmd('ace_get_accuracy_metrics'),
      ]);

    const anomalies = anomalyResult.status === 'fulfilled' ? (anomalyResult.value.anomalies || []) : [];
    const anomalyCount = anomalyResult.status === 'fulfilled' ? (anomalyResult.value.count || 0) : 0;
    const embeddingOperational = embeddingResult.status === 'fulfilled' ? (embeddingResult.value.operational ?? false) : false;
    const rateLimitStatus = rateLimitResult.status === 'fulfilled' ? rateLimitResult.value : null;
    const accuracyMetrics = accuracyResult.status === 'fulfilled' ? accuracyResult.value : null;

    set({
      systemHealth: {
        anomalies,
        anomalyCount,
        embeddingOperational,
        rateLimitStatus,
        accuracyMetrics,
      },
    });
  },

  runAnomalyDetection: async () => {
    const { loadSystemHealth, setSettingsStatus } = get();
    try {
      setSettingsStatus('Running anomaly detection...');
      const result = await cmd('ace_detect_anomalies');
      await loadSystemHealth();
      setSettingsStatus(`Found ${result.count} anomalies`);
      setTimeout(() => set({ settingsStatus: '' }), 3000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  resolveAnomaly: async (anomalyId) => {
    const { loadSystemHealth, setSettingsStatus } = get();
    try {
      await cmd('ace_resolve_anomaly', { anomalyId });
      setSettingsStatus('Anomaly resolved');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
      await loadSystemHealth();
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  findSimilarTopics: async () => {
    const { similarTopicQuery, setSettingsStatus } = get();
    if (!similarTopicQuery.trim()) return;
    try {
      const result = await cmd('ace_find_similar_topics', {
        query: similarTopicQuery.trim(),
        topK: 5,
      });
      set({ similarTopicResults: result.results || [] });
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  saveWatcherState: async () => {
    const { setSettingsStatus } = get();
    try {
      await cmd('ace_save_watcher_state');
      setSettingsStatus('Watcher state saved');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },
});
