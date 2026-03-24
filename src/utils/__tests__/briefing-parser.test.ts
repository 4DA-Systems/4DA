/**
 * Tests for briefing content parser and related utilities.
 *
 * Covers parseBriefingContent, getRelativeTime, getFreshnessColor,
 * and section classification.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { parseBriefingContent, getRelativeTime, getFreshnessColor } from '../briefing-parser';

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
