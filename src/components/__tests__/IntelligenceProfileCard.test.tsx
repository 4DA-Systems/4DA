import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

// Store mock
let currentStore: Record<string, unknown> = {};

vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) =>
    selector(currentStore),
  ),
}));

vi.mock('../../hooks/use-license', () => ({
  useLicense: () => ({ isPro: false }),
}));

import { IntelligenceProfileCard } from '../IntelligenceProfileCard';

function setStore(overrides: Record<string, unknown>) {
  currentStore = {
    learnedAffinities: [],
    intelligencePulse: null,
    feedbackGiven: {},
    ...overrides,
  };
}

describe('IntelligenceProfileCard', () => {
  it('returns null when no affinities and no pulse', () => {
    setStore({});
    const { container } = render(<IntelligenceProfileCard />);
    expect(container.firstChild).toBeNull();
  });

  it('returns null when pulse has zero cycles', () => {
    setStore({ intelligencePulse: { total_cycles: 0, calibration_accuracy: 0, items_analyzed_7d: 0, items_surfaced_7d: 0 } });
    const { container } = render(<IntelligenceProfileCard />);
    expect(container.firstChild).toBeNull();
  });

  it('renders autophagy accuracy card when pulse is present', () => {
    setStore({
      intelligencePulse: {
        total_cycles: 5,
        calibration_accuracy: 0.73,
        items_analyzed_7d: 150,
        items_surfaced_7d: 12,
      },
    });
    render(<IntelligenceProfileCard />);
    expect(screen.getByText('73%')).toBeInTheDocument();
    expect(screen.getByText('briefing.profile.autophagyAccuracy')).toBeInTheDocument();
  });

  it('renders feedback impact card when feedback exists', () => {
    setStore({
      learnedAffinities: [{ topic: 'Rust', affinity_score: 0.8 }],
      feedbackGiven: { 1: 'save', 2: 'dismiss', 3: 'save' },
    });
    render(<IntelligenceProfileCard />);
    expect(screen.getByText('3')).toBeInTheDocument();
    expect(screen.getByText('briefing.profile.feedbackImpact')).toBeInTheDocument();
  });

  it('renders intelligence profile with top affinities', () => {
    setStore({
      learnedAffinities: [
        { topic: 'Rust', affinity_score: 0.9 },
        { topic: 'TypeScript', affinity_score: 0.7 },
      ],
    });
    render(<IntelligenceProfileCard />);
    expect(screen.getByText('briefing.profile.title')).toBeInTheDocument();
    expect(screen.getByText('Rust')).toBeInTheDocument();
    expect(screen.getByText('TypeScript')).toBeInTheDocument();
  });

  it('shows learning velocity count', () => {
    setStore({
      learnedAffinities: [
        { topic: 'A', affinity_score: 0.5 },
        { topic: 'B', affinity_score: 0.3 },
        { topic: 'C', affinity_score: -0.2 },
      ],
    });
    render(<IntelligenceProfileCard />);
    expect(screen.getByText('3')).toBeInTheDocument();
    expect(screen.getByText('briefing.profile.topicsLearned')).toBeInTheDocument();
  });

  it('shows system activity when pulse is available', () => {
    setStore({
      learnedAffinities: [{ topic: 'X', affinity_score: 0.5 }],
      intelligencePulse: {
        total_cycles: 10,
        calibration_accuracy: 0.65,
        items_analyzed_7d: 500,
        items_surfaced_7d: 42,
      },
    });
    render(<IntelligenceProfileCard />);
    expect(screen.getByText('briefing.profile.systemActivity')).toBeInTheDocument();
    expect(screen.getByText('briefing.profile.cyclesComplete')).toBeInTheDocument();
  });

  it('uses correct color class for high accuracy', () => {
    setStore({
      intelligencePulse: {
        total_cycles: 3,
        calibration_accuracy: 0.85,
        items_analyzed_7d: 100,
        items_surfaced_7d: 10,
      },
    });
    render(<IntelligenceProfileCard />);
    expect(screen.getByText('85%')).toHaveClass('text-green-400');
  });

  it('uses amber color for medium accuracy', () => {
    setStore({
      intelligencePulse: {
        total_cycles: 3,
        calibration_accuracy: 0.5,
        items_analyzed_7d: 100,
        items_surfaced_7d: 10,
      },
    });
    render(<IntelligenceProfileCard />);
    expect(screen.getByText('50%')).toHaveClass('text-amber-400');
  });
});
