// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ChannelCard } from '../ChannelCard';
import type { ChannelSummary } from '../../../types/channels';

function makeChannel(overrides: Partial<ChannelSummary> = {}): ChannelSummary {
  return {
    id: 1,
    slug: 'test-channel',
    title: 'Test Channel',
    description: 'A test channel for unit tests',
    source_count: 3,
    render_count: 5,
    freshness: 'fresh',
    last_rendered_at: null,
    ...overrides,
  };
}

describe('ChannelCard', () => {
  it('renders the channel title', () => {
    render(<ChannelCard channel={makeChannel({ title: 'My Channel' })} active={false} onClick={vi.fn()} />);
    expect(screen.getByText('My Channel')).toBeInTheDocument();
  });

  it('renders the channel description', () => {
    render(<ChannelCard channel={makeChannel({ description: 'Rust ecosystem updates' })} active={false} onClick={vi.fn()} />);
    expect(screen.getByText('Rust ecosystem updates')).toBeInTheDocument();
  });

  it('shows source count', () => {
    render(<ChannelCard channel={makeChannel({ source_count: 7 })} active={false} onClick={vi.fn()} />);
    // The source count and label are in the same span: "7 channels.sources"
    expect(screen.getByText(/7\s+channels\.sources/)).toBeInTheDocument();
  });

  it('shows fresh freshness indicator', () => {
    render(<ChannelCard channel={makeChannel({ freshness: 'fresh' })} active={false} onClick={vi.fn()} />);
    expect(screen.getByText('channels.freshness.fresh')).toBeInTheDocument();
  });

  it('shows stale freshness indicator', () => {
    render(<ChannelCard channel={makeChannel({ freshness: 'stale' })} active={false} onClick={vi.fn()} />);
    expect(screen.getByText('channels.freshness.stale')).toBeInTheDocument();
  });

  it('shows never_rendered freshness indicator', () => {
    render(<ChannelCard channel={makeChannel({ freshness: 'never_rendered' })} active={false} onClick={vi.fn()} />);
    expect(screen.getByText('channels.freshness.never')).toBeInTheDocument();
  });

  it('calls onClick when card is clicked', () => {
    const onClick = vi.fn();
    render(<ChannelCard channel={makeChannel()} active={false} onClick={onClick} />);
    fireEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('sets aria-current when active', () => {
    render(<ChannelCard channel={makeChannel()} active={true} onClick={vi.fn()} />);
    expect(screen.getByRole('button')).toHaveAttribute('aria-current', 'true');
  });

  it('does not set aria-current when inactive', () => {
    render(<ChannelCard channel={makeChannel()} active={false} onClick={vi.fn()} />);
    expect(screen.getByRole('button')).not.toHaveAttribute('aria-current');
  });
});
