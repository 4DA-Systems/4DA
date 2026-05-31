// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, afterEach } from 'vitest';

import { isModK, modShortcutLabel, osFamily, __resetOsCacheForTests } from './platform';

function setPlatform(ua: string, platform = ''): void {
  Object.defineProperty(navigator, 'userAgent', { value: ua, configurable: true });
  try {
    Object.defineProperty(navigator, 'platform', { value: platform, configurable: true });
  } catch {
    /* some environments lock navigator.platform; userAgent is enough */
  }
  __resetOsCacheForTests();
}

const MAC_UA = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36';
const WIN_UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';
const LINUX_UA = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36';

function ev(partial: Partial<KeyboardEvent>): KeyboardEvent {
  return { key: 'k', metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...partial } as KeyboardEvent;
}

describe('platform detection', () => {
  afterEach(() => __resetOsCacheForTests());

  it('detects macOS, Windows, and Linux from the user agent', () => {
    setPlatform(MAC_UA);
    expect(osFamily()).toBe('mac');
    setPlatform(WIN_UA);
    expect(osFamily()).toBe('windows');
    setPlatform(LINUX_UA);
    expect(osFamily()).toBe('linux');
  });
});

describe('modShortcutLabel', () => {
  afterEach(() => __resetOsCacheForTests());

  it('renders ⌘K on macOS and Ctrl K elsewhere', () => {
    setPlatform(MAC_UA);
    expect(modShortcutLabel('K')).toBe('⌘K');
    setPlatform(WIN_UA);
    expect(modShortcutLabel('K')).toBe('Ctrl K');
    setPlatform(LINUX_UA);
    expect(modShortcutLabel('K')).toBe('Ctrl K');
  });
});

describe('isModK is platform-correct', () => {
  afterEach(() => __resetOsCacheForTests());

  it('matches Cmd+K on macOS but NOT Ctrl+K', () => {
    setPlatform(MAC_UA);
    expect(isModK(ev({ metaKey: true }))).toBe(true);
    expect(isModK(ev({ ctrlKey: true }))).toBe(false);
  });

  it('matches Ctrl+K on Windows/Linux but NOT Cmd/Meta+K', () => {
    setPlatform(WIN_UA);
    expect(isModK(ev({ ctrlKey: true }))).toBe(true);
    expect(isModK(ev({ metaKey: true }))).toBe(false);
  });

  it('rejects when alt or shift is also held', () => {
    setPlatform(WIN_UA);
    expect(isModK(ev({ ctrlKey: true, altKey: true }))).toBe(false);
    expect(isModK(ev({ ctrlKey: true, shiftKey: true }))).toBe(false);
  });

  it('is case-insensitive on the key and respects a custom key', () => {
    setPlatform(WIN_UA);
    expect(isModK(ev({ key: 'K', ctrlKey: true }))).toBe(true);
    expect(isModK(ev({ key: 'p', ctrlKey: true }), 'p')).toBe(true);
    expect(isModK(ev({ key: 'k', ctrlKey: true }), 'p')).toBe(false);
  });
});
