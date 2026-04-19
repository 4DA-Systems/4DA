// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { FeedbackMilestone } from './FeedbackMilestone';

describe('FeedbackMilestone', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns null for non-milestone counts', () => {
    const { container } = render(<FeedbackMilestone count={5} />);
    expect(container.innerHTML).toBe('');
  });

  it('shows milestone message at count 10', () => {
    render(<FeedbackMilestone count={10} />);
    expect(screen.getByText('feedback.milestone10')).toBeInTheDocument();
  });

  it('shows milestone message at count 50', () => {
    render(<FeedbackMilestone count={50} />);
    expect(screen.getByText('feedback.milestone50')).toBeInTheDocument();
  });

  it('shows milestone message at count 100', () => {
    render(<FeedbackMilestone count={100} />);
    expect(screen.getByText('feedback.milestone100')).toBeInTheDocument();
  });

  it('shows milestone message at count 500', () => {
    render(<FeedbackMilestone count={500} />);
    expect(screen.getByText('feedback.milestone500')).toBeInTheDocument();
  });

  it('has role="status" for screen reader announcement', () => {
    render(<FeedbackMilestone count={10} />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('auto-hides after 5 seconds', () => {
    render(<FeedbackMilestone count={10} />);
    expect(screen.getByText('feedback.milestone10')).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(screen.queryByText('feedback.milestone10')).not.toBeInTheDocument();
  });

  it('shows secondary text alongside milestone', () => {
    render(<FeedbackMilestone count={10} />);
    expect(screen.getByText('feedback.gettingSharper')).toBeInTheDocument();
  });
});
