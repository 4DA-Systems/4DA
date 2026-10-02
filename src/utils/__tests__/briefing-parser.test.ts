// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tests for briefing content parser and related utilities.
 *
 * Covers parseBriefingContent, getRelativeTime, getFreshnessColor,
 * and section classification.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { parseBriefingContent, parseInline, getRelativeTime, getFreshnessColor } from '../briefing-parser';

describe('parseBriefingContent', () => {
  it('parses single section', () => {
    const content = '## Action Required\n- Update dependency X\n- Fix CVE';
    const sections = parseBriefingContent(content);
    expect(sections).toHaveLength(1);
    expect(sections[0]!.title).toBe('Action Required');
    expect(sections[0]!.type).toBe('action');
    expect(sections[0]!.lines).toHaveLength(2);
  });

  it('parses multiple sections', () => {
    const content = '## Action Required\n- Update X\n\n## Worth Knowing\n- New feature\n\n## Filtered Out\n- Noise';
    const sections = parseBriefingContent(content);
    expect(sections).toHaveLength(3);
    expect(sections[0]!.type).toBe('action');
    expect(sections[1]!.type).toBe('worth_knowing');
    expect(sections[2]!.type).toBe('filtered');
  });

  it('classifies action sections', () => {
    const content = '## Urgent Actions\n- Fix now';
    const sections = parseBriefingContent(content);
    expect(sections[0]!.type).toBe('action');
  });

  it('classifies critical as action type', () => {
    const content = '## Critical Alerts\n- Security issue';
    const sections = parseBriefingContent(content);
    expect(sections[0]!.type).toBe('action');
  });

  it('classifies worth_knowing sections', () => {
    const content = '## Notable Developments\n- New release';
    const sections = parseBriefingContent(content);
    expect(sections[0]!.type).toBe('worth_knowing');
  });

  it('classifies filtered sections', () => {
    const content = '## Filtered Out\n- Blog spam';
    const sections = parseBriefingContent(content);
    expect(sections[0]!.type).toBe('filtered');
  });

  it('classifies skip as filtered type', () => {
    const content = '## Skip These\n- Not relevant';
    const sections = parseBriefingContent(content);
    expect(sections[0]!.type).toBe('filtered');
  });

  it('defaults to general for unknown section titles', () => {
    const content = '## Overview\n- Summary of findings';
    const sections = parseBriefingContent(content);
    expect(sections[0]!.type).toBe('general');
  });

  it('handles content before first section as Overview', () => {
    const content = 'Some preamble text\n## Real Section\n- Content';
    const sections = parseBriefingContent(content);
    expect(sections).toHaveLength(2);
    expect(sections[0]!.title).toBe('Overview');
    expect(sections[0]!.lines).toContain('Some preamble text');
  });

  it('returns empty array for empty string', () => {
    expect(parseBriefingContent('')).toEqual([]);
  });

  it('handles section with no lines', () => {
    const content = '## Empty Section';
    const sections = parseBriefingContent(content);
    expect(sections).toHaveLength(1);
    expect(sections[0]!.lines).toEqual([]);
  });

  it('preserves empty lines within sections', () => {
    const content = '## Test\n- Line 1\n\n- Line 2';
    const sections = parseBriefingContent(content);
    expect(sections[0]!.lines).toHaveLength(3);
  });
});

describe('getRelativeTime', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns "Just now" for times less than 1 minute ago', () => {
    const now = new Date();
    expect(getRelativeTime(now)).toBe('Just now');
  });

  it('returns minutes for times less than 1 hour ago', () => {
    const thirtyMinsAgo = new Date(Date.now() - 30 * 60 * 1000);
    expect(getRelativeTime(thirtyMinsAgo)).toBe('30 min ago');
  });

  it('returns hours for times less than 24 hours ago', () => {
    const fiveHoursAgo = new Date(Date.now() - 5 * 3600 * 1000);
    expect(getRelativeTime(fiveHoursAgo)).toBe('5h ago');
  });

  it('returns "Yesterday" for exactly 1 day ago', () => {
    const yesterday = new Date(Date.now() - 24 * 3600 * 1000);
    expect(getRelativeTime(yesterday)).toBe('Yesterday');
  });

  it('returns days for multiple days ago', () => {
    const threeDays = new Date(Date.now() - 3 * 24 * 3600 * 1000);
    expect(getRelativeTime(threeDays)).toBe('3d ago');
  });
});

describe('getFreshnessColor', () => {
  it('returns green for less than 1 hour', () => {
    expect(getFreshnessColor(new Date())).toBe('text-green-400');
  });

  it('returns yellow for 1-4 hours', () => {
    const twoHoursAgo = new Date(Date.now() - 2 * 3600 * 1000);
    expect(getFreshnessColor(twoHoursAgo)).toBe('text-yellow-400');
  });

  it('returns orange for 4-12 hours', () => {
    const sixHoursAgo = new Date(Date.now() - 6 * 3600 * 1000);
    expect(getFreshnessColor(sixHoursAgo)).toBe('text-orange-400');
  });

  it('returns red for 12+ hours', () => {
    const dayAgo = new Date(Date.now() - 24 * 3600 * 1000);
    expect(getFreshnessColor(dayAgo)).toBe('text-red-400');
  });
});

describe('facts-first brief sections (Decision 2)', () => {
  it('classifies the four new sections', () => {
    const sections = parseBriefingContent(
      '## Act now\n- a\n## Upgrades to plan\n- b\n## Worth knowing\n- c\n## Still open\nd',
    );
    expect(sections.map(s => s.type)).toEqual(['action', 'upgrades', 'worth_knowing', 'still_open']);
  });
});

describe('parseInline', () => {
  it('splits bold, code and http(s) links', () => {
    expect(parseInline('Bump **rmcp** via `cargo update` — see [advisory](https://osv.dev/X).')).toEqual([
      { kind: 'text', text: 'Bump ' },
      { kind: 'bold', text: 'rmcp' },
      { kind: 'text', text: ' via ' },
      { kind: 'code', text: 'cargo update' },
      { kind: 'text', text: ' — see ' },
      { kind: 'link', text: 'advisory', url: 'https://osv.dev/X' },
      { kind: 'text', text: '.' },
    ]);
  });

  it('keeps a bold link label and never links a non-http URL', () => {
    expect(parseInline('[**fastembed 7.1.0**](https://crates.io/crates/fastembed)')).toEqual([
      { kind: 'link', text: 'fastembed 7.1.0', url: 'https://crates.io/crates/fastembed' },
    ]);
    const unsafe = parseInline('[click](javascript:alert(1))');
    expect(unsafe.some(s => s.kind === 'link')).toBe(false);
  });

  it('returns plain text unchanged', () => {
    expect(parseInline('Nothing new touches your code today.')).toEqual([
      { kind: 'text', text: 'Nothing new touches your code today.' },
    ]);
  });
});
