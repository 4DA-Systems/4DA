// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';

import {
  SLOW_AFTER_SECONDS,
  VERY_SLOW_AFTER_SECONDS,
  formatElapsed,
  startPace,
} from './boot-clock';
import { bootLanguages, bootStrings } from './boot-strings';
import { BOOT_TOKENS } from './boot-theme';
import { SUPPORTED_LANGUAGES } from '../i18n/languages';
import en from '../locales/en/boot.json';

describe('boot clock', () => {
  it('an ordinary start says nothing; a long one escalates once, then once more', () => {
    expect(startPace(0)).toBe('normal');
    expect(startPace(SLOW_AFTER_SECONDS - 1)).toBe('normal');
    expect(startPace(SLOW_AFTER_SECONDS)).toBe('slow');
    expect(startPace(VERY_SLOW_AFTER_SECONDS - 1)).toBe('slow');
    expect(startPace(VERY_SLOW_AFTER_SECONDS)).toBe('verySlow');
    expect(startPace(600)).toBe('verySlow');
  });

  it('formats the count as minutes and seconds', () => {
    expect(formatElapsed(0)).toBe('0:00');
    expect(formatElapsed(7)).toBe('0:07');
    expect(formatElapsed(75)).toBe('1:15');
    expect(formatElapsed(182.9)).toBe('3:02');
    expect(formatElapsed(-4)).toBe('0:00');
  });
});

describe('boot strings', () => {
  const keys = Object.keys(en) as Array<keyof typeof en>;

  it('every shipped language has every line, and none is blank', () => {
    const shipped = SUPPORTED_LANGUAGES.map((l) => l.code).sort();
    expect(bootLanguages().sort()).toEqual(shipped);
    for (const code of shipped) {
      const strings = bootStrings(code);
      for (const key of keys) {
        expect(strings[key].trim(), `${code}.${key}`).not.toBe('');
      }
    }
  });

  it('translations are translations, not the English copied over', () => {
    for (const { code } of SUPPORTED_LANGUAGES) {
      if (code === 'en') continue;
      const strings = bootStrings(code);
      for (const key of keys) {
        expect(strings[key], `${code}.${key}`).not.toBe(en[key]);
      }
    }
  });

  it('a language with no table reads in English', () => {
    expect(bootStrings('xx')).toEqual(en);
  });

  it('keeps the product name untranslated in every language', () => {
    for (const { code } of SUPPORTED_LANGUAGES) {
      expect(bootStrings(code).starting, code).toContain('4DA');
      expect(bootStrings(code).failed, code).toContain('4DA');
    }
  });
});

describe('boot theme', () => {
  // That the values equal App.css's is checked by scripts/boot-tokens.test.mjs:
  // Vitest does not load CSS, and this project's tests carry no node file access.
  it('defines the same tokens for both themes', () => {
    expect(Object.keys(BOOT_TOKENS.light).sort()).toEqual(Object.keys(BOOT_TOKENS.dark).sort());
  });
});
