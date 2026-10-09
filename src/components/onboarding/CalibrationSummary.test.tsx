// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// The interest list shown after the taste test must match what was saved:
// a change appears only once persisted, and a failed save says so.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

import { CalibrationSummary } from './CalibrationSummary';

const summary = {
  dominantPersonaName: 'Systems Architect',
  dominantPersonaDescription: 'Low-level and infrastructure work',
  confidence: 0.8,
  itemsShown: 12,
  personaWeights: [],
  topInterests: ['rust', 'tokio'],
};

const renderSummary = () => render(<CalibrationSummary summary={summary} onContinue={vi.fn()} />);

/** The remove button on the first chip ('rust'). */
const firstRemoveButton = () => screen.getAllByLabelText('onboarding.calib.removeAria')[0] as HTMLElement;

function failWith(message: string) {
  cmdMock.mockImplementation(() => Promise.reject(message));
}

describe('CalibrationSummary', () => {
  beforeEach(() => { cmdMock.mockReset(); });

  it('keeps an interest on screen when removing it fails, and says why', async () => {
    failWith('settings locked');
    renderSummary();

    await act(async () => {
      fireEvent.click(firstRemoveButton());
    });

    expect(screen.getByText('rust')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('onboarding.calib.saveFailed');
  });

  it('removes an interest once the backend confirms', async () => {
    cmdMock.mockResolvedValue(undefined);
    renderSummary();

    await act(async () => {
      fireEvent.click(firstRemoveButton());
    });

    expect(cmdMock).toHaveBeenCalledWith('remove_interest', { topic: 'rust' });
    expect(screen.queryByText('rust')).not.toBeInTheDocument();
  });

  // Fresh-profile E2E 2026-10-09: five ML guesses under "These came from your
  // responses" for a user who skipped the ML card, headed "Python ML Engineer".
  it('shows guesses apart from liked topics, and never names a ruled-out persona as the user', () => {
    render(
      <CalibrationSummary
        summary={{
          ...summary,
          dominantPersonaName: 'Python ML Engineer',
          topInterests: ['SQLite', 'deep learning'],
          likedInterests: ['SQLite'],
          guessedInterests: ['deep learning'],
          personaContradicted: true,
        }}
        onContinue={vi.fn()}
      />,
    );

    expect(screen.queryByText('Python ML Engineer')).not.toBeInTheDocument();
    expect(screen.getByText('onboarding.calib.noClearProfile')).toBeInTheDocument();
    expect(screen.getByText('onboarding.calib.guessesLabel')).toBeInTheDocument();
    expect(screen.getByText('deep learning').closest('[data-guess]')).not.toBeNull();
    expect(screen.getByText('SQLite').closest('[data-guess]')).toBeNull();
  });

  it('a removed guess disappears once the backend confirms', async () => {
    cmdMock.mockResolvedValue(undefined);
    render(
      <CalibrationSummary
        summary={{ ...summary, likedInterests: ['rust'], guessedInterests: ['tokio'] }}
        onContinue={vi.fn()}
      />,
    );
    await act(async () => {
      fireEvent.click(screen.getAllByLabelText('onboarding.calib.removeAria')[1] as HTMLElement);
    });
    expect(cmdMock).toHaveBeenCalledWith('remove_interest', { topic: 'tokio' });
    expect(screen.queryByText('tokio')).not.toBeInTheDocument();
    expect(screen.queryByText('onboarding.calib.guessesLabel')).not.toBeInTheDocument();
  });

  it('a failed add keeps the draft for a retry and does not show the chip', async () => {
    failWith('disk full');
    renderSummary();
    const input = screen.getByLabelText('onboarding.calib.addPlaceholder');

    fireEvent.change(input, { target: { value: 'wasm' } });
    await act(async () => {
      fireEvent.click(screen.getByText('onboarding.calib.add'));
    });

    expect(screen.queryByText('wasm')).not.toBeInTheDocument();
    expect(input).toHaveValue('wasm');
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });
});
