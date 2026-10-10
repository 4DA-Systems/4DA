// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act } from '@testing-library/react';
import { CelebrationState, CELEBRATION_MOTION_MS } from './CelebrationState';

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
