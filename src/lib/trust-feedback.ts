// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { cmd } from './commands';

// ============================================================================
// Types
// ============================================================================

interface TrustFeedbackEvent {
  eventType: 'surfaced' | 'acted_on' | 'dismissed' | 'false_positive' | 'validated' | 'missed';
  signalId?: string;
  alertId?: string;
  sourceType?: string;
  topic?: string;
  notes?: string;
}

/** Queued event with timestamp for retry tracking */
interface QueuedFeedbackEvent {
  event: TrustFeedbackEvent;
  queuedAt: number;
  attempts: number;
}

// ============================================================================
// Constants
// ============================================================================

const QUEUE_STORAGE_KEY = '4da_feedback_queue';
const MAX_QUEUE_SIZE = 100;
const MAX_RETRY_ATTEMPTS = 5;

// ============================================================================
// Queue State
// ============================================================================

/** In-memory queue, hydrated from localStorage on init */
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
}) {
  const event: TrustFeedbackEvent = {
    eventType: params.eventType,
    signalId: params.signalId,
    alertId: params.alertId,
    sourceType: params.sourceType,
    topic: params.topic,
    notes: params.notes,
  };

  // Try to send immediately
  void sendEvent(event).catch(() => {
    // Backend call failed -- queue for retry
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
      } catch {
        queued.attempts += 1;
        if (queued.attempts < MAX_RETRY_ATTEMPTS) {
          failed.push(queued);
        }
        // Events exceeding MAX_RETRY_ATTEMPTS are dropped silently
      }
    }

    pendingQueue = failed;
    persistQueue();
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
  });
}

/** Add a failed event to the retry queue */
function enqueue(event: TrustFeedbackEvent): void {
  // Cap queue size to prevent unbounded growth
  if (pendingQueue.length >= MAX_QUEUE_SIZE) {
    // Drop oldest events to make room
    pendingQueue.shift();
  }

  pendingQueue.push({
    event,
    queuedAt: Date.now(),
    attempts: 1,
  });

  persistQueue();
}

/** Persist the queue to localStorage as fallback storage */
function persistQueue(): void {
  try {
    if (pendingQueue.length === 0) {
      localStorage.removeItem(QUEUE_STORAGE_KEY);
    } else {
      localStorage.setItem(QUEUE_STORAGE_KEY, JSON.stringify(pendingQueue));
    }
  } catch {
    // localStorage may be unavailable in some contexts -- ignore silently
  }
}

/** Load any persisted queue from localStorage */
function loadQueue(): void {
  try {
    const stored = localStorage.getItem(QUEUE_STORAGE_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      if (Array.isArray(parsed)) {
        pendingQueue = parsed.filter(
          (item: unknown): item is QueuedFeedbackEvent =>
            typeof item === 'object' &&
            item !== null &&
            'event' in item &&
            'queuedAt' in item &&
            'attempts' in item
        );
      }
    }
  } catch {
    // Corrupted data -- start fresh
    pendingQueue = [];
    localStorage.removeItem(QUEUE_STORAGE_KEY);
  }
}

// ============================================================================
// Module init: hydrate queue and flush on load
// ============================================================================

loadQueue();
if (pendingQueue.length > 0) {
  // Flush pending events after a short delay to avoid blocking app startup
  setTimeout(() => void flushPendingFeedback(), 2000);
}
