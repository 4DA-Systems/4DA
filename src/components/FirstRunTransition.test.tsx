import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';

// ---------------------------------------------------------------------------
// Tauri API mocks
// ---------------------------------------------------------------------------
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({ has_data: false })),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// ---------------------------------------------------------------------------
// i18n mock — return key as text
// ---------------------------------------------------------------------------
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultOrOpts?: string | Record<string, unknown>) => {
      if (typeof defaultOrOpts === 'string') return defaultOrOpts;
      if (defaultOrOpts && typeof defaultOrOpts === 'object' && 'defaultValue' in defaultOrOpts) {
        return defaultOrOpts.defaultValue as string;
      }
      return key;
    },
    i18n: { language: 'en', changeLanguage: vi.fn() },
  }),
}));

// ---------------------------------------------------------------------------
// VoidEngine mock — renders a simple div instead of canvas/WebGL
// ---------------------------------------------------------------------------
vi.mock('./void-engine/VoidEngine', () => ({
  VoidEngine: ({ size }: { size?: number }) => <div data-testid="void-engine" style={{ width: size, height: size }} />,
}));

// ---------------------------------------------------------------------------
// GAME components mock — custom elements use ResizeObserver (unsupported in jsdom)
// ---------------------------------------------------------------------------
vi.mock('../lib/game-components', () => ({
  registerGameComponent: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Store mock
// ---------------------------------------------------------------------------
const mockStartAnalysis = vi.fn();
const defaultAppState = {
  loading: false,
  progress: 0,
  progressStage: 'init' as string,
  status: '',
  analysisComplete: false,
  relevanceResults: [] as Array<{
    relevant: boolean;
    title: string;
    url: string;
    source_type?: string;
    final_score: number;
    score_breakdown?: {
      dep_match_score?: number;
      matched_deps?: string[];
      skill_gap_boost?: number;
    };
  }>,
};

let currentAppState = { ...defaultAppState };

vi.mock('../store', () => ({
  useAppStore: (selector: (s: Record<string, unknown>) => unknown) => {
    const store = {
      appState: currentAppState,
      embeddingMode: null as string | null,
      userContext: null as { interests?: Array<{ topic: string }> } | null,
      startAnalysis: mockStartAnalysis,
    };
    return selector(store);
  },
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { FirstRunTransition } from './FirstRunTransition';

describe('FirstRunTransition', () => {
  const mockOnComplete = vi.fn();

  beforeEach(() => {
    vi.useFakeTimers();
    currentAppState = { ...defaultAppState };
    mockStartAnalysis.mockClear();
    mockOnComplete.mockClear();
    vi.mocked(invoke).mockResolvedValue({ has_data: false });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // -------------------------------------------------------------------------
  // 1. Always transitions to intelligence phase on mount
  // -------------------------------------------------------------------------
  it('renders with intelligence aria-label after init', async () => {
    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label', 'Showing project intelligence');
    expect(status).toHaveAttribute('aria-busy', 'true');
  });

  // -------------------------------------------------------------------------
  // 2. Shows error state when progressStage is 'error'
  // -------------------------------------------------------------------------
  it('renders error state with retry and continue buttons', async () => {
    currentAppState = {
      ...defaultAppState,
      progressStage: 'error',
      status: 'Error: something failed',
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    // Wait for the phase transition effect to fire
    await act(async () => {
      await vi.runAllTimersAsync();
    });

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label', 'Analysis error');

    // Retry button exists
    const retryBtn = screen.getByLabelText('Retry analysis');
    expect(retryBtn).toBeDefined();

    // Continue anyway button exists
    expect(screen.getByText('firstRun.continueAnyway')).toBeDefined();
  });

  // -------------------------------------------------------------------------
  // 3. Retry button clears error and restarts analysis
  // -------------------------------------------------------------------------
  it('calls startAnalysis when retry is clicked', async () => {
    currentAppState = {
      ...defaultAppState,
      progressStage: 'error',
      status: 'Error: fetch failed',
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    const retryBtn = screen.getByLabelText('Retry analysis');
    await act(async () => {
      fireEvent.click(retryBtn);
    });

    expect(mockStartAnalysis).toHaveBeenCalled();
  });

  // -------------------------------------------------------------------------
  // 4. Continue anyway triggers onComplete after fade
  // -------------------------------------------------------------------------
  it('calls onComplete with results when continue anyway is clicked', async () => {
    currentAppState = {
      ...defaultAppState,
      progressStage: 'error',
      status: 'Error: something broke',
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    const continueBtn = screen.getByText('firstRun.continueAnyway');
    await act(async () => {
      fireEvent.click(continueBtn);
    });

    // Fade timer (300ms)
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(mockOnComplete).toHaveBeenCalledWith('results');
  });

  // -------------------------------------------------------------------------
  // 5. Celebration phase renders relevant count and CTA buttons
  // -------------------------------------------------------------------------
  it('shows celebration with relevant count and CTA buttons', async () => {
    currentAppState = {
      ...defaultAppState,
      analysisComplete: true,
      relevanceResults: [
        { relevant: true, title: 'Rust async patterns', url: 'https://example.com/1', final_score: 0.8 },
        { relevant: true, title: 'React hooks guide', url: 'https://example.com/2', final_score: 0.7 },
        { relevant: false, title: 'Cooking recipes', url: 'https://example.com/3', final_score: 0.1 },
      ],
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    // Should show "2" as the big relevant count
    expect(screen.getByText('2')).toBeDefined();

    // Briefing CTA
    expect(screen.getByText('firstRun.seeBriefing')).toBeDefined();

    // Results CTA
    expect(screen.getByText('firstRun.browseResults')).toBeDefined();

    // aria-label should reflect celebrating phase
    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label', 'Analysis complete: 2 relevant items found');
    expect(status).toHaveAttribute('aria-busy', 'false');
  });

  // -------------------------------------------------------------------------
  // 6. Briefing CTA triggers onComplete('briefing') after fade
  // -------------------------------------------------------------------------
  it('calls onComplete with briefing when briefing CTA is clicked', async () => {
    currentAppState = {
      ...defaultAppState,
      analysisComplete: true,
      relevanceResults: [
        { relevant: true, title: 'Test article', url: 'https://test.com', final_score: 0.9 },
      ],
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    const briefingBtn = screen.getByText('firstRun.seeBriefing');
    await act(async () => {
      fireEvent.click(briefingBtn);
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(mockOnComplete).toHaveBeenCalledWith('briefing');
  });

  // -------------------------------------------------------------------------
  // 7. Outer container has correct structure (role, classes)
  // -------------------------------------------------------------------------
  it('renders the outer container with correct role and opacity classes', async () => {
    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    const status = screen.getByRole('status');
    expect(status.className).toContain('fixed');
    expect(status.className).toContain('opacity-100');
  });

  // -------------------------------------------------------------------------
  // 8. VoidEngine renders in loading states
  // -------------------------------------------------------------------------
  it('renders VoidEngine in loading phase', async () => {
    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    expect(screen.getByTestId('void-engine')).toBeDefined();
  });

  // -------------------------------------------------------------------------
  // 9. Top signal is displayed in celebration phase
  // -------------------------------------------------------------------------
  it('shows top signal title in celebration phase', async () => {
    currentAppState = {
      ...defaultAppState,
      analysisComplete: true,
      relevanceResults: [
        { relevant: true, title: 'Amazing Rust Article', url: 'https://example.com/rust', final_score: 0.95 },
      ],
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    expect(screen.getByText('Amazing Rust Article')).toBeDefined();
    expect(screen.getByText('https://example.com/rust')).toBeDefined();
  });

  // -------------------------------------------------------------------------
  // 10. Source breakdown pills show in celebration
  // -------------------------------------------------------------------------
  it('shows source breakdown pills in celebration phase', async () => {
    currentAppState = {
      ...defaultAppState,
      analysisComplete: true,
      relevanceResults: [
        { relevant: true, title: 'HN Story', url: 'https://hn.com', final_score: 0.8, source_type: 'hackernews' },
        { relevant: true, title: 'Reddit Post', url: 'https://reddit.com', final_score: 0.7, source_type: 'reddit' },
      ],
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    // Source full names should appear (may match multiple elements)
    expect(screen.getAllByText((content) => content.includes('Hacker News')).length).toBeGreaterThan(0);
    expect(screen.getAllByText((content) => content.includes('Reddit')).length).toBeGreaterThan(0);
  });

  // -------------------------------------------------------------------------
  // 11. Starts analysis on mount (after 2s intelligence hold when no scan data)
  // -------------------------------------------------------------------------
  it('starts analysis after intelligence hold when no scan data', async () => {
    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    // Analysis should not be called immediately
    expect(mockStartAnalysis).not.toHaveBeenCalled();

    // Not yet after 1s
    await act(async () => {
      vi.advanceTimersByTime(1000);
    });
    expect(mockStartAnalysis).not.toHaveBeenCalled();

    // After 2s intelligence hold
    await act(async () => {
      vi.advanceTimersByTime(1000);
    });

    expect(mockStartAnalysis).toHaveBeenCalledTimes(1);
  });

  // -------------------------------------------------------------------------
  // 12. Error state shows embedding-specific messaging
  // -------------------------------------------------------------------------
  it('shows embedding-specific error message for embedding errors', async () => {
    currentAppState = {
      ...defaultAppState,
      progressStage: 'error',
      status: 'Error: Embedding service unavailable',
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    // Should show embedding-specific error text
    expect(screen.getByText('firstRun.errorEmbedding')).toBeDefined();
    // Should show basic mode explainer
    expect(screen.getByText('firstRun.basicModeExplainer')).toBeDefined();
  });

  // -------------------------------------------------------------------------
  // 13. Fading phase applies opacity-0
  // -------------------------------------------------------------------------
  it('applies opacity-0 class during fading phase', async () => {
    currentAppState = {
      ...defaultAppState,
      analysisComplete: true,
      relevanceResults: [
        { relevant: true, title: 'Test', url: 'https://test.com', final_score: 0.9 },
      ],
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    // Click briefing to trigger fading
    const briefingBtn = screen.getByText('firstRun.seeBriefing');
    await act(async () => {
      fireEvent.click(briefingBtn);
    });

    const status = screen.getByRole('status');
    expect(status.className).toContain('opacity-0');
  });

  // -------------------------------------------------------------------------
  // 14. Stack insights render in celebration when dep matches exist
  // -------------------------------------------------------------------------
  it('shows stack insights when dependency matches exist', async () => {
    currentAppState = {
      ...defaultAppState,
      analysisComplete: true,
      relevanceResults: [
        {
          relevant: true,
          title: 'Tokio 2.0 release',
          url: 'https://example.com',
          final_score: 0.9,
          score_breakdown: {
            dep_match_score: 0.5,
            matched_deps: ['tokio', 'serde'],
          },
        },
      ],
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    // Stack insight about dependencies should appear
    expect(screen.getByText((content) => content.includes('articles about your dependencies'))).toBeDefined();
  });

  // -------------------------------------------------------------------------
  // 15. buildStackInsights utility works correctly
  // -------------------------------------------------------------------------
  it('buildStackInsights returns correct insights', async () => {
    const { buildStackInsights } = await import('./first-run/utils');

    const results = [
      {
        relevant: true,
        title: 'Rust async runtime',
        score_breakdown: { dep_match_score: 0.5, matched_deps: ['tokio'] },
      },
      {
        relevant: true,
        title: 'Python ML guide',
        score_breakdown: { skill_gap_boost: 0.3 },
      },
      {
        relevant: false,
        title: 'Irrelevant article',
      },
    ];

    const scanSummary = {
      projects_scanned: 3,
      total_dependencies: 50,
      dependencies_by_ecosystem: { rust: 20, npm: 25, python: 5, other: 0 },
      languages: ['Rust', 'TypeScript'],
      frameworks: ['Tauri', 'React'],
      primary_stack: 'Rust + TypeScript',
      key_packages: ['tokio', 'react'],
      has_data: true,
    };

    const insights = buildStackInsights(results, scanSummary);

    expect(insights.length).toBeGreaterThan(0);
    expect(insights[0]).toContain('articles about your dependencies');
    expect(insights[0]).toContain('tokio');
  });

  // -------------------------------------------------------------------------
  // 16. Fetching phase shows stage narration
  // -------------------------------------------------------------------------
  it('shows stage narration text in fetching phase', async () => {
    currentAppState = {
      ...defaultAppState,
      loading: true,
      progressStage: 'fetch',
      progress: 0.3,
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    // Should show narration for fetch stage
    expect(screen.getByText('Connecting to 11 intelligence sources...')).toBeDefined();

    // Should show progress percentage
    expect(screen.getByText('30%')).toBeDefined();
  });

  // -------------------------------------------------------------------------
  // 17. Analyzing phase uses correct aria-label
  // -------------------------------------------------------------------------
  it('uses analyzing aria-label when in embed stage', async () => {
    currentAppState = {
      ...defaultAppState,
      loading: true,
      progressStage: 'embed',
      progress: 0.5,
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label', 'Analyzing results');
  });

  // -------------------------------------------------------------------------
  // 18. Zero relevant items shows appropriate celebration message
  // -------------------------------------------------------------------------
  it('shows profile learning message when zero relevant items', async () => {
    currentAppState = {
      ...defaultAppState,
      analysisComplete: true,
      relevanceResults: [
        { relevant: false, title: 'Item 1', url: 'https://test.com/1', final_score: 0.1 },
        { relevant: false, title: 'Item 2', url: 'https://test.com/2', final_score: 0.05 },
      ],
    };

    await act(async () => {
      render(<FirstRunTransition onComplete={mockOnComplete} />);
    });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    // Should show "0" as relevant count
    expect(screen.getByText('0')).toBeDefined();

    // Should show the profile learning message
    expect(screen.getByText((content) => content.includes('Your profile is learning'))).toBeDefined();
  });
});
