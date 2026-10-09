// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

let mockState: Record<string, unknown> = { isFirstRun: false };
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

vi.mock('./CalibrationView', () => ({
  CalibrationView: ({ open }: { open: boolean }) =>
    open ? <div data-testid="calibration-view" /> : null,
}));

import { CalibrationNudgeBanner } from './CalibrationNudgeBanner';

const mockInvoke = vi.mocked(invoke);
const DISMISS_KEY = '4da-calibration-nudge-dismissed';

function wireBackend(
  calibrated: boolean,
  labeledTotal: number,
  { ageDays = 30, labelable = 24 }: { ageDays?: number | null; labelable?: number } = {},
) {
  mockInvoke.mockImplementation((c: string) => {
    if (c === 'taste_test_is_calibrated') return Promise.resolve(calibrated);
    if (c === 'get_calibration_sprint_status')
      return Promise.resolve({
        labeledTotal,
        minFitSamples: 50,
        curveFitted: false,
        tasteTestAgeDays: calibrated ? ageDays : null,
      });
    if (c === 'get_calibration_sprint_items')
      return Promise.resolve(Array.from({ length: labelable }, (_, i) => ({ sourceItemId: i })));
    return Promise.resolve({});
  });
}

async function settled(container: HTMLElement) {
  await waitFor(() =>
    expect(mockInvoke).toHaveBeenCalledWith('get_calibration_sprint_status', expect.anything()),
  );
  await new Promise((r) => setTimeout(r, 0));
  expect(container.innerHTML).toBe('');
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  mockState = { isFirstRun: false };
});

// The banner has exactly one job: surface the dormant calibration door to
// installs that need it. These tests pin every gating condition so it can
// never nag a calibrated user or interrupt onboarding.
describe('CalibrationNudgeBanner — gating', () => {
  it('shows for an install that never took the taste test', async () => {
    wireBackend(false, 0);
    render(<CalibrationNudgeBanner />);
    await waitFor(() =>
      expect(screen.getByText('calibrationView.nudge.title')).toBeInTheDocument(),
    );
  });

  it('shows for a calibrated install with too few labels after days of real use', async () => {
    wireBackend(true, 4, { ageDays: 3.5, labelable: 12 });
    render(<CalibrationNudgeBanner />);
    await waitFor(() =>
      expect(screen.getByText('calibrationView.nudge.title')).toBeInTheDocument(),
    );
  });

  // Fresh-profile E2E 2026-10-07: the nudge appeared seconds after the taste
  // test (calibrated, 0 sprint labels) asking for labels nothing could supply.
  it('stays hidden right after the onboarding taste test', async () => {
    wireBackend(true, 0, { ageDays: 0.001 });
    const { container } = render(<CalibrationNudgeBanner />);
    await settled(container);
    expect(mockInvoke).not.toHaveBeenCalledWith('get_calibration_sprint_items', expect.anything());
  });

  it('stays hidden when there are too few scored items to label', async () => {
    wireBackend(true, 0, { ageDays: 10, labelable: 3 });
    const { container } = render(<CalibrationNudgeBanner />);
    await settled(container);
    expect(mockInvoke).toHaveBeenCalledWith('get_calibration_sprint_items', expect.anything());
  });

  it('stays hidden when calibrated AND labels are at the floor', async () => {
    wireBackend(true, 10);
    const { container } = render(<CalibrationNudgeBanner />);
    // Give the async gating a tick to (not) flip show.
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(container.innerHTML).toBe('');
  });

  it('never shows during first-run onboarding', async () => {
    mockState = { isFirstRun: true };
    wireBackend(false, 0);
    const { container } = render(<CalibrationNudgeBanner />);
    expect(container.innerHTML).toBe('');
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  // Fresh-profile E2E 2026-10-09: evaluated once while onboarding was still
  // on screen ("never took the taste test"), then shown right after the user
  // finished the taste test. A later state change must re-judge, not keep it.
  it('a verdict from before the taste test is dropped when first-run state changes', async () => {
    wireBackend(false, 0);
    const { container, rerender } = render(<CalibrationNudgeBanner />);
    await waitFor(() =>
      expect(screen.getByText('calibrationView.nudge.title')).toBeInTheDocument(),
    );

    // Onboarding completes: first run begins, and the test is now taken.
    wireBackend(true, 0, { ageDays: 0.001 });
    mockState = { isFirstRun: true };
    rerender(<CalibrationNudgeBanner />);
    await waitFor(() => expect(container.innerHTML).toBe(''));

    // Next launch (not first run): calibrated, nothing to label yet.
    mockState = { isFirstRun: false };
    rerender(<CalibrationNudgeBanner />);
    await settled(container);
  });

  it('respects a previous dismissal without asking the backend', () => {
    localStorage.setItem(DISMISS_KEY, '1');
    wireBackend(false, 0);
    const { container } = render(<CalibrationNudgeBanner />);
    expect(container.innerHTML).toBe('');
    expect(mockInvoke).not.toHaveBeenCalled();
  });
});

describe('CalibrationNudgeBanner — actions', () => {
  it('Not now dismisses and persists (one-time banner)', async () => {
    wireBackend(false, 0);
    const { container } = render(<CalibrationNudgeBanner />);
    await waitFor(() =>
      expect(screen.getByText('calibrationView.nudge.title')).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByText('calibrationView.nudge.dismiss'));
    expect(container.innerHTML).toBe('');
    expect(localStorage.getItem(DISMISS_KEY)).not.toBeNull();
  });

  it('Calibrate opens the calibration view and retires the banner', async () => {
    wireBackend(false, 0);
    render(<CalibrationNudgeBanner />);
    await waitFor(() =>
      expect(screen.getByText('calibrationView.nudge.title')).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByText('calibrationView.nudge.action'));
    expect(screen.getByTestId('calibration-view')).toBeInTheDocument();
    expect(localStorage.getItem(DISMISS_KEY)).not.toBeNull();
  });
});
