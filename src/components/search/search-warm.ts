// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Pre-warm the search backend when the palette opens, so the query embedder
 * loads while the user is still typing instead of inside their first search.
 * Fire-and-forget and throttled; the backend also skips a warm it ran recently.
 */

import { cmd } from '../../lib/commands';

/** One warm per this window, however often the palette opens. */
export const WARM_THROTTLE_MS = 20_000;

let lastWarmAt: number | null = null;

/** Returns true when a warm request was sent. */
export function warmSearchBackend(now: number = Date.now()): boolean {
  if (lastWarmAt !== null && now - lastWarmAt < WARM_THROTTLE_MS) return false;
  lastWarmAt = now;
  cmd('warm_search').catch(() => {
    /* best effort: a failed warm only means the first search pays the load */
  });
  return true;
}

/** Test seam: forget the last warm. */
export function resetSearchWarmForTest(): void {
  lastWarmAt = null;
}
