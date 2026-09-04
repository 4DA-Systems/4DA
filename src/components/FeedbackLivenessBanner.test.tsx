// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => (typeof fallback === 'string' ? fallback : key),
  }),
}));

// Run the deferred fetch synchronously — the idle scheduling is not under test.
vi.mock('../lib/defer', () => ({
  runWhenIdle: (fn: () => void) => {
    fn();
    return () => {};
  },
}));

const cmdMock = vi.fn();
vi.mock('../lib/commands', () => ({ cmd: (...args: unknown[]) => cmdMock(...args) }));

import {
  FeedbackLivenessBanner,
  LIVENESS_MIN_SURFACED,
  shouldShowFeedbackLiveness,
} from './FeedbackLivenessBanner';
import type { FeedbackLiveness } from '../lib/commands';

const LINE = 'No ratings or clicks in 14 days. Scores are not calibrated to you yet.';

function liveness(partial: Partial<FeedbackLiveness> = {}): FeedbackLiveness {
  return {
    surfaced_14d: 763,
    feedback_14d: 0,
    interactions_14d: 0,
    last_feedback_at: '2026-08-24 16:58:45',
    last_interaction_at: '2026-08-24 18:50:33',
    ...partial,
  };
}

describe('shouldShowFeedbackLiveness', () => {
  it('fires on the live 2026-09-04 shape: hundreds surfaced, nothing back', () => {
    expect(shouldShowFeedbackLiveness(liveness())).toBe(true);
  });

  it('is exactly the surfaced >= 200 AND feedback == 0 AND interactions == 0 rule', () => {
    expect(shouldShowFeedbackLiveness(liveness({ surfaced_14d: LIVENESS_MIN_SURFACED }))).toBe(true);
    expect(shouldShowFeedbackLiveness(liveness({ surfaced_14d: LIVENESS_MIN_SURFACED - 1 }))).toBe(false);
    expect(shouldShowFeedbackLiveness(liveness({ feedback_14d: 1 }))).toBe(false);
    expect(shouldShowFeedbackLiveness(liveness({ interactions_14d: 1 }))).toBe(false);
  });

  it('stays silent on a quiet corpus — a fresh install must never see this line', () => {
    expect(shouldShowFeedbackLiveness(liveness({ surfaced_14d: 0 }))).toBe(false);
  });

  it('makes no claim without data', () => {
    expect(shouldShowFeedbackLiveness(null)).toBe(false);
    expect(shouldShowFeedbackLiveness(undefined)).toBe(false);
  });
});

describe('FeedbackLivenessBanner', () => {
  beforeEach(() => {
    cmdMock.mockReset();
  });

  it('renders the one line when the loop is dead', async () => {
    cmdMock.mockResolvedValue(liveness());
    render(<FeedbackLivenessBanner />);
    const banner = await screen.findByTestId('feedback-liveness-banner');
    expect(banner).toHaveTextContent(LINE);
    expect(cmdMock).toHaveBeenCalledWith('get_feedback_liveness');
  });

  it('renders nothing when a single rating came back in the window', async () => {
    cmdMock.mockResolvedValue(liveness({ feedback_14d: 1 }));
    render(<FeedbackLivenessBanner />);
    await waitFor(() => expect(cmdMock).toHaveBeenCalled());
    expect(screen.queryByTestId('feedback-liveness-banner')).toBeNull();
  });

  it('renders nothing when the command fails — no data, no claim', async () => {
    cmdMock.mockRejectedValue(new Error('backend unavailable'));
    render(<FeedbackLivenessBanner />);
    await waitFor(() => expect(cmdMock).toHaveBeenCalled());
    expect(screen.queryByTestId('feedback-liveness-banner')).toBeNull();
  });
});
