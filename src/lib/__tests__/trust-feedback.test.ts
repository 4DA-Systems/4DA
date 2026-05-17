// SPDX-License-Identifier: FSL-1.1-Apache-2.0

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock the commands module before importing trust-feedback
vi.mock('../commands', () => ({
  cmd: vi.fn(),
}));

// Must import after mocking
import { cmd } from '../commands';
import { recordTrustEvent, getPendingFeedbackCount, flushPendingFeedback, _resetQueueForTesting } from '../trust-feedback';

const mockedCmd = vi.mocked(cmd);

describe('trust-feedback', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    _resetQueueForTesting();
    // Default: cmd succeeds
    mockedCmd.mockResolvedValue(null as never);
  });

  afterEach(() => {
    localStorage.clear();
  });

  it('sends event to backend via cmd on success', async () => {
    recordTrustEvent({ eventType: 'acted_on', signalId: '42' });

    // Allow the async send to complete
    await vi.waitFor(() => {
      expect(mockedCmd).toHaveBeenCalledWith('record_intelligence_feedback', expect.objectContaining({
        eventType: 'acted_on',
        signalId: '42',
      }));
    });
  });

  it('queues event on backend failure and persists to SQLite outbox', async () => {
    // First cmd call (record_intelligence_feedback) fails, triggering enqueue.
    // Second cmd call (queue_feedback_event) succeeds — persisted to SQLite outbox.
    mockedCmd.mockRejectedValueOnce(new Error('Backend unavailable'));

    recordTrustEvent({ eventType: 'dismissed', sourceType: 'security' });

    // Wait for the async rejection to be handled and event queued
    await vi.waitFor(() => {
      expect(getPendingFeedbackCount()).toBeGreaterThanOrEqual(1);
    });

    // Verify the SQLite outbox command was called
    await vi.waitFor(() => {
      expect(mockedCmd).toHaveBeenCalledWith('queue_feedback_event', expect.objectContaining({
        eventType: 'dismissed',
        sourceType: 'security',
      }));
    });
  });

  it('falls back to localStorage when SQLite outbox also fails', async () => {
    // Both record_intelligence_feedback AND queue_feedback_event fail
    mockedCmd
      .mockRejectedValueOnce(new Error('Backend unavailable'))
      .mockRejectedValueOnce(new Error('SQLite unavailable'));

    recordTrustEvent({ eventType: 'dismissed', sourceType: 'vulnerability' });

    // Wait for both failures to cascade
    await vi.waitFor(() => {
      expect(getPendingFeedbackCount()).toBeGreaterThanOrEqual(1);
    });

    // Give time for the SQLite outbox failure to trigger localStorage fallback
    await vi.waitFor(() => {
      const stored = localStorage.getItem('4da_feedback_queue');
      expect(stored).toBeTruthy();
      const parsed = JSON.parse(stored!);
      expect(Array.isArray(parsed)).toBe(true);
      expect(parsed[0].event.eventType).toBe('dismissed');
    });
  });

  it('flushPendingFeedback retries queued events and clears on success', async () => {
    // First call fails, subsequent calls succeed
    mockedCmd
      .mockRejectedValueOnce(new Error('Backend unavailable'))
      .mockResolvedValue(null as never);

    recordTrustEvent({ eventType: 'validated', topic: 'rust' });

    // Wait for initial failure to queue
    await vi.waitFor(() => {
      expect(getPendingFeedbackCount()).toBeGreaterThanOrEqual(1);
    });

    // Now flush -- should retry and succeed
    await flushPendingFeedback();

    expect(getPendingFeedbackCount()).toBe(0);
  });

  it('does not alter the recordTrustEvent public API (fire-and-forget)', () => {
    // recordTrustEvent should return void (undefined), not a Promise
    const result = recordTrustEvent({ eventType: 'surfaced' });
    expect(result).toBeUndefined();
  });

  it('drops events exceeding MAX_RETRY_ATTEMPTS after repeated failures', async () => {
    // All calls fail
    mockedCmd.mockRejectedValue(new Error('Persistent failure'));

    recordTrustEvent({ eventType: 'false_positive' });

    // Wait for initial queue
    await vi.waitFor(() => {
      expect(getPendingFeedbackCount()).toBeGreaterThanOrEqual(1);
    });

    // Flush multiple times to exceed retry limit (MAX_RETRY_ATTEMPTS = 5)
    for (let i = 0; i < 6; i++) {
      await flushPendingFeedback();
    }

    // After exceeding max retries, event should be dropped
    expect(getPendingFeedbackCount()).toBe(0);
  });

  it('deduplicates identical events in the queue', async () => {
    // Both sends fail, triggering enqueue
    mockedCmd.mockRejectedValue(new Error('Backend unavailable'));

    recordTrustEvent({ eventType: 'acted_on', signalId: '99' });
    recordTrustEvent({ eventType: 'acted_on', signalId: '99' });

    // Wait for both failures to enqueue
    await vi.waitFor(() => {
      expect(getPendingFeedbackCount()).toBeGreaterThanOrEqual(1);
    });

    // Should only have 1 event despite 2 identical submissions
    expect(getPendingFeedbackCount()).toBe(1);
  });

  it('allows distinct events with different keys', async () => {
    mockedCmd.mockRejectedValue(new Error('Backend unavailable'));

    recordTrustEvent({ eventType: 'acted_on', signalId: '1' });
    recordTrustEvent({ eventType: 'dismissed', signalId: '1' });

    await vi.waitFor(() => {
      expect(getPendingFeedbackCount()).toBeGreaterThanOrEqual(2);
    });

    // Different eventType = different dedup key = both queued
    expect(getPendingFeedbackCount()).toBe(2);
  });

  it('marks exhausted events as sent in SQLite outbox on drop', async () => {
    // First call (record_intelligence_feedback) fails, triggering enqueue.
    // queue_feedback_event succeeds (gives us an outbox ID).
    // All subsequent record_intelligence_feedback calls fail (flush retries fail).
    let _callCount = 0;
    mockedCmd.mockImplementation(((command: string) => {
      _callCount++;
      if (command === 'record_intelligence_feedback') {
        return Promise.reject(new Error('Always fails'));
      }
      if (command === 'queue_feedback_event') {
        return Promise.resolve(42);
      }
      if (command === 'mark_feedback_attempt') {
        return Promise.resolve(null);
      }
      if (command === 'mark_feedback_sent') {
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    }) as typeof cmd);

    recordTrustEvent({ eventType: 'validated', signalId: 'exhaust-test' });

    await vi.waitFor(() => {
      expect(getPendingFeedbackCount()).toBeGreaterThanOrEqual(1);
    });

    // Flush 6 times to exceed MAX_RETRY_ATTEMPTS (5)
    for (let i = 0; i < 6; i++) {
      await flushPendingFeedback();
    }

    expect(getPendingFeedbackCount()).toBe(0);

    // Verify mark_feedback_sent was called with the real outbox row id (42)
    const markSentCalls = mockedCmd.mock.calls.filter(
      (call) => call[0] === 'mark_feedback_sent'
    );
    expect(markSentCalls.length).toBeGreaterThanOrEqual(1);
    expect(markSentCalls[0]![1]).toEqual(expect.objectContaining({ outboxId: 42 }));
  });

  it('keeps localStorage fallback when only some queued events reach SQLite', async () => {
    mockedCmd.mockImplementation(((command: string, params?: unknown) => {
      if (command === 'record_intelligence_feedback') {
        return Promise.reject(new Error('Backend unavailable'));
      }
      if (command === 'queue_feedback_event') {
        const signalId = (params as { signalId?: string } | undefined)?.signalId;
        return signalId === 'sqlite-ok'
          ? Promise.resolve(42)
          : Promise.reject(new Error('SQLite unavailable'));
      }
      return Promise.resolve(null);
    }) as typeof cmd);

    recordTrustEvent({ eventType: 'validated', signalId: 'sqlite-ok' });
    recordTrustEvent({ eventType: 'validated', signalId: 'sqlite-fail' });

    await vi.waitFor(() => {
      expect(getPendingFeedbackCount()).toBe(2);
    });

    await vi.waitFor(() => {
      const stored = localStorage.getItem('4da_feedback_queue');
      expect(stored).toBeTruthy();
      const parsed = JSON.parse(stored!);
      expect(parsed.some((item: { event: { signalId?: string } }) => item.event.signalId === 'sqlite-fail')).toBe(true);
    });
  });
});
