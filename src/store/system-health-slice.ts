import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Anomaly } from '../types';
import type { AppStore, SystemHealthSlice, SimilarTopicResult } from './types';

export const createSystemHealthSlice: StateCreator<AppStore, [], [], SystemHealthSlice> = (set, get) => ({
  systemHealth: null,
  similarTopicQuery: '',
  similarTopicResults: [],

  setSimilarTopicQuery: (q) => set({ similarTopicQuery: q }),

  loadSystemHealth: async () => {
    let anomalies: Anomaly[] = [];
    let anomalyCount = 0;
    let embeddingOperational = false;
    let rateLimitStatus = null;
    let accuracyMetrics = null;

    try {
      const result = await invoke<{ anomalies: Anomaly[]; count: number }>('ace_get_unresolved_anomalies');
      anomalies = result.anomalies || [];
      anomalyCount = result.count || 0;
    } catch (error) {
      console.debug('Anomalies not available:', error);
    }

    try {
      const result = await invoke<{ operational: boolean }>('ace_embedding_status');
      embeddingOperational = result.operational ?? false;
    } catch (error) {
      console.error('Embedding status error:', error);
    }

    try {
      const result = await invoke<{ global_remaining: number; source_remaining: number; is_limited: boolean }>(
        'ace_get_rate_limit_status',
        { source: 'global' },
      );
      rateLimitStatus = result;
    } catch (error) {
      console.error('Rate limit status error:', error);
    }

    try {
      const result = await invoke<{ precision: number; engagement_rate: number; calibration_error: number }>(
        'ace_get_accuracy_metrics',
      );
      accuracyMetrics = result;
    } catch (error) {
      console.debug('Accuracy metrics not available:', error);
    }

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
      const result = await invoke<{ anomalies: Anomaly[]; count: number }>('ace_detect_anomalies');
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
      await invoke('ace_resolve_anomaly', { anomalyId });
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
      const result = await invoke<{ query: string; results: SimilarTopicResult[] }>(
        'ace_find_similar_topics',
        { query: similarTopicQuery.trim(), topK: 5 },
      );
      set({ similarTopicResults: result.results || [] });
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  saveWatcherState: async () => {
    const { setSettingsStatus } = get();
    try {
      await invoke('ace_save_watcher_state');
      setSettingsStatus('Watcher state saved');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },
});
