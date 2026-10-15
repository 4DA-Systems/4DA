// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act, screen } from '@testing-library/react';
import { CelebrationState, CELEBRATION_MOTION_MS } from './CelebrationState';

vi.mock('../../lib/commands', () => ({ cmd: vi.fn(() => Promise.reject(new Error('offline'))) }));

vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setShowSettings: vi.fn(), setSettingsInitialTab: vi.fn() })),
}));

// Fresh-profile E2E 2026-10-10: the celebration's brand mark kept turning for
// as long as the user read the screen. It turns as the screen arrives, then
// rests — motion means "working" (#891), and the celebration is not.
describe('CelebrationState brand mark', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('turns on arrival, then rests', () => {
    const { container } = render(
      <CelebrationState
        relevantCount={8}
        totalCount={897}
        topSignal={null}
        stackInsights={[]}
        embeddingMode={null}
        onDismiss={vi.fn()}
      />,
    );
    const mark = () => container.querySelector('.brand-mark-container')!;
    expect(mark().getAttribute('data-motion')).not.toBe('rest');

    act(() => { vi.advanceTimersByTime(CELEBRATION_MOTION_MS); });
    expect(mark().getAttribute('data-motion')).toBe('rest');
  });
});

// Fresh-profile audit E7: after the scan the screen read "0 RELEVANT …
// Nothing matched yet" over a news count. With dependencies found it leads
// with them and their vulnerability check.
describe('CelebrationState after a project scan', () => {
  const summary = {
    projects_scanned: 3,
    total_dependencies: 1048,
    dependencies_by_ecosystem: { rust: 40, npm: 900, python: 108, other: 0 },
    languages: [], frameworks: [], primary_stack: '', key_packages: [], has_data: true,
    advisory_sync: {
      state: 'syncing' as const, ecosystems: 3, synced_at: null, error: null,
      in_flight: true, skipped_temp: 0,
    },
  };

  it('leads with the dependencies and the advisory check, never a news zero', () => {
    render(
      <CelebrationState
        relevantCount={0}
        totalCount={400}
        topSignal={null}
        stackInsights={[]}
        embeddingMode={null}
        scanSummary={summary}
        onDismiss={vi.fn()}
      />,
    );
    const deps = screen.getByTestId('first-run-dependencies');
    expect(deps).toHaveTextContent('1048');
    expect(deps).toHaveTextContent('firstRun.dependencies');
    expect(screen.getByTestId('first-run-advisory')).toHaveTextContent('firstRun.advisory.checking');
    expect(screen.queryByText('firstRun.relevantToYou')).toBeNull();
    expect(screen.queryByText(/Nothing matched yet/)).toBeNull();
    expect(screen.queryByText('firstRun.addSignalCta')).toBeNull();
  });

  it('without a scan, the relevance view is unchanged', () => {
    render(
      <CelebrationState
        relevantCount={5}
        totalCount={400}
        topSignal={null}
        stackInsights={[]}
        embeddingMode={null}
        onDismiss={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('first-run-dependencies')).toBeNull();
    expect(screen.getByText('firstRun.relevantToYou')).toBeInTheDocument();
  });
});