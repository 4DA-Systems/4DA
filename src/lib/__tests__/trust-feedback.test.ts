// SPDX-License-Identifier: FSL-1.1-Apache-2.0

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock the commands module before importing trust-feedback
vi.mock('../commands', () => ({
  cmd: vi.fn(),
}));

// Must import after mocking
import { cmd } from '../commands';
import { recordTrustEvent, getPendingFeedbackCount, flushPendingFeedback } from '../trust-feedback';

const mockedCmd = vi.mocked(cmd);

describe('trust-feedback', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
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

    recordTrustEvent({ eventType: 'dismissed', sourceType: 'security' });

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
});
