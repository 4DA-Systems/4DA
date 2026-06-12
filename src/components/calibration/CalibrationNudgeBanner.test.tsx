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

function wireBackend(calibrated: boolean, labeledTotal: number) {
  mockInvoke.mockImplementation((c: string) => {
    if (c === 'taste_test_is_calibrated') return Promise.resolve(calibrated);
    if (c === 'get_calibration_sprint_status')
      return Promise.resolve({ labeledTotal, minFitSamples: 50, curveFitted: false });
    return Promise.resolve({});
  });
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

  it('shows for a taste-calibrated install with too few explicit labels', async () => {
    wireBackend(true, 4);
    render(<CalibrationNudgeBanner />);
    await waitFor(() =>
      expect(screen.getByText('calibrationView.nudge.title')).toBeInTheDocument(),
    );
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
