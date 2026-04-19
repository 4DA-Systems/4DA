// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SynthesisPanel, type SynthesisResponse } from './SynthesisPanel';

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

function makeSynthesis(text: string, sources: SynthesisResponse['sources'] = []): SynthesisResponse {
  return {
    text,
    sources,
    grounding_count: sources.length,
    total_sources: sources.length,
  };
}

describe('SynthesisPanel', () => {
  const defaultProps = {
    query: 'test query',
    isPro: true,
    synthesis: null as SynthesisResponse | null,
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
        synthesis={makeSynthesis('Rust 1.80 introduces new async features that align with your stack.')}
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
        synthesis={makeSynthesis('Some synthesis text')}
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
        synthesis={makeSynthesis('Some synthesis text')}
        loading={true}
      />,
    );
    expect(screen.queryByText('action.retry')).not.toBeInTheDocument();
  });

  it('shows AI Synthesis header', () => {
    render(
      <SynthesisPanel {...defaultProps} synthesis={makeSynthesis('text')} loading={false} />,
    );
    expect(screen.getByText('search.aiSynthesis')).toBeInTheDocument();
  });

  it('renders citation links for sources with URLs', () => {
    const synthesis = makeSynthesis(
      'Rust is evolving fast [1] with new async features [2].',
      [
        { index: 1, title: 'Rust Blog', url: 'https://blog.rust-lang.org', source_type: 'rss' },
        { index: 2, title: 'HN Discussion', url: 'https://news.ycombinator.com/item?id=123', source_type: 'hn' },
      ],
    );
    const { container } = render(
      <SynthesisPanel {...defaultProps} synthesis={synthesis} loading={false} />,
    );
    // Citation links are the inline superscript links (have bg-cyan-500/20 class)
    const citationLinks = container.querySelectorAll('a.bg-cyan-500\\/20');
    expect(citationLinks).toHaveLength(2);
    expect(citationLinks[0]).toHaveAttribute('href', 'https://blog.rust-lang.org');
    expect(citationLinks[1]).toHaveAttribute('href', 'https://news.ycombinator.com/item?id=123');
  });

  it('renders plain markers for sources without URLs', () => {
    const synthesis = makeSynthesis(
      'Local analysis shows [1] improvements.',
      [{ index: 1, title: 'Local File', url: null, source_type: 'context' }],
    );
    const { container } = render(
      <SynthesisPanel {...defaultProps} synthesis={synthesis} loading={false} />,
    );
    expect(screen.queryByRole('link')).not.toBeInTheDocument();
    expect(container.textContent).toContain('[1]');
  });

  it('shows grounding indicator dots', () => {
    const synthesis: SynthesisResponse = {
      text: 'Summary text [1].',
      sources: [
        { index: 1, title: 'Source A', url: 'https://a.com', source_type: 'rss' },
      ],
      grounding_count: 1,
      total_sources: 3,
    };
    const { container } = render(
      <SynthesisPanel {...defaultProps} synthesis={synthesis} loading={false} />,
    );
    const dots = container.querySelectorAll('.rounded-full.w-1\\.5');
    expect(dots).toHaveLength(3);
  });

  it('shows ungrounded warning when zero citations', () => {
    const synthesis: SynthesisResponse = {
      text: 'A synthesis with no citations at all.',
      sources: [
        { index: 1, title: 'Source A', url: 'https://a.com', source_type: 'rss' },
        { index: 2, title: 'Source B', url: 'https://b.com', source_type: 'hn' },
      ],
      grounding_count: 0,
      total_sources: 2,
    };
    render(
      <SynthesisPanel {...defaultProps} synthesis={synthesis} loading={false} />,
    );
    expect(screen.getByText('search.ungrounded')).toBeInTheDocument();
  });

  it('shows streaming text with cursor when loading with tokens', () => {
    const { container } = render(
      <SynthesisPanel {...defaultProps} loading={true} streamingText="Partial synthesis..." />,
    );
    expect(screen.getByText('Partial synthesis...')).toBeInTheDocument();
    expect(screen.getByText('search.synthesizing')).toBeInTheDocument();
    // Blinking cursor should be present
    const cursor = container.querySelector('.animate-pulse.bg-cyan-400');
    expect(cursor).toBeInTheDocument();
  });

  it('shows expandable sources list', () => {
    const synthesis = makeSynthesis(
      'Text [1].',
      [{ index: 1, title: 'My Source', url: 'https://example.com', source_type: 'rss' }],
    );
    render(
      <SynthesisPanel {...defaultProps} synthesis={synthesis} loading={false} />,
    );
    const details = screen.getByText('search.viewSources');
    expect(details).toBeInTheDocument();
  });
});
