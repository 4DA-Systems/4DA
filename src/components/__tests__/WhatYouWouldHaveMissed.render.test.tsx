// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { SourceRelevance } from '../../types';
import { makeItem } from '../../test/factories';

let mockResults: SourceRelevance[] = [];

vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) =>
      selector({
        appState: { relevanceResults: mockResults, analysisComplete: true },
        briefVerdicts: null,
      }),
    ),
    { getState: () => ({}) },
  ),
}));

import { WhatYouWouldHaveMissed } from '../WhatYouWouldHaveMissed';

function grounded(overrides: Partial<SourceRelevance> = {}): SourceRelevance {
  return makeItem({
    title: 'axios SSRF advisory affects your installed version',
    relevant: true,
    signal_type: 'security_alert',
    score_breakdown: { strongly_grounded: true, matched_deps: ['axios'] } as never,
    ...overrides,
  });
}

describe('WhatYouWouldHaveMissed — no vanity counters (doctrine rule 3)', () => {
  beforeEach(() => {
    mockResults = [];
  });

  it('shows the hero item and no scanned/rejected/filtered numbers', () => {
    const noise = Array.from({ length: 40 }, (_, i) =>
      makeItem({ id: 100 + i, title: `noise ${i}`, relevant: false, url: `https://e.com/${i}` }),
    );
    mockResults = [grounded({ id: 1 }), ...noise];
    const { container } = render(<WhatYouWouldHaveMissed />);

    expect(screen.getByText('axios SSRF advisory affects your installed version')).toBeTruthy();
    const text = container.textContent ?? '';
    // The removed counters: "40 noise rejected", "1 signal surfaced",
    // "97.6% filtered", "41 items scanned", "Ranked from 41 items scanned".
    expect(text).not.toMatch(/%/);
    expect(text).not.toMatch(/\b41\b/);
    expect(text).not.toMatch(/\b40\b/);
    expect(text.toLowerCase()).not.toContain('scanned');
    expect(text.toLowerCase()).not.toContain('rejected');
  });

  it('no longer hides behind a >= 80% rejection gate — the item is the story', () => {
    // Low rejection rate (1 of 3 rejected): the old card returned null here.
    mockResults = [
      grounded({ id: 1 }),
      makeItem({ id: 2, title: 'other signal', relevant: true, url: 'https://e.com/2' }),
      makeItem({ id: 3, title: 'noise', relevant: false, url: 'https://e.com/3' }),
    ];
    render(<WhatYouWouldHaveMissed />);
    expect(screen.getByText('axios SSRF advisory affects your installed version')).toBeTruthy();
  });

  it('renders nothing when nothing was surfaced', () => {
    mockResults = [makeItem({ id: 1, relevant: false })];
    const { container } = render(<WhatYouWouldHaveMissed />);
    expect(container.firstChild).toBeNull();
  });
});

// Fresh-profile E2E 2026-10-09 (B19): "No security, breaking change, or
// dependency alert needs your attention" sat directly above "Breaking upgrade:
// typescript 6.0.3 -> 7.0.2" in the Your-stack lane. The card must derive from
// the same list the lane shows.
describe('WhatYouWouldHaveMissed — agrees with the Your-stack lane below it', () => {
  beforeEach(() => {
    mockResults = [];
  });

  const breakingRelease = (overrides: Partial<SourceRelevance> = {}) => makeItem({
    id: 7,
    title: 'npm: typescript v7.0.2',
    relevant: true,
    signal_type: null as never,
    explanation: 'Breaking upgrade: typescript 6.0.3 → 7.0.2',
    score_breakdown: {
      content_type: 'release_notes',
      necessity_category: 'breaking_change',
      strongly_grounded: true,
      dependency_event: true,
      matched_deps: ['typescript'],
    } as never,
    ...overrides,
  });

  it('heroes a graded breaking upgrade instead of claiming the stack is clear', () => {
    mockResults = [
      breakingRelease(),
      makeItem({ id: 8, title: 'unrelated signal', relevant: true, url: 'https://e.com/8' }),
    ];
    const { container } = render(<WhatYouWouldHaveMissed />);
    expect(screen.getByText('npm: typescript v7.0.2')).toBeTruthy();
    expect(screen.getByText('Breaking change')).toBeTruthy();
    expect(container.textContent ?? '').not.toContain('missed.clearTitle');
    expect(container.textContent ?? '').not.toContain('missed.clearBody');
  });

  it('a minor release in the stack lane is not "no dependency alert"', () => {
    mockResults = [
      breakingRelease({
        title: 'npm: eslint v9.40.0',
        score_breakdown: {
          content_type: 'release_notes',
          necessity_category: 'ecosystem_shift',
          strongly_grounded: true,
          dependency_event: true,
          matched_deps: ['eslint'],
        } as never,
      }),
    ];
    const { container } = render(<WhatYouWouldHaveMissed />);
    const text = container.textContent ?? '';
    expect(text).toContain('missed.clearTitle');
    expect(text).toContain('missed.clearBodyUpdates');
    // ...and not the plain "no dependency alert" body.
    expect(text.replace('missed.clearBodyUpdates', '')).not.toContain('missed.clearBody');
  });

  it('keeps the plain clear state only when nothing in the stack lane exists', () => {
    mockResults = [makeItem({ id: 9, title: 'ambient', relevant: true, url: 'https://e.com/9' })];
    const { container } = render(<WhatYouWouldHaveMissed />);
    const text = container.textContent ?? '';
    expect(text).toContain('missed.clearTitle');
    expect(text).toContain('missed.clearBody');
    expect(text).not.toContain('missed.clearBodyUpdates');
    expect(text).not.toContain('missed.stackReview');
  });
});
