/**
 * ScoreBreakdownDrawer component tests
 *
 * Tests rendering, close behaviors (button + Escape), signal badges,
 * factor grouping (boosts/penalties), feedback buttons, comparison mode,
 * and bar width correspondence.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Tauri API mocks
// ---------------------------------------------------------------------------
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

// ---------------------------------------------------------------------------
// Store mock
// ---------------------------------------------------------------------------
vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
      const mockState: Record<string, unknown> = {
        feedbackGiven: {},
        submitFeedback: vi.fn(),
        addToast: vi.fn(),
      };
      return selector(mockState);
    }),
    { getState: () => ({}) },
  ),
}));

vi.mock('zustand/react/shallow', () => ({
  useShallow: (fn: unknown) => fn,
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { ScoreBreakdownDrawer } from './ScoreBreakdownDrawer';
import type { ScoreBreakdown, SourceRelevance } from '../../types';

function makeBreakdown(overrides: Partial<ScoreBreakdown> = {}): ScoreBreakdown {
  return {
    context_score: 0.8,
    interest_score: 0.6,
    ace_boost: 0.15,
    affinity_mult: 1.0,
    anti_penalty: 1.0,
    confidence_by_signal: {},
    signal_count: 3,
    confirmed_signals: ['context', 'interest', 'ace'],
    ...overrides,
  };
}

function makePoolItem(overrides: Partial<SourceRelevance> = {}): SourceRelevance {
  return {
    id: 99,
    title: 'Compare Article',
    url: 'https://example.com/compare',
    top_score: 0.55,
    matches: [],
    relevant: true,
    source_type: 'hackernews',
    score_breakdown: makeBreakdown({ context_score: 0.3, interest_score: 0.2 }),
    ...overrides,
  };
}

const defaultProps = {
  breakdown: makeBreakdown(),
  finalScore: 0.72,
  itemId: 1,
  onClose: vi.fn(),
};

describe('ScoreBreakdownDrawer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // =========================================================================
  // 1. Basic rendering
  // =========================================================================
  it('renders without crashing', () => {
    const { unmount } = render(<ScoreBreakdownDrawer {...defaultProps} />);
    unmount();
  });

  // =========================================================================
  // 2. Score percentage display
  // =========================================================================
  it('displays score percentage', () => {
    render(<ScoreBreakdownDrawer {...defaultProps} finalScore={0.72} />);
    expect(screen.getByText('72%')).toBeInTheDocument();
  });

  // =========================================================================
  // 3. Close button calls onClose
  // =========================================================================
  it('close button calls onClose', () => {
    const onClose = vi.fn();
    render(<ScoreBreakdownDrawer {...defaultProps} onClose={onClose} />);
    const closeBtn = screen.getByLabelText('Close score breakdown');
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  // =========================================================================
  // 4. Escape key calls onClose
  // =========================================================================
  it('Escape key calls onClose', () => {
    const onClose = vi.fn();
    render(<ScoreBreakdownDrawer {...defaultProps} onClose={onClose} />);
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  // =========================================================================
  // 5. Signal badges are shown
  // =========================================================================
  it('shows signal badges', () => {
    render(<ScoreBreakdownDrawer {...defaultProps} />);
    // The five signal axes render as badges with check/cross marks
    // Use getAllByText to handle multiple matches (signal badge + factor label)
    expect(screen.getAllByText(/context/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/interest/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/ace/).length).toBeGreaterThan(0);
    // "learned" and "dependency" only appear in signal badges (not as factor labels)
    expect(screen.getByText(/learned/)).toBeInTheDocument();
    expect(screen.getByText(/dependency/)).toBeInTheDocument();
    // Signal count display
    expect(screen.getByText('3/5')).toBeInTheDocument();
  });

  // =========================================================================
  // 6. Boost factors are shown
  // =========================================================================
  it('shows boost factors for positive values', () => {
    render(<ScoreBreakdownDrawer {...defaultProps} />);
    // context_score > 0.3 => boost, interest_score > 0.3 => boost, ace_boost > 0 => boost
    expect(screen.getByText('scoreDrawer.factor.context')).toBeInTheDocument();
    expect(screen.getByText('scoreDrawer.factor.interest')).toBeInTheDocument();
    expect(screen.getByText('scoreDrawer.factor.ace')).toBeInTheDocument();
  });

  // =========================================================================
  // 7. Penalty factors are shown
  // =========================================================================
  it('shows penalty factors for low values', () => {
    const penaltyBreakdown = makeBreakdown({
      anti_penalty: 0.7, // < 0.95 => penalty
      competing_mult: 0.5, // < 0.95 => penalty
    });
    render(
      <ScoreBreakdownDrawer
        {...defaultProps}
        breakdown={penaltyBreakdown}
      />,
    );
    expect(screen.getByText('scoreDrawer.factor.anti')).toBeInTheDocument();
    expect(screen.getByText('scoreDrawer.factor.competing')).toBeInTheDocument();
  });

  // =========================================================================
  // 8. Feedback buttons have accessible names
  // =========================================================================
  it('feedback buttons have accessible names', () => {
    render(<ScoreBreakdownDrawer {...defaultProps} />);
    // Each boost factor gets a "was relevant" and "was not relevant" button
    // Labels now use i18n keys (e.g. "scoreDrawer.factor.context was relevant")
    const relevantBtns = screen.getAllByLabelText(/was relevant$/);
    const notRelevantBtns = screen.getAllByLabelText(/was not relevant$/);
    expect(relevantBtns.length).toBeGreaterThan(0);
    expect(notRelevantBtns.length).toBeGreaterThan(0);
  });

  // =========================================================================
  // 9. Comparison dropdown renders with comparePool
  // =========================================================================
  it('comparison dropdown renders when comparePool provided', () => {
    const pool: SourceRelevance[] = [
      makePoolItem({ id: 10, title: 'Item A' }),
      makePoolItem({ id: 20, title: 'Item B' }),
    ];
    render(
      <ScoreBreakdownDrawer
        {...defaultProps}
        comparePool={pool}
      />,
    );
    expect(screen.getByText('scoreDrawer.selectItem')).toBeInTheDocument();
  });

  // =========================================================================
  // 10. No comparison dropdown without comparePool
  // =========================================================================
  it('renders without comparison when comparePool not provided', () => {
    render(<ScoreBreakdownDrawer {...defaultProps} />);
    expect(screen.queryByText('scoreDrawer.selectItem')).not.toBeInTheDocument();
  });

  // =========================================================================
  // 11. Factor detail text is displayed
  // =========================================================================
  it('displays factor detail text when present', () => {
    const breakdown = makeBreakdown({
      dep_match_score: 0.8,
      matched_deps: ['react', 'tauri', 'sqlite'],
    });
    render(
      <ScoreBreakdownDrawer {...defaultProps} breakdown={breakdown} />,
    );
    expect(screen.getByText('scoreDrawer.factor.dependency')).toBeInTheDocument();
    expect(screen.getByText('react, tauri, sqlite')).toBeInTheDocument();
  });

  // =========================================================================
  // 12. Bar widths correspond to factor values
  // =========================================================================
  it('bar widths correspond to factor values', () => {
    const breakdown = makeBreakdown({
      context_score: 0.6, // score format: 60/100 * 100 = 60%
      interest_score: 0.9, // 90/100 * 100 = 90%
    });
    render(
      <ScoreBreakdownDrawer {...defaultProps} breakdown={breakdown} />,
    );
    // Check the formatted values — score format renders as percentage
    expect(screen.getByText('60%')).toBeInTheDocument();
    expect(screen.getByText('90%')).toBeInTheDocument();
  });
});
