// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Platform detection + keyboard-modifier helpers.
 *
 * Centralizes the OS sniffing that was previously duplicated inline in
 * `use-update-check.ts` and friends. The OS cannot change at
 * runtime, so detection runs once and is memoized.
 *
 * The modifier helpers exist because a command shortcut must be PLATFORM
 * CORRECT: macOS users expect ⌘ (metaKey), Windows/Linux users expect Ctrl
 * (ctrlKey). The most common hand-rolled bug is binding `ctrlKey` globally —
 * which then also fires on macOS where Ctrl is a distinct, in-use modifier —
 * or binding `metaKey` on Windows where it's the OS "Windows key". `isModK`
 * gates on exactly the right modifier for the detected OS and rejects the
 * wrong one.
 */

export type OsFamily = 'mac' | 'windows' | 'linux';

interface UaDataNavigator extends Navigator {
  userAgentData?: { platform?: string };
}

function detectOs(): OsFamily {
  // `navigator.userAgentData.platform` is the modern, non-deprecated source.
  // Fall back to the userAgent / deprecated `navigator.platform` strings for
  // environments (older webviews) that don't expose it.
  const nav = navigator as UaDataNavigator;
  const hint = (nav.userAgentData?.platform ?? '').toLowerCase();
  const ua = navigator.userAgent?.toLowerCase() ?? '';
  const plat = navigator.platform?.toLowerCase() ?? '';
  const haystack = `${hint} ${ua} ${plat}`;

  if (haystack.includes('mac') || haystack.includes('iphone') || haystack.includes('ipad')) {
    return 'mac';
  }
  if (haystack.includes('win')) return 'windows';
  return 'linux';
}

let cachedOs: OsFamily | null = null;

/** The detected OS family, memoized after first call. */
export function osFamily(): OsFamily {
  if (cachedOs === null) cachedOs = detectOs();
  return cachedOs;
}

export function isMac(): boolean {
  return osFamily() === 'mac';
}

/**
 * Label for the primary command modifier: "⌘" on macOS, "Ctrl" elsewhere.
 */
export function modKeyLabel(): string {
  return isMac() ? '⌘' : 'Ctrl';
}

/**
 * Human-readable hint for a mod+<key> combo: "⌘K" on macOS (concatenated, per
 * Apple convention), "Ctrl K" on Windows/Linux (space-separated).
 */
export function modShortcutLabel(key: string): string {
  return isMac() ? `${modKeyLabel()}${key}` : `${modKeyLabel()} ${key}`;
}

/**
 * True when a keydown event is the primary command modifier held for `key` —
 * metaKey (⌘) on macOS, ctrlKey on Windows/Linux — and the cross-platform
 * WRONG modifier is NOT also held. Alt/Shift disqualify the match so we don't
 * swallow richer combos.
 */
export function isModK(e: KeyboardEvent, key = 'k'): boolean {
  if (e.key.toLowerCase() !== key.toLowerCase()) return false;
  if (e.altKey || e.shiftKey) return false;
  return isMac() ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
}

/** Test-only: clear the memoized OS so a test can re-detect under a stubbed navigator. */
export function __resetOsCacheForTests(): void {
  cachedOs = null;
}
