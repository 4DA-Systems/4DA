import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ScoreAutopsy } from './ScoreAutopsy';

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const { invoke } = await import('@tauri-apps/api/core');

const mockAutopsyResult = {
  item: {
    id: 123,
    title: 'Test Item',
    url: 'https://example.com',
    source_type: 'hackernews',
    created_at: '2026-01-01 00:00:00',
    age_hours: 2,
  },
  final_score: 0.85,
  components: [
    {
      name: 'Interest Match',
      raw_value: 0.8,
      weight: 0.3,
      contribution: 0.4,
      explanation: 'Matches interests: TypeScript, React',
    },
    {
      name: 'Tech Stack Match',
      raw_value: 0.7,
      weight: 0.2,
      contribution: 0.2,
      explanation: 'Matches tech: Rust, SQLite',
    },
  ],
  matching_context: {
    interests: ['TypeScript', 'React'],
    tech_stack: ['Rust', 'SQLite'],
    active_topics: ['tauri-app'],
    learned_affinities: ['performance'],
    exclusions_hit: [],
  },
  similar_items: [
    {
      id: 456,
      title: 'Similar Item',
      score: 0.75,
      score_difference: -0.1,
      key_difference: 'Lower: fewer context matches',
    },
  ],
  recommendations: ['Continue providing feedback to refine'],
  narrative: 'This item is highly relevant to you (85% match).',
  ai_analysis: {
    verdict: 'Score makes sense based on matching interests',
    score_assessment: 'accurate' as const,
    reasoning: 'Strong alignment with declared interests',
    suggested_action: 'No specific action needed',
    model_used: 'claude-3-5-sonnet-20241022',
  },
};

describe('ScoreAutopsy', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders trigger button initially', () => {
    render(
      <ScoreAutopsy itemId={123} sourceType="hackernews" currentScore={0.85} />,
    );

    const button = screen.getByRole('button', { name: /score autopsy/i });
    expect(button).toBeInTheDocument();
  });

  it('calls MCP command when button clicked', async () => {
    vi.mocked(invoke).mockResolvedValue(mockAutopsyResult);

    render(
      <ScoreAutopsy itemId={123} sourceType="hackernews" currentScore={0.85} />,
    );

    const button = screen.getByRole('button', { name: /score autopsy/i });
    fireEvent.click(button);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('mcp_score_autopsy', {
        itemId: 123,
        sourceType: 'hackernews',
        synthesize: true,
        compact: false,
      });
    });
  });

  it('displays autopsy results after loading', async () => {
    vi.mocked(invoke).mockResolvedValue(mockAutopsyResult);

    render(
      <ScoreAutopsy itemId={123} sourceType="hackernews" currentScore={0.85} />,
    );

    const button = screen.getByRole('button', { name: /score autopsy/i });
    fireEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText(/ai assessment/i)).toBeInTheDocument();
      expect(screen.getByText(/score makes sense/i)).toBeInTheDocument();
      expect(screen.getByText(/interest match/i)).toBeInTheDocument();
      expect(screen.getByText(/tech stack match/i)).toBeInTheDocument();
    });
  });

  it('displays component breakdown with visual bars', async () => {
    vi.mocked(invoke).mockResolvedValue(mockAutopsyResult);

    render(
      <ScoreAutopsy itemId={123} sourceType="hackernews" currentScore={0.85} />,
    );

    fireEvent.click(screen.getByRole('button', { name: /score autopsy/i }));

    await waitFor(() => {
      const bars = document.querySelectorAll('.component-bar');
      expect(bars.length).toBeGreaterThan(0);
      expect(screen.getByText('40.0%')).toBeInTheDocument(); // First component contribution
    });
  });

  it('handles error states gracefully', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('MCP call failed'));

    render(
      <ScoreAutopsy itemId={123} sourceType="hackernews" currentScore={0.85} />,
    );

    fireEvent.click(screen.getByRole('button', { name: /score autopsy/i }));

    await waitFor(() => {
      expect(screen.getByText('MCP call failed')).toBeInTheDocument();
    });
  });
});
