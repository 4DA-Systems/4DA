// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { cmd } from './commands';
import {
  dedupKey,
  loadFromSqliteOutbox,
  mergeQueuedFeedback,
  persistQueueFallback,
  syncLocalStorageFallback,
} from './trust-feedback-persistence';

import type { TrustFeedbackEvent, QueuedFeedbackEvent } from './trust-feedback-types';

// Re-export types for consumers that imported them transitively
export type { TrustFeedbackEvent, QueuedFeedbackEvent } from './trust-feedback-types';

// ============================================================================
// Constants
// ============================================================================

const MAX_QUEUE_SIZE = 100;
const MAX_RETRY_ATTEMPTS = 5;

// ============================================================================
// Queue State
// ============================================================================

/** In-memory queue, hydrated from SQLite outbox (or localStorage fallback) on init */
let pendingQueue: QueuedFeedbackEvent[] = [];
let flushInProgress = false;

// ============================================================================
// Core API (public)
// ============================================================================

/**
 * Record a trust event when user interacts with intelligence.
 * Fire-and-forget from the caller's perspective -- never blocks the UI.
 * Internally queues events for retry if the backend call fails.
 */
export function recordTrustEvent(params: {
  eventType: 'surfaced' | 'acted_on' | 'dismissed' | 'false_positive' | 'validated' | 'missed';
  signalId?: string;
  alertId?: string;
  sourceType?: string;
  topic?: string;
  notes?: string;
  dismissReason?: string;
  dismissCategory?: string;
}) {
  const event: TrustFeedbackEvent = {
    eventType: params.eventType,
    signalId: params.signalId,
    alertId: params.alertId,
    sourceType: params.sourceType,
    topic: params.topic,
    notes: params.notes,
    dismissReason: params.dismissReason,
    dismissCategory: params.dismissCategory,
  };

  // Try to send immediately
  void sendEvent(event).catch(() => {
    // Backend call failed -- queue for durable retry
    enqueue(event);
  });
}

/**
 * Returns the number of feedback events waiting to be sent.
 * Useful for UI status indicators.
 */
export function getPendingFeedbackCount(): number {
  return pendingQueue.length;
}

/** @internal Reset queue state for test isolation. Not for production use. */
export function _resetQueueForTesting(): void {
  pendingQueue = [];
  flushInProgress = false;
}

/**
 * Flush any pending queued events to the backend.
 * Called automatically on module load (page load / app restart).
 * Can also be called manually to force a flush.
 */
export async function flushPendingFeedback(): Promise<void> {
  if (flushInProgress || pendingQueue.length === 0) return;
  flushInProgress = true;

  try {
    // Process a copy so new events can still be enqueued during flush
    const toProcess = [...pendingQueue];
    const failed: QueuedFeedbackEvent[] = [];

    for (const queued of toProcess) {
      try {
        await sendEvent(queued.event);
        // Mark as sent in the SQLite outbox
        if (queued.id != null) {
          try {
            await cmd('mark_feedback_sent', { outboxId: queued.id });
          } catch {
            // Non-fatal: the event was already delivered to the trust ledger
          }
        }
      } catch {
        queued.attempts += 1;
        // Record the attempt in the SQLite outbox
        if (queued.id != null) {
          try {
            await cmd('mark_feedback_attempt', { outboxId: queued.id });
          } catch {
            // Non-fatal: attempt tracking is best-effort
          }
        }
        if (queued.attempts < MAX_RETRY_ATTEMPTS) {
          failed.push(queued);
        } else if (queued.id != null) {
          // Mark as exhausted in SQLite so it's not reloaded on restart
          void cmd('mark_feedback_sent', { outboxId: queued.id }).catch(() => {});
        }
      }
    }

    const processed = new Set(toProcess);
    const queuedDuringFlush = pendingQueue.filter(q => !processed.has(q));
    pendingQueue = mergeQueuedFeedback([...failed, ...queuedDuringFlush], MAX_QUEUE_SIZE);
    syncLocalStorageFallback(pendingQueue);
  } finally {
    flushInProgress = false;
  }
}

// ============================================================================
// Internal
// ============================================================================

/** Send a single event to the backend via Tauri invoke */
async function sendEvent(event: TrustFeedbackEvent): Promise<void> {
  await cmd('record_intelligence_feedback', {
    eventType: event.eventType,
    signalId: event.signalId,
    alertId: event.alertId,
    sourceType: event.sourceType,
    topic: event.topic,
    notes: event.notes,
    dismissReason: event.dismissReason,
    dismissCategory: event.dismissCategory,
  });
}

/** Add a failed event to the retry queue, persisting to SQLite outbox */
function enqueue(event: TrustFeedbackEvent): void {
  // Dedup: skip if an identical event is already queued
  const key = dedupKey(event);
  if (pendingQueue.some(q => dedupKey(q.event) === key)) return;

  // Cap queue size to prevent unbounded growth
  if (pendingQueue.length >= MAX_QUEUE_SIZE) {
    // Drop oldest events to make room
    pendingQueue.shift();
  }

  const queued: QueuedFeedbackEvent = {
    event,
    queuedAt: Date.now(),
    attempts: 1,
  };

  pendingQueue.push(queued);

  // Persist to localStorage immediately (sync, always available)
  persistQueueFallback(pendingQueue);

  // Persist to SQLite outbox (durable across crashes)
  void cmd('queue_feedback_event', {
    eventType: event.eventType,
    signalId: event.signalId,
    alertId: event.alertId,
    sourceType: event.sourceType,
    topic: event.topic,
    notes: event.notes,
    dismissReason: event.dismissReason,
    dismissCategory: event.dismissCategory,
  })
    .then((rowId) => {
      // Store the outbox row id so flushPendingFeedback can mark it sent
      if (typeof rowId === 'number' && rowId > 0) {
        queued.id = rowId;
      }
      syncLocalStorageFallback(pendingQueue);
    })
    .catch(() => {
      // SQLite outbox failed -- localStorage already has the data, nothing to do
    });
}

// ============================================================================
// Module init: hydrate queue and flush on load
// ============================================================================

void loadFromSqliteOutbox(
  () => cmd('get_pending_feedback'),
  MAX_QUEUE_SIZE,
).then((loaded) => {
  pendingQueue = loaded;
  if (pendingQueue.length > 0) {
    // Flush pending events after a short delay to avoid blocking app startup
    setTimeout(() => void flushPendingFeedback(), 2000);
  }
});
