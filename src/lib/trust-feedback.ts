// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { cmd } from './commands';

/**
 * Record a trust event when user interacts with intelligence.
 * Fire-and-forget — never blocks the UI.
 */
export function recordTrustEvent(params: {
  eventType: 'surfaced' | 'acted_on' | 'dismissed' | 'false_positive' | 'validated' | 'missed';
  signalId?: string;
  alertId?: string;
  sourceType?: string;
  topic?: string;
  notes?: string;
}) {
  cmd('record_intelligence_feedback', {
    eventType: params.eventType,
    signalId: params.signalId,
    alertId: params.alertId,
    sourceType: params.sourceType,
    topic: params.topic,
    notes: params.notes,
  }).catch(() => {
    // Fire-and-forget — trust events should never block the UI
  });
}
