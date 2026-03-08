import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SynthesisPanel } from './SynthesisPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (opts) {
        let result = key;
        for (const [k, v] of Object.entries(opts)) {
          result = result.replace(`{{${k}}}`, String(v));
        }
        return result;
      }
      return key;
    },
  }),
}));

describe('SynthesisPanel', () => {
  const defaultProps = {
    query: 'test query',
    isPro: true,
    synthesis: null as string | null,
    loading: false,
    onRetry: vi.fn(),
  };

  it('returns null when isPro is false', () => {
    const { container } = render(
      <SynthesisPanel {...defaultProps} isPro={false} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('returns null when not loading and no synthesis', () => {
    const { container } = render(
      <SynthesisPanel {...defaultProps} synthesis={null} loading={false} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('shows loading state with pulse animation', () => {
    const { container } = render(
      <SynthesisPanel {...defaultProps} loading={true} />,
    );
    const pulseEl = container.querySelector('.animate-pulse');
    expect(pulseEl).toBeInTheDocument();
    expect(screen.getByText('search.analyzingSignals')).toBeInTheDocument();
  });

  it('shows synthesis text when available', () => {
    render(
      <SynthesisPanel
        {...defaultProps}
        synthesis="Rust 1.80 introduces new async features that align with your stack."
        loading={false}
      />,
    );
    expect(
      screen.getByText('Rust 1.80 introduces new async features that align with your stack.'),
    ).toBeInTheDocument();
  });

  it('shows retry button when synthesis is displayed', () => {
    const retryFn = vi.fn();
    render(
      <SynthesisPanel
        {...defaultProps}
        synthesis="Some synthesis text"
        loading={false}
        onRetry={retryFn}
      />,
    );
    const retryButton = screen.getByText('action.retry');
    expect(retryButton).toBeInTheDocument();
    fireEvent.click(retryButton);
    expect(retryFn).toHaveBeenCalledTimes(1);
  });

  it('does not show retry button when loading', () => {
    render(
      <SynthesisPanel
        {...defaultProps}
        synthesis="Some synthesis text"
        loading={true}
      />,
    );
    expect(screen.queryByText('action.retry')).not.toBeInTheDocument();
  });

  it('shows AI Synthesis header', () => {
    render(
      <SynthesisPanel {...defaultProps} synthesis="text" loading={false} />,
    );
    expect(screen.getByText('search.aiSynthesis')).toBeInTheDocument();
  });
});
