import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { GhostPreview, type GhostPreviewData } from './GhostPreview';

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

function makePreview(overrides: Partial<GhostPreviewData> = {}): GhostPreviewData {
  return {
    total_results: 0,
    hidden_results: 0,
    decision_count: 0,
    gap_count: 0,
    synthesis_available: false,
    ...overrides,
  };
}

describe('GhostPreview', () => {
  it('renders nothing when all counts are 0 and synthesis not available', () => {
    const { container } = render(<GhostPreview preview={makePreview()} />);
    expect(container.innerHTML).toBe('');
  });

  it('shows synthesis line when synthesis_available is true', () => {
    render(<GhostPreview preview={makePreview({ synthesis_available: true })} />);
    expect(screen.getByText('search.ghostSynthesis')).toBeInTheDocument();
  });

  it('shows hidden results count', () => {
    render(<GhostPreview preview={makePreview({ hidden_results: 12 })} />);
    expect(screen.getByText('search.ghostMoreResults')).toBeInTheDocument();
  });

  it('shows decision count', () => {
    render(<GhostPreview preview={makePreview({ decision_count: 3 })} />);
    expect(screen.getByText('search.ghostDecisions')).toBeInTheDocument();
  });

  it('shows gap count with "blind spots" text', () => {
    render(<GhostPreview preview={makePreview({ gap_count: 2 })} />);
    // The translation key resolves to "search.ghostGaps" which maps to "{{count}} blind spots in this area"
    // With our mock, it returns the key with interpolated count
    expect(screen.getByText('search.ghostGaps')).toBeInTheDocument();
  });

  it('has "Intelligence depth" header text', () => {
    render(<GhostPreview preview={makePreview({ synthesis_available: true })} />);
    expect(screen.getByText('search.proIntelligence')).toBeInTheDocument();
  });

  it('shows multiple lines when multiple fields are set', () => {
    render(
      <GhostPreview
        preview={makePreview({
          synthesis_available: true,
          hidden_results: 5,
          decision_count: 2,
          gap_count: 1,
        })}
      />,
    );
    expect(screen.getByText('search.ghostSynthesis')).toBeInTheDocument();
    expect(screen.getByText('search.ghostMoreResults')).toBeInTheDocument();
    expect(screen.getByText('search.ghostDecisions')).toBeInTheDocument();
    expect(screen.getByText('search.ghostGaps')).toBeInTheDocument();
  });
});
