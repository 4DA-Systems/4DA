// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// The trial starts by itself on first launch, so by the time a paywall shows
// it has almost always run. The CTA still offered "Start 14-Day Free Trial"
// whenever the trial status had not loaded, and ignored a refused start: a
// button that did nothing. These tests pin what the buttons may claim.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

import type { TrialOffer } from '../lib/trial-offer';

let trialOffer: TrialOffer = 'unavailable';
vi.mock('../hooks/use-license', () => ({
  useLicense: () => ({ isPro: false, trialOffer }),
}));

const startTrial = vi.fn();
vi.mock('../store', () => ({
  useAppStore: (selector: (s: Record<string, unknown>) => unknown) => selector({ startTrial }),
}));

import { SignalUpgradeCTA } from './SignalUpgradeCTA';

beforeEach(() => {
  trialOffer = 'unavailable';
  startTrial.mockReset();
});

describe('SignalUpgradeCTA', () => {
  it('always links to the upgrade page, tagged with the surface that rendered it', () => {
    render(<SignalUpgradeCTA source="blindspots-paywall" />);
    const link = screen.getByText('pro.upgrade');
    expect(link).toHaveAttribute('href', expect.stringContaining('https://4da.ai/signal?'));
    expect(link).toHaveAttribute('href', expect.stringContaining('utm_content=blindspots-paywall'));
  });

  it('does not offer a trial while the trial status is unknown', () => {
    trialOffer = 'unavailable';
    render(<SignalUpgradeCTA />);
    expect(screen.queryByText('pro.startTrial')).not.toBeInTheDocument();
  });

  it('does not offer a trial that already ran, and says it ended', () => {
    trialOffer = 'used';
    render(<SignalUpgradeCTA />);
    expect(screen.queryByText('pro.startTrial')).not.toBeInTheDocument();
    expect(screen.getByTestId('signal-cta-trial-used')).toHaveTextContent('pro.trialEnded');
  });

  it('keeps compact rows to the upgrade button when the trial already ran', () => {
    trialOffer = 'used';
    render(<SignalUpgradeCTA compact />);
    expect(screen.queryByText('pro.startTrial')).not.toBeInTheDocument();
    expect(screen.queryByTestId('signal-cta-trial-used')).not.toBeInTheDocument();
  });

  it('does not offer a trial while one is running', () => {
    trialOffer = 'active';
    render(<SignalUpgradeCTA />);
    expect(screen.queryByText('pro.startTrial')).not.toBeInTheDocument();
    expect(screen.queryByTestId('signal-cta-trial-used')).not.toBeInTheDocument();
  });

  it('offers the trial when one can start, and starts it', async () => {
    trialOffer = 'available';
    startTrial.mockResolvedValue(true);
    render(<SignalUpgradeCTA />);

    await act(async () => {
      fireEvent.click(screen.getByText('pro.startTrial'));
    });

    expect(startTrial).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('says so when the start fails and a trial is still on offer', async () => {
    trialOffer = 'available';
    startTrial.mockResolvedValue(false);
    render(<SignalUpgradeCTA compact />);

    await act(async () => {
      fireEvent.click(screen.getByText('pro.startTrial'));
    });

    expect(screen.getByRole('alert')).toHaveTextContent('pro.trialStartFailed');
    // Still retryable.
    expect(screen.getByText('pro.startTrial')).not.toBeDisabled();
  });

  it('says the trial already ran when that is why the start was refused', async () => {
    trialOffer = 'available';
    // The store re-reads the status on a refusal; the hook then reports `used`.
    startTrial.mockImplementation(() => {
      trialOffer = 'used';
      return Promise.resolve(false);
    });
    render(<SignalUpgradeCTA compact />);

    await act(async () => {
      fireEvent.click(screen.getByText('pro.startTrial'));
    });

    expect(screen.queryByText('pro.startTrial')).not.toBeInTheDocument();
    expect(screen.getByTestId('signal-cta-trial-used')).toHaveTextContent('pro.trialEnded');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
