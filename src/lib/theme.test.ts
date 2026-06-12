// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { getTheme, applyTheme, useTheme, THEME_STORAGE_KEY } from './theme';

describe('theme', () => {
  beforeEach(() => {
    localStorage.removeItem(THEME_STORAGE_KEY);
    document.documentElement.removeAttribute('data-theme');
  });

  afterEach(() => {
    localStorage.removeItem(THEME_STORAGE_KEY);
    document.documentElement.removeAttribute('data-theme');
  });

  it('defaults to dark with no stored preference', () => {
    expect(getTheme()).toBe('dark');
  });

  it('applyTheme(light) sets the html attribute and persists', () => {
    applyTheme('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe('light');
    expect(getTheme()).toBe('light');
  });

  it('applyTheme(dark) removes the attribute (dark = no attribute, brand default)', () => {
    applyTheme('light');
    applyTheme('dark');
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
    expect(getTheme()).toBe('dark');
  });

  it('ignores garbage stored values (treated as dark)', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'neon');
    expect(getTheme()).toBe('dark');
  });

  it('useTheme exposes state and toggle round-trips', () => {
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe('dark');
    expect(result.current.isLight).toBe(false);

    act(() => result.current.toggle());
    expect(result.current.theme).toBe('light');
    expect(result.current.isLight).toBe(true);
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');

    act(() => result.current.toggle());
    expect(result.current.theme).toBe('dark');
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
  });

  it('hook re-renders when applyTheme is called externally', () => {
    const { result } = renderHook(() => useTheme());
    act(() => applyTheme('light'));
    expect(result.current.theme).toBe('light');
  });
});
