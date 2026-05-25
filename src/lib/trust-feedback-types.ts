// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

export interface TrustFeedbackEvent {
  eventType: 'surfaced' | 'acted_on' | 'dismissed' | 'false_positive' | 'validated' | 'missed';
  signalId?: string;
  alertId?: string;
  sourceType?: string;
  topic?: string;
  notes?: string;
  dismissReason?: string;
  dismissCategory?: string;
}

/** Queued event with outbox metadata for retry tracking */
export interface QueuedFeedbackEvent {
  /** SQLite outbox row id (present when persisted to outbox, absent for localStorage-only fallback) */
  id?: number;
  event: TrustFeedbackEvent;
  queuedAt: number;
  attempts: number;
}
