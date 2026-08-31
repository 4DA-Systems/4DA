// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Persisted Preemption dismissals (7-day TTL, localStorage).
 *
 * Storage is a per-user convenience and stays client-side, but the VISIBILITY
 * decision moved to the backend (AD-035): `loadPreemption` sends the live id
 * set as `get_preemption_alerts`' `dismissedIds`, and the command filters AND
 * counts with the same definition — the returned feed is exactly what the
 * screen renders. Nothing else may filter items out client-side.
 */

const DISMISS_STORAGE_KEY = 'preemption_dismissed';
const DISMISS_TTL_MS = 7 * 24 * 60 * 60 * 1000;

interface DismissalEntry {
  id: string;
  ts: number;
}

/** Load live (non-expired) dismissals, pruning expired entries in place. */
export function loadPersistedDismissals(): Set<string> {
  try {
    const raw = localStorage.getItem(DISMISS_STORAGE_KEY);
    if (!raw) return new Set();
    const parsed = JSON.parse(raw) as DismissalEntry[];
    const now = Date.now();
    const valid = parsed.filter(e => now - e.ts < DISMISS_TTL_MS);
    if (valid.length !== parsed.length) {
      localStorage.setItem(DISMISS_STORAGE_KEY, JSON.stringify(valid));
    }
    return new Set(valid.map(e => e.id));
  } catch { return new Set(); }
}

export function persistDismissal(id: string): void {
  try {
    const raw = localStorage.getItem(DISMISS_STORAGE_KEY);
    const parsed: DismissalEntry[] = raw ? JSON.parse(raw) : [];
    parsed.push({ id, ts: Date.now() });
    localStorage.setItem(DISMISS_STORAGE_KEY, JSON.stringify(parsed));
  } catch { /* non-fatal */ }
}

export function removeDismissal(id: string): void {
  try {
    const raw = localStorage.getItem(DISMISS_STORAGE_KEY);
    if (!raw) return;
    const parsed: DismissalEntry[] = JSON.parse(raw);
    localStorage.setItem(
      DISMISS_STORAGE_KEY,
      JSON.stringify(parsed.filter(e => e.id !== id)),
    );
  } catch { /* non-fatal */ }
}
