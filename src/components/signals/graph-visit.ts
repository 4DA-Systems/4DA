// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// "New since your last visit" for the Themes and Graph views: one stamp,
// shared, so switching views does not make everything new again.

const LAST_VIEW_KEY = '4da:graph:lastViewedAt';

export function readLastViewed(): number {
  try {
    const raw = localStorage.getItem(LAST_VIEW_KEY);
    return raw ? new Date(raw).getTime() || 0 : 0;
  } catch {
    return 0;
  }
}

export function markViewed() {
  try {
    localStorage.setItem(LAST_VIEW_KEY, new Date().toISOString());
  } catch {
    // Private window / blocked storage: "new" highlighting is a convenience.
  }
}

/** Read a remembered JSON value; null when absent, unreadable or blocked. */
export function readMemory<T>(key: string, valid: (v: unknown) => v is T): T | null {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return null;
    const v: unknown = JSON.parse(raw);
    return valid(v) ? v : null;
  } catch {
    return null;
  }
}

export function writeMemory(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Blocked storage: the next visit lays out fresh instead.
  }
}
