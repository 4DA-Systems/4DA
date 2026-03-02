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
import { VoidHeartbeat } from './void-engine/VoidHeartbeat';
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

describe('VoidHeartbeat', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // jsdom does not support WebGL2 — the component should fall back to CSS
  });

  it('renders without crash with default signal', () => {
    render(<VoidHeartbeat signal={makeSignal()} />);
    // The container has role="status"
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('has accessible aria-label with state', () => {
    render(<VoidHeartbeat signal={makeSignal({ item_count: 0, heat: 0 })} />);
    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label');
    expect(status.getAttribute('aria-label')).toContain('4DA status');
  });

  it('shows "Dormant" label when no items and high staleness', () => {
    render(
      <VoidHeartbeat signal={makeSignal({ item_count: 0, heat: 0, staleness: 0.95 })} />,
    );
    expect(screen.getByText('Dormant')).toBeInTheDocument();
  });

  it('shows "Awakening" label when no items and low staleness', () => {
    render(
      <VoidHeartbeat signal={makeSignal({ item_count: 0, heat: 0, staleness: 0.2 })} />,
    );
    expect(screen.getByText('Awakening')).toBeInTheDocument();
  });

  it('shows "Error" label when error signal is high', () => {
    render(
      <VoidHeartbeat signal={makeSignal({ error: 0.8, item_count: 5 })} />,
    );
    expect(screen.getByText('Error')).toBeInTheDocument();
  });

  it('shows "Stale" label when staleness is high', () => {
    render(
      <VoidHeartbeat signal={makeSignal({ staleness: 0.9, item_count: 10 })} />,
    );
    expect(screen.getByText('Stale')).toBeInTheDocument();
  });

  it('shows "Scanning" label when pulse is active', () => {
    render(
      <VoidHeartbeat signal={makeSignal({ pulse: 0.7, item_count: 5 })} />,
    );
    expect(screen.getByText('Scanning')).toBeInTheDocument();
  });

  it('shows "Discoveries" label when heat is high', () => {
    render(
      <VoidHeartbeat signal={makeSignal({ heat: 0.7, item_count: 10 })} />,
    );
    expect(screen.getByText('Discoveries')).toBeInTheDocument();
  });

  it('shows "Active" label when items exist', () => {
    render(
      <VoidHeartbeat signal={makeSignal({ item_count: 10, heat: 0.2 })} />,
    );
    expect(screen.getByText('Active')).toBeInTheDocument();
  });

  it('shows "Alert" label for critical signals', () => {
    render(
      <VoidHeartbeat
        signal={makeSignal({
          critical_count: 1,
          signal_intensity: 0.8,
          item_count: 5,
        })}
      />,
    );
    expect(screen.getByText('Alert')).toBeInTheDocument();
  });

  it('shows multi-alert label with count for multiple critical signals', () => {
    render(
      <VoidHeartbeat
        signal={makeSignal({
          critical_count: 3,
          signal_intensity: 0.9,
          item_count: 5,
        })}
      />,
    );
    expect(screen.getByText('3 Alerts')).toBeInTheDocument();
  });

  it('shows "Breaking" label for high positive color shift', () => {
    render(
      <VoidHeartbeat
        signal={makeSignal({ signal_color_shift: 0.6, item_count: 5 })}
      />,
    );
    expect(screen.getByText('Breaking')).toBeInTheDocument();
  });

  it('shows "Learning" label for negative color shift', () => {
    render(
      <VoidHeartbeat
        signal={makeSignal({ signal_color_shift: -0.5, item_count: 5 })}
      />,
    );
    expect(screen.getByText('Learning')).toBeInTheDocument();
  });

  it('hides label when size is small (< 100)', () => {
    render(
      <VoidHeartbeat signal={makeSignal({ item_count: 10, heat: 0.7 })} size={48} />,
    );
    // Label should not be present at small sizes
    expect(screen.queryByText('Discoveries')).not.toBeInTheDocument();
  });

  it('includes item count in aria-label when items exist', () => {
    render(
      <VoidHeartbeat signal={makeSignal({ item_count: 42, heat: 0.3 })} />,
    );
    const status = screen.getByRole('status');
    expect(status.getAttribute('aria-label')).toContain('42 items found');
  });

  it('renders the GAME component container div', () => {
    const { container } = render(
      <VoidHeartbeat signal={makeSignal()} />,
    );
    // The container wraps the GAME custom element
    const gameContainer = container.querySelector('.void-heartbeat-container > div');
    expect(gameContainer).toBeInTheDocument();
  });

  it('includes decision window count in title', () => {
    render(
      <VoidHeartbeat signal={makeSignal({ item_count: 5, open_windows: 2 })} />,
    );
    const status = screen.getByRole('status');
    expect(status.getAttribute('title')).toContain('2 decision windows');
  });
});
