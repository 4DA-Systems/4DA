import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';

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

vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { AccuracyTab } from '../intelligence/AccuracyTab';
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { cmd } = await import('../../lib/commands') as any;

describe('AccuracyTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading skeleton initially', () => {
    // Never resolve — stays in loading state
    vi.mocked(cmd).mockReturnValue(new Promise(() => {}));
    const { container } = render(<AccuracyTab />);
    expect(container.querySelector('.animate-pulse')).toBeInTheDocument();
  });

  it('shows empty state when no data is available', async () => {
    // Reject all commands so allSettled yields {status:'rejected'} for each,
    // leaving state at initial values (null/null/null/[]).
    vi.mocked(cmd).mockRejectedValue(new Error('no data'));
    render(<AccuracyTab />);
    await waitFor(() => {
      expect(screen.getByText('No intelligence data yet')).toBeInTheDocument();
    });
  });

  it('renders accuracy metrics when data is returned', async () => {
    vi.mocked(cmd).mockImplementation((command: string) => {
      switch (command) {
        case 'get_accuracy_report':
          return Promise.resolve({
            id: 1, period: 'month', total_scored: 100,
            total_relevant: 80, user_confirmed: 70, user_rejected: 10,
            accuracy_pct: 70.0, created_at: '2026-03-01',
          });
        case 'get_intelligence_report':
          return Promise.resolve({
            period: 'month', accuracy_current: 70, accuracy_previous: 65,
            accuracy_delta: 5.0, topics_tracked: 12, topics_added: 3,
            noise_rejected: 40, noise_rejection_pct: 85,
            time_saved_hours: 4.2, security_alerts: 1,
            security_acted_on: 1, decisions_recorded: 5, feedback_signals: 8,
          });
        case 'get_temporal_snapshot':
          return Promise.resolve({
            period: 'month', tech_snapshot: [], interest_snapshot: [],
            decision_count: 5, feedback_count: 8,
          });
        case 'get_knowledge_decay_report':
          return Promise.resolve([]);
        default:
          return Promise.resolve(null);
      }
    });

    render(<AccuracyTab />);

    await waitFor(() => {
      expect(screen.getByText('Accuracy')).toBeInTheDocument();
    });
    expect(screen.getByText('Topics Tracked')).toBeInTheDocument();
    expect(screen.getByText('12')).toBeInTheDocument();
    expect(screen.getByText('Time Saved')).toBeInTheDocument();
    expect(screen.getByText('Monthly Intelligence')).toBeInTheDocument();
  });

  it('renders knowledge decay table when entries exist', async () => {
    vi.mocked(cmd).mockImplementation((command: string) => {
      if (command === 'get_knowledge_decay_report') {
        return Promise.resolve([
          { tech_name: 'Python', last_engagement: '2026-01-01',
            weeks_since_engagement: 10, risk_level: 'high',
            recommendation: 'Review recent changes' },
        ]);
      }
      return Promise.resolve(null);
    });

    render(<AccuracyTab />);

    await waitFor(() => {
      expect(screen.getByText('Knowledge Decay')).toBeInTheDocument();
    });
    expect(screen.getByText('Python')).toBeInTheDocument();
    expect(screen.getByText('high')).toBeInTheDocument();
  });
});
