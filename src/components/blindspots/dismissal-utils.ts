// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

const DISMISS_STORAGE_KEY = 'blindspots_dismissed';
const DISMISS_TTL_MS = 7 * 24 * 60 * 60 * 1000;

export function loadPersistedDismissals(): Set<string> {
  try {
    const raw = localStorage.getItem(DISMISS_STORAGE_KEY);
    if (!raw) return new Set();
    const parsed = JSON.parse(raw) as Array<{ id: string; ts: number }>;
    const now = Date.now();
    const valid = parsed.filter(e => now - e.ts < DISMISS_TTL_MS);
    if (valid.length !== parsed.length) {
      localStorage.setItem(DISMISS_STORAGE_KEY, JSON.stringify(valid));
    }
    return new Set(valid.map(e => e.id));
  } catch { return new Set(); }
}

export function persistDismissal(id: string) {
  try {
    const raw = localStorage.getItem(DISMISS_STORAGE_KEY);
    const parsed: Array<{ id: string; ts: number }> = raw ? JSON.parse(raw) : [];
    parsed.push({ id, ts: Date.now() });
    localStorage.setItem(DISMISS_STORAGE_KEY, JSON.stringify(parsed));
  } catch { /* non-fatal */ }
}

export function removeDismissal(id: string) {
  try {
    const raw = localStorage.getItem(DISMISS_STORAGE_KEY);
    if (!raw) return;
    const parsed: Array<{ id: string; ts: number }> = JSON.parse(raw);
    localStorage.setItem(DISMISS_STORAGE_KEY, JSON.stringify(parsed.filter(e => e.id !== id)));
  } catch { /* non-fatal */ }
}
