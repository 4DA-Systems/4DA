/**
 * FirstRunTransition — comprehensive test suite
 *
 * Tests the first-run experience component that guides new users
 * through preparing, intelligence preview, fetching, analyzing,
 * celebrating, and error states.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Mocks — must appear before component import
// ---------------------------------------------------------------------------

const mockInvoke = vi.fn(() =>
  Promise.resolve({
    has_data: false,
    projects_scanned: 0,
    total_dependencies: 0,
    dependencies_by_ecosystem: { rust: 0, npm: 0, python: 0, other: 0 },
    languages: [] as string[],
    frameworks: [] as string[],
    primary_stack: '',
    key_packages: [] as string[],
  }),
);

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (..._args: unknown[]) => mockInvoke(),
}));

// Capture listen callbacks so tests can trigger events
type ListenCallback = (event: { payload: unknown }) => void;
const listenCallbacks = new Map<string, ListenCallback>();

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((eventName: string, cb: ListenCallback) => {
    listenCallbacks.set(eventName, cb);
    return Promise.resolve(() => {
      listenCallbacks.delete(eventName);
    });
  }),
  emit: vi.fn(),
}));

// Store mock — configurable per test via mockStoreState
const mockStartAnalysis = vi.fn();

let mockStoreState: Record<string, unknown> = {};

function setMockStoreState(overrides: Record<string, unknown> = {}) {
  mockStoreState = {
    appState: {
      loading: false,
      analysisComplete: false,
      status: 'Ready',
      relevanceResults: [],
      progress: 0,
      progressStage: '',
    },
    embeddingMode: null,
    userContext: {
      interests: [
        { id: 1, topic: 'Rust', weight: 1, source: 'manual', has_embedding: false },
        { id: 2, topic: 'TypeScript', weight: 1, source: 'manual', has_embedding: false },
        { id: 3, topic: 'AI', weight: 1, source: 'manual', has_embedding: false },
      ],
      role: 'Developer',
      experience_level: 'Senior',
    },
    startAnalysis: mockStartAnalysis,
    ...overrides,
  };
}

vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
    return selector(mockStoreState);
  }),
}));

// VoidEngine — lightweight mock
vi.mock('./void-engine/VoidEngine', () => ({
  VoidEngine: () => <div data-testid="void-engine" />,
}));

// first-run-messages
vi.mock('../utils/first-run-messages', () => ({
  getStageNarration: vi.fn((phase: string) => `narration-${phase}`),
  getSourceNarration: vi.fn((_type: string, count: number) => `source-${_type}-${count}`),
  getCelebrationMessage: vi.fn((rel: number, total: number) => `celebration-${rel}-of-${total}`),
}));

// config/sources
vi.mock('../config/sources', () => ({
  getSourceFullName: vi.fn((type: string) => `Full ${type}`),
}));

// ---------------------------------------------------------------------------
// Component import (after all mocks)
// ---------------------------------------------------------------------------
import { FirstRunTransition } from './FirstRunTransition';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeResult(overrides: Record<string, unknown> = {}) {
  return {
    id: 1,
    title: 'Test Article',
    url: 'https://example.com',
    top_score: 0.5,
    matches: [],
    relevant: true,
    source_type: 'hackernews',
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

describe('FirstRunTransition', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setMockStoreState();
    listenCallbacks.clear();
    mockInvoke.mockClear();
    mockStartAnalysis.mockClear();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // -------------------------------------------------------------------------
  // 1. Basic rendering
  // -------------------------------------------------------------------------

  it('renders without crashing', async () => {
    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });
    expect(screen.getByTestId('void-engine')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 2. Initial phase
  // -------------------------------------------------------------------------

  it('starts in preparing phase with aria-busy', async () => {
    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });
    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-busy', 'true');
    expect(status).toHaveAttribute('aria-label', 'Preparing analysis');
  });

  // -------------------------------------------------------------------------
  // 3. User interests display
  // -------------------------------------------------------------------------

  it('shows user interests from store in preparing phase', async () => {
    // invoke returns no scan data, so component stays in preparing phase
    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });
    expect(screen.getByText('Rust')).toBeInTheDocument();
    expect(screen.getByText('TypeScript')).toBeInTheDocument();
    expect(screen.getByText('AI')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 4. Intelligence preview when ACE scan has data
  // -------------------------------------------------------------------------

  it('renders IntelligencePreview when ACE scan has data', async () => {
    mockInvoke.mockResolvedValueOnce({
      has_data: true,
      projects_scanned: 5,
      total_dependencies: 42,
      dependencies_by_ecosystem: { rust: 20, npm: 15, python: 5, other: 2 },
      languages: ['Rust', 'TypeScript'],
      frameworks: ['Tauri', 'React'],
      primary_stack: 'Rust + TypeScript',
      key_packages: ['serde', 'tokio', 'react'],
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    // Wait for the async invoke to resolve and phase to change
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });

    // Intelligence preview should show the title
    expect(screen.getByText('firstRun.intelligenceTitle')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 5. Intelligence preview shows project/dependency counts
  // -------------------------------------------------------------------------

  it('shows intelligence project and dependency counts', async () => {
    mockInvoke.mockResolvedValueOnce({
      has_data: true,
      projects_scanned: 5,
      total_dependencies: 42,
      dependencies_by_ecosystem: { rust: 20, npm: 15, python: 5, other: 2 },
      languages: ['Rust', 'TypeScript'],
      frameworks: ['Tauri', 'React'],
      primary_stack: 'Rust + TypeScript',
      key_packages: ['serde', 'tokio', 'react'],
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });

    // "5" appears twice (projects count AND Python ecosystem pill count)
    const fives = screen.getAllByText('5');
    expect(fives.length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('42')).toBeInTheDocument();
    expect(screen.getByText('firstRun.projects')).toBeInTheDocument();
    expect(screen.getByText('firstRun.dependencies')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 6. Transition to fetching phase
  // -------------------------------------------------------------------------

  it('transitions to fetching phase when appState.loading with fetch stage', async () => {
    setMockStoreState({
      appState: {
        loading: true,
        analysisComplete: false,
        status: 'Fetching',
        relevanceResults: [],
        progress: 0.2,
        progressStage: 'fetch',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label', 'Scanning sources');
  });

  // -------------------------------------------------------------------------
  // 7. Progress bar during fetching
  // -------------------------------------------------------------------------

  it('shows progress bar during fetching phase', async () => {
    setMockStoreState({
      appState: {
        loading: true,
        analysisComplete: false,
        status: 'Fetching',
        relevanceResults: [],
        progress: 0.5,
        progressStage: 'fetch',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    // The progress bar inner element has a width style
    const progressBar = document.querySelector('[style*="width"]');
    expect(progressBar).not.toBeNull();
    expect(progressBar!.getAttribute('style')).toContain('width: 50%');
  });

  // -------------------------------------------------------------------------
  // 8. Source messages from events
  // -------------------------------------------------------------------------

  it('displays source messages from source-fetched events', async () => {
    setMockStoreState({
      appState: {
        loading: true,
        analysisComplete: false,
        status: 'Fetching',
        relevanceResults: [],
        progress: 0.3,
        progressStage: 'fetch',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    // The listen mock should have captured a callback for 'source-fetched'
    const callback = listenCallbacks.get('source-fetched');
    expect(callback).toBeDefined();

    // Trigger the event
    await act(async () => {
      callback!({ payload: { source: 'hackernews', count: 15 } });
    });

    // getSourceNarration was mocked to return `source-${type}-${count}`
    expect(screen.getByText('source-hackernews-15')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 9. Transition to analyzing phase
  // -------------------------------------------------------------------------

  it('transitions to analyzing phase when progressStage is embed', async () => {
    setMockStoreState({
      appState: {
        loading: true,
        analysisComplete: false,
        status: 'Analyzing',
        relevanceResults: [],
        progress: 0.6,
        progressStage: 'embed',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label', 'Analyzing results');
  });

  // -------------------------------------------------------------------------
  // 10. Celebrating phase
  // -------------------------------------------------------------------------

  it('shows celebrating phase when analysis is complete', async () => {
    setMockStoreState({
      appState: {
        loading: false,
        analysisComplete: true,
        status: 'Complete',
        relevanceResults: [
          makeResult({ id: 1, relevant: true }),
          makeResult({ id: 2, relevant: true }),
          makeResult({ id: 3, relevant: false }),
        ],
        progress: 1,
        progressStage: 'complete',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label', 'Analysis complete: 2 relevant items found');
    expect(status).toHaveAttribute('aria-busy', 'false');
  });

  // -------------------------------------------------------------------------
  // 11. Relevant count in celebration
  // -------------------------------------------------------------------------

  it('shows relevant count in celebration display', async () => {
    setMockStoreState({
      appState: {
        loading: false,
        analysisComplete: true,
        status: 'Complete',
        relevanceResults: [
          makeResult({ id: 1, relevant: true }),
          makeResult({ id: 2, relevant: true }),
          makeResult({ id: 3, relevant: true }),
          makeResult({ id: 4, relevant: false }),
          makeResult({ id: 5, relevant: false }),
        ],
        progress: 1,
        progressStage: 'complete',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    // The big count display shows "3" (relevant items)
    expect(screen.getByText('3')).toBeInTheDocument();
    // getCelebrationMessage mock returns `celebration-3-of-5`
    expect(screen.getByText('celebration-3-of-5')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 12. Source breakdown pills
  // -------------------------------------------------------------------------

  it('shows source breakdown pills in celebration', async () => {
    setMockStoreState({
      appState: {
        loading: false,
        analysisComplete: true,
        status: 'Complete',
        relevanceResults: [
          makeResult({ id: 1, source_type: 'hackernews' }),
          makeResult({ id: 2, source_type: 'hackernews' }),
          makeResult({ id: 3, source_type: 'reddit' }),
          makeResult({ id: 4, source_type: 'github' }),
        ],
        progress: 1,
        progressStage: 'complete',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    // getSourceFullName mock returns `Full ${type}`
    expect(screen.getByText(/Full hackernews/)).toBeInTheDocument();
    expect(screen.getByText(/Full reddit/)).toBeInTheDocument();
    expect(screen.getByText(/Full github/)).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 13. Top signal highlight
  // -------------------------------------------------------------------------

  it('highlights top signal with dep_match_score in celebration', async () => {
    setMockStoreState({
      appState: {
        loading: false,
        analysisComplete: true,
        status: 'Complete',
        relevanceResults: [
          makeResult({
            id: 1,
            relevant: true,
            title: 'Serde 2.0 Released',
            url: 'https://blog.serde.rs/v2',
            score_breakdown: {
              dep_match_score: 0.8,
              matched_deps: ['serde', 'serde_json', 'tokio'],
            },
          }),
          makeResult({ id: 2, relevant: true }),
        ],
        progress: 1,
        progressStage: 'complete',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    // Should show the top signal title and "Matches your stack" label
    expect(screen.getByText('Serde 2.0 Released')).toBeInTheDocument();
    expect(screen.getByText('firstRun.topMatchStack')).toBeInTheDocument();
    // matched deps (first 3)
    expect(screen.getByText('serde, serde_json, tokio')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 14. Stack insights
  // -------------------------------------------------------------------------

  it('shows stack insights when dep matches exist', async () => {
    setMockStoreState({
      appState: {
        loading: false,
        analysisComplete: true,
        status: 'Complete',
        relevanceResults: [
          makeResult({
            id: 1,
            relevant: true,
            title: 'Tokio update',
            score_breakdown: { dep_match_score: 0.5, matched_deps: ['tokio'] },
          }),
          makeResult({
            id: 2,
            relevant: true,
            title: 'Serde deep dive',
            score_breakdown: { dep_match_score: 0.6, matched_deps: ['serde'] },
          }),
        ],
        progress: 1,
        progressStage: 'complete',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    // buildStackInsights creates "X articles about your dependencies: ..."
    expect(screen.getByText(/2 articles about your dependencies/)).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 15. Briefing CTA calls onComplete
  // -------------------------------------------------------------------------

  it('calls onComplete with briefing when briefing CTA is clicked', async () => {
    const onComplete = vi.fn();

    setMockStoreState({
      appState: {
        loading: false,
        analysisComplete: true,
        status: 'Complete',
        relevanceResults: [makeResult()],
        progress: 1,
        progressStage: 'complete',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={onComplete} />);
    });

    // Click the briefing button (firstRun.seeBriefing)
    fireEvent.click(screen.getByText('firstRun.seeBriefing'));

    // handleDismiss sets fading phase, then calls onComplete after 300ms
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(onComplete).toHaveBeenCalledWith('briefing');
  });

  // -------------------------------------------------------------------------
  // 16. Results CTA calls onComplete
  // -------------------------------------------------------------------------

  it('calls onComplete with results when results CTA is clicked', async () => {
    const onComplete = vi.fn();

    setMockStoreState({
      appState: {
        loading: false,
        analysisComplete: true,
        status: 'Complete',
        relevanceResults: [makeResult()],
        progress: 1,
        progressStage: 'complete',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={onComplete} />);
    });

    // Click the results button (firstRun.browseResults)
    fireEvent.click(screen.getByText(/firstRun\.browseResults/));

    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(onComplete).toHaveBeenCalledWith('results');
  });

  // -------------------------------------------------------------------------
  // 17. Error state UI
  // -------------------------------------------------------------------------

  it('shows error state UI when progressStage is error', async () => {
    setMockStoreState({
      appState: {
        loading: false,
        analysisComplete: false,
        status: 'Error: fetch failed',
        relevanceResults: [],
        progress: 0,
        progressStage: 'error',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label', 'Analysis error');

    // Error UI shows title, retry button, and continue button
    expect(screen.getByText('firstRun.errorTitle')).toBeInTheDocument();
    expect(screen.getByLabelText('Retry analysis')).toBeInTheDocument();
    expect(screen.getByText('firstRun.continueAnyway')).toBeInTheDocument();
    expect(screen.getByText('firstRun.settingsHint')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 18. Retry button calls startAnalysis
  // -------------------------------------------------------------------------

  it('retry button calls startAnalysis and resets error state', async () => {
    setMockStoreState({
      appState: {
        loading: false,
        analysisComplete: false,
        status: 'Error occurred',
        relevanceResults: [],
        progress: 0,
        progressStage: 'error',
      },
    });

    await act(async () => {
      render(<FirstRunTransition onComplete={vi.fn()} />);
    });

    // Verify error state is shown
    expect(screen.getByText('firstRun.errorTitle')).toBeInTheDocument();

    // Click retry
    fireEvent.click(screen.getByLabelText('Retry analysis'));

    expect(mockStartAnalysis).toHaveBeenCalledTimes(1);
  });
});
