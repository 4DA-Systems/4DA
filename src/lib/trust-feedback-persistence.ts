// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import type { TrustFeedbackEvent, QueuedFeedbackEvent } from './trust-feedback-types';

// ============================================================================
// Constants
// ============================================================================

const QUEUE_STORAGE_KEY = '4da_feedback_queue';

// ============================================================================
// localStorage Persistence
// ============================================================================

/** Persist the queue to localStorage as last-resort fallback */
export function persistQueueFallback(queue: QueuedFeedbackEvent[]): void {
  try {
    if (queue.length === 0) {
      localStorage.removeItem(QUEUE_STORAGE_KEY);
    } else {
      localStorage.setItem(QUEUE_STORAGE_KEY, JSON.stringify(queue));
    }
  } catch {
    // localStorage may be unavailable in some contexts -- ignore silently
  }
}

/** Clear the localStorage fallback (called when SQLite outbox has the data) */
export function clearLocalStorageFallback(): void {
  try {
    localStorage.removeItem(QUEUE_STORAGE_KEY);
  } catch {
    // ignore
  }
}

/** Sync localStorage: clear if all events are in SQLite, otherwise persist */
export function syncLocalStorageFallback(queue: QueuedFeedbackEvent[]): void {
  if (queue.length === 0 || queue.every(q => q.id != null)) {
    clearLocalStorageFallback();
  } else {
    persistQueueFallback(queue);
  }
}

function isQueuedFeedbackEvent(item: unknown): item is QueuedFeedbackEvent {
  return (
    typeof item === 'object' &&
    item !== null &&
    'event' in item &&
    'queuedAt' in item &&
    'attempts' in item
  );
}

export function loadLocalStorageFallback(): QueuedFeedbackEvent[] {
  try {
    const stored = localStorage.getItem(QUEUE_STORAGE_KEY);
    if (!stored) return [];
    const parsed = JSON.parse(stored);
    return Array.isArray(parsed) ? parsed.filter(isQueuedFeedbackEvent) : [];
  } catch {
    localStorage.removeItem(QUEUE_STORAGE_KEY);
    return [];
  }
}

// ============================================================================
// Queue Merging & Dedup
// ============================================================================

/** Composite key for deduplication — two events with the same key are considered identical */
export function dedupKey(event: TrustFeedbackEvent): string {
  return `${event.eventType}|${event.signalId ?? ''}|${event.alertId ?? ''}|${event.sourceType ?? ''}|${event.topic ?? ''}`;
}

export function mergeQueuedFeedback(
  items: QueuedFeedbackEvent[],
  maxQueueSize: number,
): QueuedFeedbackEvent[] {
  const byKey = new Map<string, QueuedFeedbackEvent>();
  for (const item of items) {
    const key = dedupKey(item.event);
    const existing = byKey.get(key);
    if (!existing || (existing.id == null && item.id != null)) {
      byKey.set(key, item);
    }
  }
  return Array.from(byKey.values()).slice(-maxQueueSize);
}

// ============================================================================
// SQLite Outbox Loading
// ============================================================================

/** Load pending events from SQLite outbox, falling back to localStorage */
export async function loadFromSqliteOutbox(
  getPendingFeedback: () => Promise<Array<{
    id: number;
    eventType: string;
    signalId?: string;
    alertId?: string;
    sourceType?: string;
    topic?: string;
    notes?: string;
    dismissReason?: string;
    dismissCategory?: string;
    queuedAt: number;
    attempts: number;
  }>>,
  maxQueueSize: number,
): Promise<QueuedFeedbackEvent[]> {
  const fallbackQueue = loadLocalStorageFallback();

  // Primary: load from SQLite outbox
  try {
    const rows = await getPendingFeedback();
    if (rows && rows.length > 0) {
      const sqliteQueue: QueuedFeedbackEvent[] = rows.map((row) => ({
        id: row.id,
        event: {
          eventType: row.eventType as TrustFeedbackEvent['eventType'],
          signalId: row.signalId ?? undefined,
          alertId: row.alertId ?? undefined,
          sourceType: row.sourceType ?? undefined,
          topic: row.topic ?? undefined,
          notes: row.notes ?? undefined,
          dismissReason: row.dismissReason ?? undefined,
          dismissCategory: row.dismissCategory ?? undefined,
        },
        queuedAt: row.queuedAt,
        attempts: row.attempts,
      }));
      const merged = mergeQueuedFeedback([...sqliteQueue, ...fallbackQueue], maxQueueSize);
      syncLocalStorageFallback(merged);
      return merged;
    }
  } catch {
    // SQLite outbox unavailable -- fall through to localStorage
  }

  const merged = mergeQueuedFeedback(fallbackQueue, maxQueueSize);
  syncLocalStorageFallback(merged);
  return merged;
}
