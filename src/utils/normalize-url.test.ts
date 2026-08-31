// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { normalizeUrlForDedup } from './normalize-url';

// Parity contract: these cases mirror the backend's canonical implementation
// (`scoring::dedup::normalize_result_url` in src-tauri/src/scoring/dedup.rs).
describe('normalizeUrlForDedup', () => {
  it('returns null for missing or empty URLs', () => {
    expect(normalizeUrlForDedup(null)).toBeNull();
    expect(normalizeUrlForDedup(undefined)).toBeNull();
    expect(normalizeUrlForDedup('')).toBeNull();
    expect(normalizeUrlForDedup('   ')).toBeNull();
  });

  it('folds protocol, www, trailing slash, and case into one identity', () => {
    // The live 2026-08-31 Key Signals offender: one URL, three rows.
    const canonical = normalizeUrlForDedup('https://blog.wybxc.cc/blog/rust-gui-survey-2026/');
    expect(normalizeUrlForDedup('http://www.blog.wybxc.cc/Blog/Rust-GUI-Survey-2026')).toBe(
      canonical,
    );
    expect(normalizeUrlForDedup('https://blog.wybxc.cc/blog/rust-gui-survey-2026')).toBe(
      canonical,
    );
  });

  it('drops fragments and tracking params but keeps content params', () => {
    expect(
      normalizeUrlForDedup('https://safedep.io/arrayref?utm_source=hn&utm_medium=social#top'),
    ).toBe('https://safedep.io/arrayref');
    expect(normalizeUrlForDedup('https://example.com/a?ref=newsletter&fbclid=x')).toBe(
      'https://example.com/a',
    );
    // Content-bearing params survive — a YouTube video's query IS its identity.
    expect(normalizeUrlForDedup('https://youtube.com/watch?v=AAA')).not.toBe(
      normalizeUrlForDedup('https://youtube.com/watch?v=BBB'),
    );
  });

  it('sorts query params so order cannot defeat dedup, preserving value case', () => {
    expect(normalizeUrlForDedup('https://example.com/x?b=2&a=1')).toBe(
      normalizeUrlForDedup('https://example.com/x?a=1&b=2'),
    );
    // Values keep their case (they are identifiers).
    expect(normalizeUrlForDedup('https://youtube.com/watch?v=dQw4w9WgXcQ')).toBe(
      'https://youtube.com/watch?v=dQw4w9WgXcQ',
    );
  });
});
