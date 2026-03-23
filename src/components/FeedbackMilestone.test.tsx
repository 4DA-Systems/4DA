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
    expect(screen.getByText('Your 4DA learned from 10 signals this session')).toBeInTheDocument();
  });

  it('shows milestone message at count 50', () => {
    render(<FeedbackMilestone count={50} />);
    expect(screen.getByText('50 signals — your model is now personalized')).toBeInTheDocument();
  });

  it('shows milestone message at count 100', () => {
    render(<FeedbackMilestone count={100} />);
    expect(screen.getByText('100 signals — top 5% of active users')).toBeInTheDocument();
  });

  it('shows milestone message at count 500', () => {
    render(<FeedbackMilestone count={500} />);
    expect(screen.getByText('500 signals — deeply calibrated')).toBeInTheDocument();
  });

  it('has role="status" for screen reader announcement', () => {
    render(<FeedbackMilestone count={10} />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('auto-hides after 5 seconds', () => {
    render(<FeedbackMilestone count={10} />);
    expect(screen.getByText('Your 4DA learned from 10 signals this session')).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(screen.queryByText('Your 4DA learned from 10 signals this session')).not.toBeInTheDocument();
  });

  it('shows secondary text alongside milestone', () => {
    render(<FeedbackMilestone count={10} />);
    expect(screen.getByText('Your feed keeps getting sharper')).toBeInTheDocument();
  });
});
