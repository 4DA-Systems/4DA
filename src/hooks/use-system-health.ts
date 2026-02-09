import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Anomaly, SystemHealth } from '../types';

export function useSystemHealth(onStatusChange?: (status: string) => void) {
  const [systemHealth, setSystemHealth] = useState<SystemHealth | null>(null);
  const [similarTopicQuery, setSimilarTopicQuery] = useState('');
  const [similarTopicResults, setSimilarTopicResults] = useState<Array<{ topic: string; similarity: number }>>([]);

  const setStatus = useCallback((status: string, duration = 3000) => {
    if (onStatusChange) {
      onStatusChange(status);
      if (duration > 0) {
        setTimeout(() => onStatusChange(''), duration);
      }
    }
  }, [onStatusChange]);

  const loadSystemHealth = useCallback(async () => {
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
      console.log('Anomalies not available:', error);
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
      console.log('Accuracy metrics not available:', error);
    }

    setSystemHealth({
      anomalies,
      anomalyCount,
      embeddingOperational,
      rateLimitStatus,
      accuracyMetrics,
    });
  }, []);

  const runAnomalyDetection = useCallback(async () => {
    try {
      setStatus('Running anomaly detection...', 0);
      const result = await invoke<{ anomalies: Anomaly[]; count: number }>('ace_detect_anomalies');
      await loadSystemHealth();
      setStatus(`Found ${result.count} anomalies`);
    } catch (error) {
      setStatus(`Error: ${error}`);
    }
  }, [loadSystemHealth, setStatus]);

  const resolveAnomaly = useCallback(async (anomalyId: number) => {
    try {
      await invoke('ace_resolve_anomaly', { anomalyId });
      setStatus('Anomaly resolved', 2000);
      await loadSystemHealth();
    } catch (error) {
      setStatus(`Error: ${error}`);
    }
  }, [loadSystemHealth, setStatus]);

  const findSimilarTopics = useCallback(async () => {
    if (!similarTopicQuery.trim()) return;
    try {
      const result = await invoke<{ query: string; results: Array<{ topic: string; similarity: number }> }>(
        'ace_find_similar_topics',
        { query: similarTopicQuery.trim(), topK: 5 },
      );
      setSimilarTopicResults(result.results || []);
    } catch (error) {
      setStatus(`Error: ${error}`);
    }
  }, [similarTopicQuery, setStatus]);

  const saveWatcherState = useCallback(async () => {
    try {
      await invoke('ace_save_watcher_state');
      setStatus('Watcher state saved', 2000);
    } catch (error) {
      setStatus(`Error: ${error}`);
    }
  }, [setStatus]);

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
