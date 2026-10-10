// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';

import { ReportAge, reportAgeMs, REPORT_AGE_LABEL_AFTER_MS, REPORT_RELOAD_DELAYS_MS } from './ReportAge';

// Audit 2026-10-07 follow-up (wave 9b): after a restart Blind Spots serves the
// previous run's persisted report while it rebuilds. It must say how old that
// report is, and swap in the rebuilt one without a manual reload.

const ago = (ms: number) => new Date(Date.now() - ms).toISOString();

describe('ReportAge', () => {
  beforeEach(() => { vi.useFakeTimers({ shouldAdvanceTime: false }); });
  afterEach(() => { vi.useRealTimers(); });

  it('says nothing about a report from this cycle', () => {
    const onReload = vi.fn();
    const { container } = render(<ReportAge computedAt={ago(5 * 60 * 1000)} onReload={onReload} />);
    expect(container).toBeEmptyDOMElement();
    act(() => { vi.advanceTimersByTime(60_000); });
    expect(onReload).not.toHaveBeenCalled();
  });

  it('labels a report older than a cycle with when it was computed', () => {
    render(<ReportAge computedAt={ago(3 * 60 * 60 * 1000)} onReload={vi.fn()} />);
    // The test i18n renders keys; the en string is "Analysis computed {{age}}".
    expect(screen.getByTestId('blindspots-report-age').textContent).toBe('blindspots.reportAge');
  });

  it('takes each surface its own label and test id (Preemption, Knowledge Gaps)', () => {
    render(
      <ReportAge
        computedAt={ago(5 * 60 * 60 * 1000)}
        onReload={vi.fn()}
        i18nKey="preemption.reportAge"
        testId="preemption-report-age"
      />,
    );
    expect(screen.getByTestId('preemption-report-age').textContent).toBe('preemption.reportAge');
  });

  it('re-asks twice for the rebuilt report, then stops', () => {
    const onReload = vi.fn();
    render(<ReportAge computedAt={ago(REPORT_AGE_LABEL_AFTER_MS + 1000)} onReload={onReload} />);
    act(() => { vi.advanceTimersByTime(REPORT_RELOAD_DELAYS_MS[0]); });
    expect(onReload).toHaveBeenCalledTimes(1);
    act(() => { vi.advanceTimersByTime(REPORT_RELOAD_DELAYS_MS[1]); });
    expect(onReload).toHaveBeenCalledTimes(2);
    act(() => { vi.advanceTimersByTime(10 * 60_000); });
    expect(onReload).toHaveBeenCalledTimes(2);
  });

  it('stops re-asking once the fresh report arrives', () => {
    const onReload = vi.fn();
    const { rerender, container } = render(
      <ReportAge computedAt={ago(2 * 60 * 60 * 1000)} onReload={onReload} />,
    );
    rerender(<ReportAge computedAt={ago(0)} onReload={onReload} />);
    expect(container).toBeEmptyDOMElement();
    act(() => { vi.advanceTimersByTime(10 * 60_000); });
    expect(onReload).not.toHaveBeenCalled();
  });

  it('renders nothing for a missing or unparseable timestamp', () => {
    const { container } = render(<ReportAge computedAt={null} onReload={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
    expect(reportAgeMs(undefined)).toBeNull();
    expect(reportAgeMs('not a date')).toBeNull();
    expect(reportAgeMs('2026-10-10T00:00:00Z', Date.parse('2026-10-10T01:00:00Z'))).toBe(3_600_000);
  });
});
