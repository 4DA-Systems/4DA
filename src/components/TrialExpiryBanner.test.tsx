// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { TrialExpiryBanner } from './TrialExpiryBanner';

// The 14-day reverse trial used to expire SILENTLY — the user's first hint
// was features going dark. This banner announces the cliff honestly in the
// final 4 days. These tests pin: eligibility window, per-day dismissal
// persistence (reappears as the count drops), and the no-license guard.

interface MockTrialStatus {
  active: boolean;
  days_remaining: number;
  started_at: string | null;
  has_license: boolean;
}

let mockTrialStatus: MockTrialStatus | null = null;
vi.mock('../hooks/use-license', () => ({
  useLicense: () => ({
    tier: 'free',
    isPro: mockTrialStatus?.active === true,
    trialStatus: mockTrialStatus,
    expired: false,
    daysRemaining: 0,
    expiresAt: null,
  }),
}));

vi.mock('./SignalUpgradeCTA', () => ({
  SignalUpgradeCTA: () => <div data-testid="signal-upgrade-cta" />,
}));

function setTrial(days: number, overrides: Partial<MockTrialStatus> = {}) {
  mockTrialStatus = {
    active: true,
    days_remaining: days,
    started_at: '2026-06-01T00:00:00Z',
    has_license: false,
    ...overrides,
  };
}

beforeEach(() => {
  mockTrialStatus = null;
  localStorage.clear();
});

describe('TrialExpiryBanner — eligibility', () => {
  it('renders nothing while trial status has not loaded', () => {
    const { container } = render(<TrialExpiryBanner />);
    expect(container.innerHTML).toBe('');
  });

  it('renders nothing with more than 4 days remaining', () => {
    setTrial(5);
    const { container } = render(<TrialExpiryBanner />);
    expect(container.innerHTML).toBe('');
  });

  it('shows the banner with the upgrade CTA at 4 days remaining', () => {
    setTrial(4);
    render(<TrialExpiryBanner />);
    expect(screen.getByText('trialExpiry.title')).toBeInTheDocument();
    expect(screen.getByText('trialExpiry.body')).toBeInTheDocument();
    expect(screen.getByTestId('signal-upgrade-cta')).toBeInTheDocument();
  });

  it('shows the banner at 1 day remaining', () => {
    setTrial(1);
    render(<TrialExpiryBanner />);
    expect(screen.getByText('trialExpiry.title')).toBeInTheDocument();
  });

  it('renders nothing once the trial has ended (0 days / inactive)', () => {
    setTrial(0, { active: false });
    const { container } = render(<TrialExpiryBanner />);
    expect(container.innerHTML).toBe('');
  });

  it('renders nothing when the user holds a license', () => {
    setTrial(2, { has_license: true });
    const { container } = render(<TrialExpiryBanner />);
    expect(container.innerHTML).toBe('');
  });

  it('renders nothing during the comfortable middle of the trial', () => {
    setTrial(10);
    const { container } = render(<TrialExpiryBanner />);
    expect(container.innerHTML).toBe('');
  });
});

describe('TrialExpiryBanner — per-day dismissal', () => {
  it('dismiss hides the banner and persists keyed by days remaining', () => {
    setTrial(4);
    const { container } = render(<TrialExpiryBanner />);
    fireEvent.click(screen.getByText('trialExpiry.dismiss'));
    expect(container.innerHTML).toBe('');
    expect(localStorage.getItem('4da-trial-expiry-dismissed-d4')).not.toBeNull();
  });

  it('stays hidden on remount for the same day count', () => {
    localStorage.setItem('4da-trial-expiry-dismissed-d3', '1');
    setTrial(3);
    const { container } = render(<TrialExpiryBanner />);
    expect(container.innerHTML).toBe('');
  });

  it('reappears when the remaining-day count drops below a dismissed day', () => {
    localStorage.setItem('4da-trial-expiry-dismissed-d4', '1');
    setTrial(3);
    render(<TrialExpiryBanner />);
    expect(screen.getByText('trialExpiry.title')).toBeInTheDocument();
  });

  it('each day is shown at most once: dismissing day 2 leaves day 1 pending', () => {
    setTrial(2);
    render(<TrialExpiryBanner />);
    fireEvent.click(screen.getByText('trialExpiry.dismiss'));
    expect(localStorage.getItem('4da-trial-expiry-dismissed-d2')).not.toBeNull();
    expect(localStorage.getItem('4da-trial-expiry-dismissed-d1')).toBeNull();
  });
});
