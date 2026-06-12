// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

vi.mock('../onboarding/TasteTestStep', () => ({
  TasteTestStep: ({ onComplete, onSkip }: { onComplete: () => void; onSkip: () => void }) => (
    <div data-testid="taste-test-step">
      <button data-testid="taste-complete" onClick={onComplete}>complete</button>
      <button data-testid="taste-skip" onClick={onSkip}>skip</button>
    </div>
  ),
}));

vi.mock('./SprintPhase', () => ({
  SprintPhase: ({ onClose }: { onClose: () => void }) => (
    <div data-testid="sprint-phase">
      <button data-testid="sprint-close" onClick={onClose}>close</button>
    </div>
  ),
}));

import { CalibrationView } from './CalibrationView';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
});

// The phase router is the contract of this surface: an install that has
// never taken the taste test gets it FIRST (the dormant onboarding flow
// finally reachable post-onboarding); an already-calibrated install goes
// straight to the review sprint.
describe('CalibrationView — phase routing', () => {
  it('renders nothing when closed and asks the backend nothing', () => {
    const { container } = render(<CalibrationView open={false} onClose={() => {}} />);
    expect(container.innerHTML).toBe('');
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('routes an uncalibrated install to the taste test first', async () => {
    mockInvoke.mockImplementation((c) =>
      c === 'taste_test_is_calibrated' ? Promise.resolve(false) : Promise.resolve({}),
    );
    render(<CalibrationView open={true} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('taste-test-step')).toBeInTheDocument());
    expect(screen.queryByTestId('sprint-phase')).not.toBeInTheDocument();
  });

  it('routes a taste-calibrated install straight to the sprint', async () => {
    mockInvoke.mockImplementation((c) =>
      c === 'taste_test_is_calibrated' ? Promise.resolve(true) : Promise.resolve({}),
    );
    render(<CalibrationView open={true} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('sprint-phase')).toBeInTheDocument());
    expect(screen.queryByTestId('taste-test-step')).not.toBeInTheDocument();
  });

  it('advances from taste test to sprint on completion', async () => {
    mockInvoke.mockImplementation((c) =>
      c === 'taste_test_is_calibrated' ? Promise.resolve(false) : Promise.resolve({}),
    );
    render(<CalibrationView open={true} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('taste-test-step')).toBeInTheDocument());
    screen.getByTestId('taste-complete').click();
    await waitFor(() => expect(screen.getByTestId('sprint-phase')).toBeInTheDocument());
  });

  it('skipping the taste test still lands on the sprint (labels remain collectable)', async () => {
    mockInvoke.mockImplementation((c) =>
      c === 'taste_test_is_calibrated' ? Promise.resolve(false) : Promise.resolve({}),
    );
    render(<CalibrationView open={true} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('taste-test-step')).toBeInTheDocument());
    screen.getByTestId('taste-skip').click();
    await waitFor(() => expect(screen.getByTestId('sprint-phase')).toBeInTheDocument());
  });

  it('falls back to the sprint when the calibration check fails', async () => {
    mockInvoke.mockImplementation((c) =>
      c === 'taste_test_is_calibrated' ? Promise.reject(new Error('ipc down')) : Promise.resolve({}),
    );
    render(<CalibrationView open={true} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('sprint-phase')).toBeInTheDocument());
  });
});
