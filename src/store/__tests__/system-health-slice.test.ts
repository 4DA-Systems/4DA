// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('system-health-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has systemHealth null', () => {
      expect(useAppStore.getState().systemHealth).toBeNull();
    });

    it('has empty similarTopicQuery', () => {
      expect(useAppStore.getState().similarTopicQuery).toBe('');
    });

    it('has empty similarTopicResults', () => {
      expect(useAppStore.getState().similarTopicResults).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // setSimilarTopicQuery
  // ---------------------------------------------------------------------------
  describe('setSimilarTopicQuery', () => {
    it('updates similarTopicQuery', () => {
      useAppStore.getState().setSimilarTopicQuery('rust async');

      expect(useAppStore.getState().similarTopicQuery).toBe('rust async');
    });
  });

  // ---------------------------------------------------------------------------
  // loadSystemHealth
  // ---------------------------------------------------------------------------
  describe('loadSystemHealth', () => {
    it('aggregates health data from multiple invoke calls', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce({ anomalies: [{ id: 1, type: 'drift', message: 'Score drift detected' }], count: 1 }) // ace_get_unresolved_anomalies
        .mockResolvedValueOnce({ operational: true })   // ace_embedding_status
        .mockResolvedValueOnce({ global_remaining: 100, source_remaining: 50, is_limited: false }) // ace_get_rate_limit_status
        .mockResolvedValueOnce({ precision: 0.85, engagement_rate: 0.7, calibration_error: 0.05 }); // ace_get_accuracy_metrics

      await useAppStore.getState().loadSystemHealth();

      const health = useAppStore.getState().systemHealth;
      expect(health).not.toBeNull();
      expect(health!.anomalyCount).toBe(1);
      expect(health!.anomalies).toHaveLength(1);
      expect(health!.embeddingOperational).toBe(true);
      expect(health!.rateLimitStatus).toEqual({ global_remaining: 100, source_remaining: 50, is_limited: false });
      expect(health!.accuracyMetrics).toEqual({ precision: 0.85, engagement_rate: 0.7, calibration_error: 0.05 });
    });

    it('handles partial failures gracefully', async () => {
      vi.mocked(invoke)
        .mockRejectedValueOnce(new Error('anomalies fail'))  // ace_get_unresolved_anomalies
        .mockResolvedValueOnce({ operational: false })        // ace_embedding_status
        .mockRejectedValueOnce(new Error('rate limit fail'))  // ace_get_rate_limit_status
        .mockRejectedValueOnce(new Error('accuracy fail'));   // ace_get_accuracy_metrics

      await useAppStore.getState().loadSystemHealth();

      const health = useAppStore.getState().systemHealth;
      expect(health).not.toBeNull();
      expect(health!.anomalies).toEqual([]);
      expect(health!.anomalyCount).toBe(0);
      expect(health!.embeddingOperational).toBe(false);
      expect(health!.rateLimitStatus).toBeNull();
      expect(health!.accuracyMetrics).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // runAnomalyDetection
  // ---------------------------------------------------------------------------
  describe('runAnomalyDetection', () => {
    it('runs detection and reloads health', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce({ anomalies: [], count: 0 })  // ace_detect_anomalies
        // loadSystemHealth calls (4 invokes):
        .mockResolvedValueOnce({ anomalies: [], count: 0 })  // ace_get_unresolved_anomalies
        .mockResolvedValueOnce({ operational: true })         // ace_embedding_status
        .mockResolvedValueOnce({ global_remaining: 100, source_remaining: 50, is_limited: false }) // ace_get_rate_limit_status
        .mockResolvedValueOnce({ precision: 0.9, engagement_rate: 0.8, calibration_error: 0.02 }); // ace_get_accuracy_metrics

      await useAppStore.getState().runAnomalyDetection();

      expect(invoke).toHaveBeenCalledWith('ace_detect_anomalies', {});
    });

    it('handles errors gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('detection fail'));

      await useAppStore.getState().runAnomalyDetection();

      // Should not throw
    });
  });

  // ---------------------------------------------------------------------------
  // resolveAnomaly
  // ---------------------------------------------------------------------------
  describe('resolveAnomaly', () => {
    it('resolves anomaly and reloads health', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined)                     // ace_resolve_anomaly
        // loadSystemHealth calls (4 invokes):
        .mockResolvedValueOnce({ anomalies: [], count: 0 })
        .mockResolvedValueOnce({ operational: true })
        .mockResolvedValueOnce({ global_remaining: 100, source_remaining: 50, is_limited: false })
        .mockResolvedValueOnce({ precision: 0.9, engagement_rate: 0.8, calibration_error: 0.02 });

      await useAppStore.getState().resolveAnomaly(42);

      expect(invoke).toHaveBeenCalledWith('ace_resolve_anomaly', { anomalyId: 42 });
    });

    it('handles errors gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('resolve fail'));

      await useAppStore.getState().resolveAnomaly(42);

      // Should not throw
    });
  });

  // ---------------------------------------------------------------------------
  // findSimilarTopics
  // ---------------------------------------------------------------------------
  describe('findSimilarTopics', () => {
    it('finds similar topics for query', async () => {
      useAppStore.setState({ similarTopicQuery: 'machine learning' });
      const mockResult = {
        query: 'machine learning',
        results: [
          { topic: 'deep learning', similarity: 0.92 },
          { topic: 'neural networks', similarity: 0.88 },
        ],
      };
      vi.mocked(invoke).mockResolvedValueOnce(mockResult);

      await useAppStore.getState().findSimilarTopics();

      expect(invoke).toHaveBeenCalledWith('ace_find_similar_topics', { query: 'machine learning', topK: 5 });
      expect(useAppStore.getState().similarTopicResults).toHaveLength(2);
      expect(useAppStore.getState().similarTopicResults[0]!.topic).toBe('deep learning');
    });

    it('does nothing with empty query', async () => {
      useAppStore.setState({ similarTopicQuery: '   ' });

      await useAppStore.getState().findSimilarTopics();

      expect(invoke).not.toHaveBeenCalled();
    });

    it('handles errors gracefully', async () => {
      useAppStore.setState({ similarTopicQuery: 'test' });
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().findSimilarTopics();

      // Should not throw
    });
  });

  // ---------------------------------------------------------------------------
  // saveWatcherState
  // ---------------------------------------------------------------------------
  describe('saveWatcherState', () => {
    it('saves watcher state', async () => {
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await useAppStore.getState().saveWatcherState();

      expect(invoke).toHaveBeenCalledWith('ace_save_watcher_state', {});
    });

    it('handles errors gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().saveWatcherState();

      // Should not throw
    });
  });
});
