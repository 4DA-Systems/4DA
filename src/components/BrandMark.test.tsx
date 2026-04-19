// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Tauri API mocks
// ---------------------------------------------------------------------------
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { BrandMark } from './void-engine/BrandMark';
import type { VoidSignal } from '../types';

function makeSignal(overrides: Partial<VoidSignal> = {}): VoidSignal {
  return {
    pulse: 0,
    heat: 0,
    burst: 0,
    morph: 0,
    error: 0,
    staleness: 0,
    item_count: 0,
    signal_intensity: 0,
    signal_urgency: 0,
    critical_count: 0,
    signal_color_shift: 0,
    metabolism: 0,
    open_windows: 0,
    advantage_trend: 0,
    ...overrides,
  };
}

describe('BrandMark', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders without crash with default signal', () => {
    render(<BrandMark signal={makeSignal()} />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('renders without signal prop (no-signal fallback)', () => {
    render(<BrandMark />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('has accessible aria-label with state', () => {
    render(<BrandMark signal={makeSignal({ item_count: 0, heat: 0 })} />);
    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label');
    expect(status.getAttribute('aria-label')).toContain('4DA status');
  });

  it('renders SVG with tetrahedron geometry (4 vertices)', () => {
    const { container } = render(<BrandMark signal={makeSignal()} />);
    const circles = container.querySelectorAll('circle');
    expect(circles.length).toBe(4); // 3 outer + 1 center
  });

  it('shows "Dormant" label when no items and high staleness at large size', () => {
    render(
      <BrandMark signal={makeSignal({ item_count: 0, heat: 0, staleness: 0.95 })} size={200} />,
    );
    expect(screen.getByText('Dormant')).toBeInTheDocument();
  });

  it('shows "Awakening" label when no items and low staleness at large size', () => {
    render(
      <BrandMark signal={makeSignal({ item_count: 0, heat: 0, staleness: 0.2 })} size={200} />,
    );
    expect(screen.getByText('Awakening')).toBeInTheDocument();
  });

  it('shows "Error" label when error signal is high', () => {
    render(
      <BrandMark signal={makeSignal({ error: 0.8, item_count: 5 })} size={200} />,
    );
    expect(screen.getByText('Error')).toBeInTheDocument();
  });

  it('shows "Stale" label when staleness is high', () => {
    render(
      <BrandMark signal={makeSignal({ staleness: 0.9, item_count: 10 })} size={200} />,
    );
    expect(screen.getByText('Stale')).toBeInTheDocument();
  });

  it('shows "Scanning" label when pulse is active', () => {
    render(
      <BrandMark signal={makeSignal({ pulse: 0.7, item_count: 5 })} size={200} />,
    );
    expect(screen.getByText('Scanning')).toBeInTheDocument();
  });

  it('shows "Discoveries" label when heat is high', () => {
    render(
      <BrandMark signal={makeSignal({ heat: 0.7, item_count: 10 })} size={200} />,
    );
    expect(screen.getByText('Discoveries')).toBeInTheDocument();
  });

  it('shows "Active" label when items exist', () => {
    render(
      <BrandMark signal={makeSignal({ item_count: 10, heat: 0.2 })} size={200} />,
    );
    expect(screen.getByText('Active')).toBeInTheDocument();
  });

  it('shows "Alert" label for critical signals', () => {
    render(
      <BrandMark
        signal={makeSignal({
          critical_count: 1,
          signal_intensity: 0.8,
          item_count: 5,
        })}
        size={200}
      />,
    );
    expect(screen.getByText('Alert')).toBeInTheDocument();
  });

  it('shows multi-alert label with count for multiple critical signals', () => {
    render(
      <BrandMark
        signal={makeSignal({
          critical_count: 3,
          signal_intensity: 0.9,
          item_count: 5,
        })}
        size={200}
      />,
    );
    expect(screen.getByText('3 Alerts')).toBeInTheDocument();
  });

  it('shows "Breaking" label for high positive color shift', () => {
    render(
      <BrandMark
        signal={makeSignal({ signal_color_shift: 0.6, item_count: 5 })}
        size={200}
      />,
    );
    expect(screen.getByText('Breaking')).toBeInTheDocument();
  });

  it('shows "Learning" label for negative color shift', () => {
    render(
      <BrandMark
        signal={makeSignal({ signal_color_shift: -0.5, item_count: 5 })}
        size={200}
      />,
    );
    expect(screen.getByText('Learning')).toBeInTheDocument();
  });

  it('hides label when size is small (< 100)', () => {
    render(
      <BrandMark signal={makeSignal({ item_count: 10, heat: 0.7 })} size={48} />,
    );
    expect(screen.queryByText('Discoveries')).not.toBeInTheDocument();
  });

  it('includes item count in aria-label when items exist', () => {
    render(
      <BrandMark signal={makeSignal({ item_count: 42, heat: 0.3 })} size={200} />,
    );
    const status = screen.getByRole('status');
    expect(status.getAttribute('aria-label')).toContain('42 items found');
  });

  it('includes decision window count in title', () => {
    render(
      <BrandMark signal={makeSignal({ item_count: 5, open_windows: 2 })} />,
    );
    const status = screen.getByRole('status');
    expect(status.getAttribute('title')).toContain('2 decision windows');
  });
});
