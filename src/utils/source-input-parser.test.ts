// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, expect, it } from 'vitest';
import { parseSourceInput } from './source-input-parser';

describe('parseSourceInput', () => {
  describe('RSS / URL handling', () => {
    it('accepts a full https URL as-is', () => {
      const r = parseSourceInput('https://blog.deno.com/feed.xml');
      expect(r.kind).toBe('rss');
      expect(r.value).toBe('https://blog.deno.com/feed.xml');
      expect(r.warnings).toHaveLength(0);
    });

    it('auto-prepends https:// for bare domains', () => {
      const r = parseSourceInput('www.deno.com/blog');
      expect(r.kind).toBe('rss');
      expect(r.value).toBe('https://www.deno.com/blog');
      expect(r.warnings[0]).toContain('Added "https://"');
    });

    it('handles the screenshot case (www.x.com)', () => {
      const r = parseSourceInput('www.x.com');
      // x.com is a known Twitter domain but without a handle path →
      // should not route to twitter-handle. Fall back to RSS with https.
      expect(r.value).toBe('https://www.x.com');
      expect(r.warnings[0]).toContain('Added "https://"');
    });

    it('labels feed-like URLs', () => {
      const r = parseSourceInput('https://example.com/feed.xml');
      expect(r.detected).toContain('RSS/Atom feed');
    });

    it('labels non-feed URLs for autodiscovery', () => {
      const r = parseSourceInput('https://example.com/blog');
      expect(r.detected).toContain('autodiscover');
    });

    it('rejects garbage', () => {
      const r = parseSourceInput('this is not a url');
      expect(r.kind).toBe('unknown');
    });
  });

  describe('YouTube handling', () => {
    it('extracts channel ID from /channel/UC... URL', () => {
      const r = parseSourceInput('https://youtube.com/channel/UCsBjURrPoezykLs9EqgamOA');
      expect(r.kind).toBe('youtube-channel-id');
      expect(r.value).toBe('UCsBjURrPoezykLs9EqgamOA');
    });

    it('extracts @handle from /@Name URL', () => {
      const r = parseSourceInput('https://www.youtube.com/@Fireship');
      expect(r.kind).toBe('youtube-handle');
      expect(r.value).toBe('@Fireship');
    });

    it('accepts bare UC-prefix IDs', () => {
      const r = parseSourceInput('UCsBjURrPoezykLs9EqgamOA');
      expect(r.kind).toBe('youtube-channel-id');
    });

    it('handles legacy /c/Name with a warning', () => {
      const r = parseSourceInput('https://youtube.com/c/Fireship');
      expect(r.kind).toBe('youtube-handle');
      expect(r.warnings.length).toBeGreaterThan(0);
    });
  });

  describe('Twitter/X handling', () => {
    it('extracts handle from x.com URL', () => {
      const r = parseSourceInput('https://x.com/dan_abramov');
      expect(r.kind).toBe('twitter-handle');
      expect(r.value).toBe('dan_abramov');
    });

    it('extracts handle from twitter.com URL', () => {
      const r = parseSourceInput('https://twitter.com/elonmusk');
      expect(r.kind).toBe('twitter-handle');
      expect(r.value).toBe('elonmusk');
    });

    it('accepts bare @handle as twitter', () => {
      const r = parseSourceInput('@elonmusk');
      expect(r.kind).toBe('twitter-handle');
      expect(r.value).toBe('elonmusk');
    });

    it('warns when bare @handle could be ambiguous', () => {
      const r = parseSourceInput('@elonmusk');
      // Should note it defaulted to Twitter
      expect(r.warnings.some(w => w.toLowerCase().includes('twitter') || w.toLowerCase().includes('x'))).toBe(true);
    });
  });

  describe('GitHub languages', () => {
    it('splits comma-separated list', () => {
      const r = parseSourceInput('rust, typescript, python');
      expect(r.kind).toBe('github-languages');
      expect(r.value).toBe('rust,typescript,python');
    });

    it('does not treat single-word input as languages', () => {
      const r = parseSourceInput('rust');
      // Single word without comma is ambiguous — don't auto-claim GitHub
      expect(r.kind).not.toBe('github-languages');
    });
  });

  describe('empty and edge cases', () => {
    it('returns unknown for empty input', () => {
      const r = parseSourceInput('');
      expect(r.kind).toBe('unknown');
    });

    it('returns unknown for whitespace', () => {
      const r = parseSourceInput('   ');
      expect(r.kind).toBe('unknown');
    });

    it('preserves original input for display', () => {
      const r = parseSourceInput('www.example.com');
      expect(r.original).toBe('www.example.com');
      expect(r.value).toBe('https://www.example.com');
    });
  });
});
