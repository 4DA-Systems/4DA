// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// The Signal gate card. It used to promise "AI briefings" (not gated: they run
// on the user's own AI provider) and to offer a trial that had already run.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

import type { TrialOffer } from '../lib/trial-offer';

let license: { isPro: boolean; trialOffer: TrialOffer; expired: boolean; daysRemaining: number };
vi.mock('../hooks/use-license', () => ({
  useLicense: () => license,
}));

const startTrial = vi.fn();
const activateLicense = vi.fn();
vi.mock('../store', () => ({
  useAppStore: (selector: (s: Record<string, unknown>) => unknown) => selector({ startTrial, activateLicense }),
}));

import { ProGate } from './ProGate';

beforeEach(() => {
  license = { isPro: false, trialOffer: 'unavailable', expired: false, daysRemaining: 0 };
  startTrial.mockReset();
  activateLicense.mockReset();
});

function renderGate() {
  return render(<ProGate feature="Learning Opportunities"><div data-testid="gated-child" /></ProGate>);
}

describe('ProGate', () => {
  it('shows the content, ungated, to a Signal user', () => {
    license = { ...license, isPro: true };
    renderGate();
    expect(screen.getByTestId('gated-child')).toBeInTheDocument();
    expect(screen.queryByText('pro.upgrade')).not.toBeInTheDocument();
  });

  it('names the gated feature and what Signal adds, with no trial offer while the status is unknown', () => {
    renderGate();
    expect(screen.getByText('pro.featureGated')).toBeInTheDocument();
    expect(screen.getByText('pro.upgradeDescription')).toBeInTheDocument();
    expect(screen.getByText('pro.upgrade')).toBeInTheDocument();
    expect(screen.queryByText('pro.startTrial')).not.toBeInTheDocument();
  });

  it('says the trial has ended instead of offering it again', () => {
    license = { ...license, trialOffer: 'used' };
    renderGate();
    expect(screen.getByText('pro.trialEnded')).toBeInTheDocument();
    expect(screen.queryByText('pro.startTrial')).not.toBeInTheDocument();
  });

  it('asks an expired licence to renew, and offers no trial', () => {
    license = { ...license, expired: true };
    renderGate();
    expect(screen.getByText('pro.licenseExpired')).toBeInTheDocument();
    expect(screen.queryByText('pro.startTrial')).not.toBeInTheDocument();
  });

  it('offers the trial only when one can start, and says so when the start fails', async () => {
    license = { ...license, trialOffer: 'available' };
    startTrial.mockResolvedValue(false);
    renderGate();

    await act(async () => {
      fireEvent.click(screen.getByText('pro.startTrial'));
    });

    expect(startTrial).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('alert')).toHaveTextContent('pro.trialStartFailed');
  });

  it('shows the backend reason when a pasted licence key is refused', async () => {
    activateLicense.mockResolvedValue({ ok: false, reason: 'License key cannot be empty' });
    renderGate();

    fireEvent.click(screen.getByText('pro.haveLicenseKey'));
    fireEvent.change(screen.getByPlaceholderText('4DA-xxxxx.xxxxx'), { target: { value: '4DA-bad' } });
    await act(async () => {
      fireEvent.click(screen.getByText('action.activate'));
    });

    expect(activateLicense).toHaveBeenCalledWith('4DA-bad');
    expect(screen.getByText('License key cannot be empty')).toBeInTheDocument();
  });
});
