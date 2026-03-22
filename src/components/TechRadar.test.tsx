import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';

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
// Child component mocks — RadarSVG, RadarEntryPanel, TemporalSlider
// ---------------------------------------------------------------------------
vi.mock('./tech-radar/RadarSVG', () => ({
  RadarSVG: ({ entries }: { entries: unknown[] }) => (
    <div data-testid="radar-svg">
      {entries.length} entries
    </div>
  ),
}));

vi.mock('./tech-radar/StackIntelligence', () => ({
  StackIntelligence: ({ entries }: { entries: unknown[] }) => (
    <div data-testid="radar-svg">
      {entries.length} entries
    </div>
  ),
}));

vi.mock('./tech-radar/RadarEntryPanel', () => ({
  RadarEntryPanel: () => <div data-testid="radar-entry-panel" />,
}));

vi.mock('./tech-radar/TemporalSlider', () => ({
  TemporalSlider: () => <div data-testid="temporal-slider" />,
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { TechRadar } from './TechRadar';

const mockInvoke = vi.mocked(invoke);

function makeRadarEntry(overrides = {}) {
  return {
    name: 'React',
    ring: 'adopt',
    quadrant: 'frameworks',
    movement: 'stable',
    signals: ['trending'],
    decision_ref: null,
    score: 0.8,
    ...overrides,
  };
}

describe('TechRadar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading state initially', () => {
    // Make invoke hang so loading state persists
    mockInvoke.mockImplementation(() => new Promise(() => {}));
    render(<TechRadar />);
    expect(screen.getByText('techRadar.loading')).toBeInTheDocument();
  });

  it('shows empty state when no entries returned', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_tech_radar') return Promise.resolve({ generated_at: '', entries: [] });
      if (cmd === 'get_user_context') return Promise.resolve({ tech_stack: [] });
      return Promise.resolve({});
    });

    render(<TechRadar />);

    await waitFor(() => {
      expect(screen.getByText('techRadar.empty')).toBeInTheDocument();
    });
  });

  it('renders radar with entries after loading', async () => {
    const entries = [
      makeRadarEntry({ name: 'React' }),
      makeRadarEntry({ name: 'Rust', ring: 'trial', quadrant: 'languages' }),
    ];

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_tech_radar')
        return Promise.resolve({ generated_at: '2025-01-15T00:00:00Z', entries });
      if (cmd === 'get_user_context')
        return Promise.resolve({ tech_stack: ['react'] });
      return Promise.resolve({});
    });

    render(<TechRadar />);

    await waitFor(() => {
      expect(screen.getByText('techRadar.title')).toBeInTheDocument();
    });

    // RadarSVG mock shows entry count
    expect(screen.getByTestId('radar-svg')).toHaveTextContent('2 entries');
    // Temporal slider should be present
    expect(screen.getByTestId('temporal-slider')).toBeInTheDocument();
    // Entry panel should be present
    expect(screen.getByTestId('radar-entry-panel')).toBeInTheDocument();
  });

  it('shows entry count in subtitle', async () => {
    const entries = [makeRadarEntry(), makeRadarEntry({ name: 'Vue' }), makeRadarEntry({ name: 'Svelte' })];

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_tech_radar')
        return Promise.resolve({ generated_at: '2025-01-15T00:00:00Z', entries });
      if (cmd === 'get_user_context')
        return Promise.resolve({ tech_stack: [] });
      return Promise.resolve({});
    });

    render(<TechRadar />);

    await waitFor(() => {
      expect(screen.getByText('techRadar.count')).toBeInTheDocument();
    });
  });

  it('renders legend items after loading', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_tech_radar')
        return Promise.resolve({ generated_at: '2025-01-15T00:00:00Z', entries: [makeRadarEntry()] });
      if (cmd === 'get_user_context')
        return Promise.resolve({ tech_stack: [] });
      return Promise.resolve({});
    });

    render(<TechRadar />);

    await waitFor(() => {
      expect(screen.getByText('techRadar.movingIn')).toBeInTheDocument();
      expect(screen.getByText('techRadar.movingOut')).toBeInTheDocument();
      expect(screen.getByText('techRadar.new')).toBeInTheDocument();
      expect(screen.getByText('techRadar.stable')).toBeInTheDocument();
      expect(screen.getByText('techRadar.yourStack')).toBeInTheDocument();
    });
  });
});
