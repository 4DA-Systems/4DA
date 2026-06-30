// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';

import { classifySignal, getSignalLabel, getSignalColor, findMostCriticalSave } from './WhatYouWouldHaveMissed';
import type { SourceRelevance } from '../types/analysis';

// Minimal SourceRelevance factory — only the fields the label/color + chooser read.
function item(partial: {
  signal_type?: string | null;
  content_type?: string | null;
  dep_match_score?: number;
  top_score?: number;
}): SourceRelevance {
  return {
    title: 'x',
    top_score: partial.top_score ?? 0.9,
    signal_type: partial.signal_type ?? null,
    score_breakdown: {
      content_type: partial.content_type ?? null,
      dep_match_score: partial.dep_match_score ?? 0,
    },
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any as SourceRelevance;
}

const RED = '#EF4444';
const GOLD = '#D4AF37';

describe('WhatYouWouldHaveMissed signal classification', () => {
  it('labels a content-vocab security advisory as Security advisory (the bug)', () => {
    // Real CVEs are tagged content_type="security_advisory" with signal_type unset.
    // Previously read content_type first against the signal-vocab switch -> null/gold.
    const it_ = item({ content_type: 'security_advisory', signal_type: null });
    expect(classifySignal(it_)).toBe('security');
    expect(getSignalLabel(it_)).toBe('Security advisory');
    expect(getSignalColor(it_)).toBe(RED);
  });

  it('labels a signal-vocab security alert as Security advisory', () => {
    const it_ = item({ signal_type: 'security_alert', content_type: null });
    expect(getSignalLabel(it_)).toBe('Security advisory');
    expect(getSignalColor(it_)).toBe(RED);
  });

  it('security takes precedence even mixed with a content type', () => {
    const it_ = item({ signal_type: 'security_alert', content_type: 'release_notes' });
    expect(getSignalLabel(it_)).toBe('Security advisory');
    expect(getSignalColor(it_)).toBe(RED);
  });

  it('labels the AI-CAD shape (tool_discovery + show_and_tell) as Tool discovery, not null', () => {
    // signal_type carried the type; content-first precedence used to drop the label.
    const it_ = item({ signal_type: 'tool_discovery', content_type: 'show_and_tell' });
    expect(getSignalLabel(it_)).toBe('Tool discovery');
  });

  it('labels breaking_change correctly from either vocabulary', () => {
    expect(getSignalLabel(item({ content_type: 'breaking_change' }))).toBe('Breaking change');
    expect(getSignalLabel(item({ signal_type: 'breaking_change' }))).toBe('Breaking change');
  });

  it('returns null label + gold for unrecognized content (no false labeling)', () => {
    const it_ = item({ content_type: 'release_notes', signal_type: null });
    expect(classifySignal(it_)).toBeNull();
    expect(getSignalLabel(it_)).toBeNull();
    expect(getSignalColor(it_)).toBe(GOLD);
  });
});

describe('findMostCriticalSave hero selection', () => {
  it('picks a dep-confirmed content-vocab CVE over a lower-priority tool item (the bug)', () => {
    // Real CVEs arrive as content_type="security_advisory" with signal_type unset.
    // The old chooser compared the signal-vocab string against both fields, so it
    // skipped this at the security tier and a Show HN won the hero card (bug_001).
    const cve = item({ content_type: 'security_advisory', signal_type: null, dep_match_score: 0.4, top_score: 0.72 });
    const showHn = item({ signal_type: 'tool_discovery', content_type: 'show_and_tell', dep_match_score: 0, top_score: 0.55 });
    expect(findMostCriticalSave([showHn, cve])).toBe(cve);
  });

  it('still requires dependency confirmation for security items', () => {
    // A security advisory with no dep match must NOT be hero'd just for being
    // security — an irrelevant CVE as hero card destroys trust. It loses to a
    // (dep-unconstrained) tool item in the priority walk.
    const irrelevantCve = item({ content_type: 'security_advisory', dep_match_score: 0, top_score: 0.9 });
    const tool = item({ signal_type: 'tool_discovery', dep_match_score: 0, top_score: 0.5 });
    expect(findMostCriticalSave([irrelevantCve, tool])).toBe(tool);
  });
});
