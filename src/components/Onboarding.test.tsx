// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Finishing onboarding must be honest about persistence: if the "setup
// finished" flag cannot be saved, the user is told and chooses, rather than
// entering the app and silently getting the wizard again next launch.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

vi.mock('./LanguageSwitcher', () => ({ LanguageSwitcher: () => null }));
vi.mock('./ThemeToggle', () => ({ ThemeToggle: () => null }));
vi.mock('./onboarding/CalibrationStep', () => ({
  CalibrationStep: ({ onComplete, finishing }: { onComplete: () => void; finishing?: boolean }) => (
    <button onClick={onComplete} disabled={finishing}>finish-setup</button>
  ),
}));

import { Onboarding } from './Onboarding';

const STEP_KEY = '4da-onboarding-wizard-step';

describe('Onboarding completion', () => {
  beforeEach(() => {
    cmdMock.mockReset();
    localStorage.setItem(STEP_KEY, 'calibrate');
  });

  it('enters the app and clears the resume key once completion is saved', async () => {
    cmdMock.mockResolvedValue(undefined);
    const onComplete = vi.fn();
    render(<Onboarding onComplete={onComplete} />);

    await act(async () => {
      fireEvent.click(screen.getByText('finish-setup'));
    });

    expect(onComplete).toHaveBeenCalledTimes(1);
    expect(localStorage.getItem(STEP_KEY)).toBeNull();
  });

  it('a save that keeps failing is shown, with Retry and Continue anyway', async () => {
    cmdMock.mockImplementation((command: string) =>
      command === 'mark_onboarding_complete' ? Promise.reject('disk full') : Promise.resolve(),
    );
    const onComplete = vi.fn();
    render(<Onboarding onComplete={onComplete} />);

    await act(async () => {
      fireEvent.click(screen.getByText('finish-setup'));
    });

    expect(onComplete).not.toHaveBeenCalled();
    expect(screen.getByRole('alert')).toHaveTextContent('onboarding.completion.saveFailed');
    // retried once before giving up
    expect(cmdMock.mock.calls.filter((c) => c[0] === 'mark_onboarding_complete')).toHaveLength(2);
    // the resume point survives, so a later launch picks up here
    expect(localStorage.getItem(STEP_KEY)).toBe('calibrate');

    fireEvent.click(screen.getByText('onboarding.completion.continueAnyway'));
    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  it('Retry succeeds after a transient failure', async () => {
    let failing = true;
    cmdMock.mockImplementation((command: string) =>
      command === 'mark_onboarding_complete' && failing ? Promise.reject('locked') : Promise.resolve(),
    );
    const onComplete = vi.fn();
    render(<Onboarding onComplete={onComplete} />);

    await act(async () => {
      fireEvent.click(screen.getByText('finish-setup'));
    });
    failing = false;
    await act(async () => {
      fireEvent.click(screen.getByText('action.retry'));
    });

    expect(onComplete).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
