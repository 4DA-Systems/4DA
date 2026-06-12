// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useCallback, useSyncExternalStore } from 'react';

/**
 * Theme system — dark is the brand default ("the void"); light is the
 * founder-approved "Paper" variant (tokens in App.css under
 * [data-theme='light']). State lives in localStorage like the language
 * preference (`4da_language`), applied pre-paint by the inline script in
 * index.html. THE KEY MUST STAY IN SYNC with that script.
 */

export type Theme = 'dark' | 'light';

export const THEME_STORAGE_KEY = '4da-theme';
const THEME_EVENT = '4da-theme-changed';

export function getTheme(): Theme {
  try {
    return localStorage.getItem(THEME_STORAGE_KEY) === 'light' ? 'light' : 'dark';
  } catch {
    return 'dark';
  }
}

/** Sync the NATIVE window chrome (titlebar) with the webview theme. */
function applyNativeTheme(theme: Theme): void {
  import('@tauri-apps/api/window')
    .then(({ getCurrentWindow }) => getCurrentWindow().setTheme(theme))
    .catch(() => {
      // Non-Tauri environment (tests, browser) — webview-only theming.
    });
}

/** Set the theme on <html>, persist it, and notify subscribers. */
export function applyTheme(theme: Theme): void {
  if (theme === 'light') {
    document.documentElement.setAttribute('data-theme', 'light');
  } else {
    document.documentElement.removeAttribute('data-theme');
  }
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // Storage unavailable — theme still applies for this session.
  }
  applyNativeTheme(theme);
  window.dispatchEvent(new CustomEvent(THEME_EVENT));
}

/**
 * Boot-time init: the DOM attribute is already set pre-paint by the inline
 * script in index.html; this syncs the native titlebar to match. Called once
 * from main.tsx.
 */
export function initTheme(): void {
  applyNativeTheme(getTheme());
}

function subscribe(onChange: () => void): () => void {
  // Same-window changes via our custom event; cross-window via 'storage'.
  window.addEventListener(THEME_EVENT, onChange);
  window.addEventListener('storage', onChange);
  return () => {
    window.removeEventListener(THEME_EVENT, onChange);
    window.removeEventListener('storage', onChange);
  };
}

/** React hook: current theme + toggle. Re-renders subscribers on change. */
export function useTheme(): { theme: Theme; isLight: boolean; toggle: () => void } {
  const theme = useSyncExternalStore(subscribe, getTheme, (): Theme => 'dark');
  const toggle = useCallback(() => {
    applyTheme(getTheme() === 'light' ? 'dark' : 'light');
  }, []);
  return { theme, isLight: theme === 'light', toggle };
}

/**
 * Geometry palette for SVG components whose colors flow through presentation
 * attributes (which cannot resolve CSS var()). Values mirror the
 * --geometry-stroke / --geometry-glow tokens in App.css — keep in sync.
 */
export interface GeometryPalette {
  stroke: string;
  glow: string;
  glowOpacity: number;
  isLight: boolean;
}

export function useGeometryPalette(): GeometryPalette {
  const { isLight } = useTheme();
  return isLight
    ? { stroke: '#8F7118', glow: '#A8861D', glowOpacity: 0.1, isLight }
    : { stroke: '#C8B560', glow: '#D4AF37', glowOpacity: 0.2, isLight };
}
