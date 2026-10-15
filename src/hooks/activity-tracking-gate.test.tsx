// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Audit C10: the "Local activity tracking" opt-in (off by default) must stop
// every passive interaction write — expand dwell (click + engagement_complete)
// and title-link clicks — not just trackEvent. 743 `interactions` rows were
// found on an install whose opt-in was false.
import { fireEvent, render, renderHook, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { SourceRelevance } from '../types';
import { ResultItemCollapsed } from '../components/result-item/ResultItemCollapsed';
import { useExpandTracking } from './use-expand-tracking';
import { isActivityTrackingEnabled, setActivityTrackingEnabled, trackEvent } from './use-telemetry';

const cmdMock = vi.fn((..._args: unknown[]) => Promise.resolve(undefined));
vi.mock('../lib/commands', () => ({ cmd: (...args: unknown[]) => cmdMock(...args) }));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: string | { defaultValue?: string }) =>
      typeof options === 'string' ? options : options?.defaultValue ?? key,
  }),
}));

vi.mock('../components/ContentTranslationProvider', () => ({
  useTranslatedContent: () => ({ getTranslated: (_id: string, title: string) => title }),
}));

function interactionCalls() {
  return cmdMock.mock.calls.filter(([name]) => name === 'ace_record_interaction');
}

function expandThenCollapse() {
  const { rerender } = renderHook(
    ({ expanded }: { expanded: boolean }) => useExpandTracking(7, 'hackernews', expanded, ['rust']),
    { initialProps: { expanded: true } },
  );
  vi.advanceTimersByTime(30_000);
  rerender({ expanded: false });
}

const ITEM: SourceRelevance = {
  id: 9,
  title: 'tokio 1.40 released',
  url: 'https://example.com/tokio',
  top_score: 0.7,
  matches: [],
  relevant: true,
  source_type: 'hackernews',
};

function clickTitle() {
  render(
    <ResultItemCollapsed
      item={ITEM}
      isExpanded={false}
      onToggleExpand={() => {}}
      feedback={undefined}
      fallbackReason=""
    />,
  );
  fireEvent.click(screen.getByRole('link', { name: /tokio 1.40 released/ }));
}

describe('activity tracking opt-in gates passive interaction writes', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    cmdMock.mockClear();
  });
  afterEach(() => {
    vi.useRealTimers();
    setActivityTrackingEnabled(false);
  });

  it('is off until explicitly enabled', () => {
    setActivityTrackingEnabled(false);
    expect(isActivityTrackingEnabled()).toBe(false);
    trackEvent('view_open:brief');
    expect(cmdMock).not.toHaveBeenCalled();
  });

  it('records no expand dwell while opted out', () => {
    setActivityTrackingEnabled(false);
    expandThenCollapse();
    expect(interactionCalls()).toHaveLength(0);
  });

  it('records expand dwell once opted in', () => {
    setActivityTrackingEnabled(true);
    expandThenCollapse();
    const types = interactionCalls().map(([, args]) => (args as { actionType: string }).actionType);
    expect(types).toEqual(['click', 'engagement_complete']);
  });

  it('records no title click while opted out', () => {
    setActivityTrackingEnabled(false);
    clickTitle();
    expect(interactionCalls()).toHaveLength(0);
  });

  it('records the title click once opted in', () => {
    setActivityTrackingEnabled(true);
    clickTitle();
    expect(interactionCalls()).toHaveLength(1);
  });
});
