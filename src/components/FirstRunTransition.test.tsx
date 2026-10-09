// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
// BrandMark mock — renders a simple div instead of SVG
// ---------------------------------------------------------------------------
vi.mock('./void-engine/BrandMark', () => ({
  BrandMark: ({ size }: { size?: number }) => <div data-testid="brand-mark" style={{ width: size, height: size }} />,
}));

// ---------------------------------------------------------------------------
// Store mock — reads the module-level state on every render, so a test moves
// the "store" forward by reassigning `currentAppState` and re-rendering.
// ---------------------------------------------------------------------------
const mockStartAnalysis = vi.fn();
type TestResult = {
  relevant: boolean;
  excluded?: boolean;
  title: string;
  url: string;
  source_type?: string;
  final_score: number;
  score_breakdown?: {
    dep_match_score?: number;
    matched_deps?: string[];
    skill_gap_boost?: number;
  };
};
const defaultAppState = {
  loading: false,
  progress: 0,
  progressStage: 'init' as string,
  status: '',
  analysisComplete: false,
  lastAnalyzedAt: null as Date | null,
  relevanceResults: [] as TestResult[],
};
type TestAppState = typeof defaultAppState;

let currentAppState: TestAppState = { ...defaultAppState };
let currentUserContext: { interests?: Array<{ topic: string }> } | null = null;
let currentDetectedTech: Array<{ name: string; category: string; confidence: number }> | null = null;

vi.mock('../store', () => ({
  useAppStore: (selector: (s: Record<string, unknown>) => unknown) => {
    const store = {
      appState: currentAppState,
      embeddingMode: null as string | null,
      userContext: currentUserContext,
      discoveredContext: currentDetectedTech ? { tech: currentDetectedTech } : null,
      startAnalysis: mockStartAnalysis,
    };
    return selector(store);
  },
}));

// ---------------------------------------------------------------------------
// Source metadata mock — backend not available in tests
// ---------------------------------------------------------------------------
vi.mock('../config/sources', () => ({
  getSourceLabel: (id: string) => {
    const labels: Record<string, string> = {
      hackernews: 'HN', reddit: 'Reddit', github: 'GitHub',
    };
    return labels[id] ?? id;
  },
  getSourceColorClass: () => 'bg-gray-500/20 text-gray-400',
  getSourceFullName: (id: string) => {
    const names: Record<string, string> = {
      hackernews: 'Hacker News', reddit: 'Reddit', github: 'GitHub',
    };
    return names[id] ?? id;
  },
  getSourceCategory: () => 'general',
  loadSourceMeta: vi.fn(),
  ALL_SOURCE_IDS: new Set(),
  getSourcesByCategory: () => new Map(),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { FirstRunTransition } from './FirstRunTransition';

const mockOnComplete = vi.fn();
const overlay = () => <FirstRunTransition onComplete={mockOnComplete} />;

/** Move the mocked store and re-render so the component reads the new state. */
async function setStore(view: ReturnType<typeof render>, next: Partial<TestAppState>) {
  currentAppState = { ...defaultAppState, ...next };
  await act(async () => { view.rerender(overlay()); });
}

/**
 * Drive the overlay through ITS OWN pass: mount, let the intelligence hold
 * request the pass, observe it running, then complete it after the request.
 * Celebration only ever belongs to this pass (fresh-profile E2E 2026-10-09).
 */
async function renderThroughOwnPass(final: Partial<TestAppState>) {
  const view = render(overlay());
  await act(async () => { await vi.advanceTimersByTimeAsync(2500); });
  expect(mockStartAnalysis).toHaveBeenCalledTimes(1);
  await setStore(view, { loading: true, progressStage: 'relevance', progress: 0.5 });
  await act(async () => { await vi.advanceTimersByTimeAsync(10); });
  await setStore(view, {
    ...final,
    analysisComplete: true,
    loading: false,
    progressStage: 'complete',
    progress: 1,
    lastAnalyzedAt: new Date(),
  });
  return view;
}

describe('FirstRunTransition', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    currentAppState = { ...defaultAppState };
    currentUserContext = null;
    currentDetectedTech = null;
    mockStartAnalysis.mockClear();
    mockOnComplete.mockClear();
    vi.mocked(invoke).mockResolvedValue({ has_data: false });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders with intelligence aria-label after init', async () => {
    await act(async () => {
      render(overlay());
    });

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label', 'Showing project intelligence');
    expect(status).toHaveAttribute('aria-busy', 'true');
  });

  it('renders error state with retry and continue buttons', async () => {
    currentAppState = { ...defaultAppState, progressStage: 'error', status: 'Error: something failed' };
    await act(async () => { render(overlay()); });
    await act(async () => { await vi.advanceTimersByTimeAsync(2500); });

    expect(screen.getByRole('status')).toHaveAttribute('aria-label', 'Analysis error');
    expect(screen.getByLabelText('firstRun.retryAnalysisAria')).toBeDefined();
    expect(screen.getByText('firstRun.continueAnyway')).toBeDefined();
  });

  it('retry requests a fresh pass', async () => {
    currentAppState = { ...defaultAppState, progressStage: 'error', status: 'Error: fetch failed' };
    await act(async () => { render(overlay()); });
    await act(async () => { await vi.advanceTimersByTimeAsync(2500); });
    mockStartAnalysis.mockClear();

    await act(async () => {
      fireEvent.click(screen.getByLabelText('firstRun.retryAnalysisAria'));
      await vi.advanceTimersByTimeAsync(10);
    });

    expect(mockStartAnalysis).toHaveBeenCalledTimes(1);
  });

  it('calls onComplete with results when continue anyway is clicked', async () => {
    currentAppState = { ...defaultAppState, progressStage: 'error', status: 'Error: something broke' };
    await act(async () => { render(overlay()); });
    await act(async () => { await vi.advanceTimersByTimeAsync(2500); });

    await act(async () => { fireEvent.click(screen.getByText('firstRun.continueAnyway')); });
    await act(async () => { vi.advanceTimersByTime(300); });

    expect(mockOnComplete).toHaveBeenCalledWith('results');
  });

  it('shows celebration with relevant count and CTA buttons', async () => {
    await renderThroughOwnPass({
      relevanceResults: [
        { relevant: true, title: 'Rust async patterns', url: 'https://example.com/1', final_score: 0.8 },
        { relevant: true, title: 'React hooks guide', url: 'https://example.com/2', final_score: 0.7 },
        { relevant: false, title: 'Cooking recipes', url: 'https://example.com/3', final_score: 0.1 },
      ],
    });

    expect(screen.getByText('2')).toBeDefined();
    expect(screen.getByText('firstRun.seeBriefing')).toBeDefined();
    expect(screen.getByText('firstRun.browseResults')).toBeDefined();

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label', 'Analysis complete: 2 relevant items found');
    expect(status).toHaveAttribute('aria-busy', 'false');
  });

  // Doctrine rule 3: how much was read informs no action. The overlay led with
  // a big "375 ANALYZED" (and "552 ANALYZED" next to the relevant count).
  it('shows no "analyzed" counter, only the relevant count', async () => {
    await renderThroughOwnPass({
      relevanceResults: [
        { relevant: true, title: 'A', url: 'https://a.example', final_score: 0.8 },
        { relevant: false, title: 'B', url: 'https://b.example', final_score: 0.1 },
        { relevant: false, title: 'C', url: 'https://c.example', final_score: 0.1 },
      ],
    });

    expect(screen.queryByText('analyzed')).toBeNull();
    expect(screen.queryByText('3')).toBeNull();
    expect(screen.getByText('1')).toBeDefined();
  });

  // Same predicate as the header chip: an exclusion-demoted row is not
  // "relevant" (the overlay said 21 where the app then said 20).
  it('counts relevant items with the header chip predicate (excluded rows are not relevant)', async () => {
    await renderThroughOwnPass({
      relevanceResults: [
        { relevant: true, title: 'Kept', url: 'https://k.example', final_score: 0.8 },
        { relevant: true, excluded: true, title: 'Demoted', url: 'https://d.example', final_score: 0.8 },
      ],
    });

    expect(screen.getByRole('status')).toHaveAttribute('aria-label', 'Analysis complete: 1 relevant items found');
  });

  it('calls onComplete with briefing when briefing CTA is clicked', async () => {
    await renderThroughOwnPass({
      relevanceResults: [{ relevant: true, title: 'Test article', url: 'https://test.com', final_score: 0.9 }],
    });

    await act(async () => { fireEvent.click(screen.getByText('firstRun.seeBriefing')); });
    await act(async () => { vi.advanceTimersByTime(300); });

    expect(mockOnComplete).toHaveBeenCalledWith('briefing');
  });

  it('renders the outer container with correct role and opacity classes', async () => {
    await act(async () => { render(overlay()); });

    const status = screen.getByRole('status');
    expect(status.className).toContain('fixed');
    expect(status.className).toContain('opacity-100');
  });

  it('renders BrandMark in loading phase', async () => {
    await act(async () => { render(overlay()); });
    expect(screen.getByTestId('brand-mark')).toBeDefined();
  });

  it('shows top signal title in celebration phase', async () => {
    await renderThroughOwnPass({
      relevanceResults: [
        { relevant: true, title: 'Amazing Rust Article', url: 'https://example.com/rust', final_score: 0.95 },
      ],
    });

    expect(screen.getByText('Amazing Rust Article')).toBeDefined();
    expect(screen.getByText('https://example.com/rust')).toBeDefined();
  });

  // No per-source item counts and no "sources" stat (doctrine rule 3).
  it('shows no per-source item counts or sources stat in celebration phase', async () => {
    await renderThroughOwnPass({
      relevanceResults: [
        { relevant: false, title: 'Story A', url: 'https://a.example', final_score: 0.2, source_type: 'reddit' },
        { relevant: false, title: 'Post B', url: 'https://b.example', final_score: 0.1, source_type: 'github' },
        { relevant: true, title: 'Story C', url: 'https://c.example', final_score: 0.8, source_type: 'hackernews' },
      ],
    });

    expect(screen.getByText('Story C')).toBeDefined();
    expect(screen.queryByText('sources')).toBeNull();
    expect(screen.queryAllByText((content) => content.includes('Reddit'))).toHaveLength(0);
    expect(screen.queryAllByText((content) => content.includes('GitHub'))).toHaveLength(0);
  });

  it('hides the Developer DNA header when no tech chip is confident', async () => {
    currentDetectedTech = [{ name: 'Cobol', category: 'Language', confidence: 0.3 }];
    await renderThroughOwnPass({
      relevanceResults: [{ relevant: true, title: 'Story C', url: 'https://c.example', final_score: 0.8 }],
    });
    expect(screen.queryByText('Your Developer DNA')).toBeNull();
    expect(screen.queryByText('Cobol')).toBeNull();
  });

  it('shows the Developer DNA header with confident tech chips only', async () => {
    currentDetectedTech = [
      { name: 'Cobol', category: 'Language', confidence: 0.3 },
      { name: 'Rust', category: 'Language', confidence: 0.9 },
    ];
    await renderThroughOwnPass({
      relevanceResults: [{ relevant: true, title: 'Story C', url: 'https://c.example', final_score: 0.8 }],
    });
    expect(screen.getByText('Your Developer DNA')).toBeDefined();
    expect(screen.getByText('Rust')).toBeDefined();
    expect(screen.queryByText('Cobol')).toBeNull();
  });

  it('starts analysis after intelligence hold when no scan data', async () => {
    await act(async () => { render(overlay()); });

    expect(mockStartAnalysis).not.toHaveBeenCalled();
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(mockStartAnalysis).not.toHaveBeenCalled();
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });

    expect(mockStartAnalysis).toHaveBeenCalledTimes(1);
  });

  it('shows embedding-specific error message for embedding errors', async () => {
    currentAppState = { ...defaultAppState, progressStage: 'error', status: 'Error: Embedding service unavailable' };
    await act(async () => { render(overlay()); });
    await act(async () => { await vi.advanceTimersByTimeAsync(2500); });

    expect(screen.getByText('firstRun.errorEmbedding')).toBeDefined();
    expect(screen.getByText('firstRun.basicModeExplainer')).toBeDefined();
  });

  it('applies opacity-0 class during fading phase', async () => {
    await renderThroughOwnPass({
      relevanceResults: [{ relevant: true, title: 'Test', url: 'https://test.com', final_score: 0.9 }],
    });

    await act(async () => { fireEvent.click(screen.getByText('firstRun.seeBriefing')); });
    expect(screen.getByRole('status').className).toContain('opacity-0');
  });

  it('shows stack insights when dependency matches exist', async () => {
    await renderThroughOwnPass({
      relevanceResults: [
        {
          relevant: true,
          title: 'Tokio 2.0 release',
          url: 'https://example.com',
          final_score: 0.9,
          score_breakdown: { dep_match_score: 0.5, matched_deps: ['tokio', 'serde'] },
        },
      ],
    });

    expect(screen.getByText('firstRun.insightDependencies')).toBeDefined();
  });

  it('buildStackInsights returns correct insights', async () => {
    const { buildStackInsights } = await import('./first-run/utils');

    const results = [
      { relevant: true, title: 'Rust async runtime', score_breakdown: { dep_match_score: 0.5, matched_deps: ['tokio'] } },
      { relevant: true, title: 'Python ML guide', score_breakdown: { skill_gap_boost: 0.3 } },
      { relevant: false, title: 'Irrelevant article' },
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
    expect(insights[0]).toEqual({ kind: 'dependencies', count: 1, deps: 'tokio' });
    expect(insights).toContainEqual({ kind: 'skillGap', count: 1 });
  });

  it('shows stage narration text in fetching phase', async () => {
    currentUserContext = { interests: [{ topic: 'rust' }] };
    currentAppState = { ...defaultAppState, loading: true, progressStage: 'fetch', progress: 0.3 };
    await act(async () => { render(overlay()); });
    await act(async () => { await vi.advanceTimersByTimeAsync(2500); });

    expect(screen.getByText('Reading the developer internet — pulling from your intelligence sources...')).toBeDefined();
    expect(screen.getByText('30%')).toBeDefined();
  });

  it('uses analyzing aria-label when in embed stage', async () => {
    currentAppState = { ...defaultAppState, loading: true, progressStage: 'embed', progress: 0.5 };
    await act(async () => { render(overlay()); });
    await act(async () => { await vi.advanceTimersByTimeAsync(2500); });

    expect(screen.getByRole('status')).toHaveAttribute('aria-label', 'Analyzing results');
  });

  // A user with no interests, no detected tech and no scan has nothing to be
  // "matched against" — the copy must not claim "your interests / your stack".
  it('does not claim to match "your interests" or "your stack" for an empty profile', async () => {
    currentAppState = { ...defaultAppState, loading: true, progressStage: 'relevance', progress: 0.5 };
    await act(async () => { render(overlay()); });
    await act(async () => { await vi.advanceTimersByTimeAsync(10); });

    expect(screen.getByText('firstRun.analyzingNoProfile')).toBeDefined();
    expect(screen.queryByText('firstRun.analyzing')).toBeNull();
    expect(screen.getByText('Scoring and ranking by freshness and quality...')).toBeDefined();
    expect(screen.queryAllByText((c) => c.includes('your stack'))).toHaveLength(0);
  });

  it('keeps the profile copy when the user has interests', async () => {
    currentUserContext = { interests: [{ topic: 'rust' }] };
    currentAppState = { ...defaultAppState, loading: true, progressStage: 'relevance', progress: 0.5 };
    await act(async () => { render(overlay()); });
    await act(async () => { await vi.advanceTimersByTimeAsync(10); });

    expect(screen.getByText('firstRun.analyzing')).toBeDefined();
    expect(screen.getByText('Scoring and ranking for relevance to your stack...')).toBeDefined();
  });

  it('shows honest fresh-picks message (no vanity 0, no analyzed count) when profile is empty', async () => {
    await renderThroughOwnPass({
      relevanceResults: [
        { relevant: false, title: 'Item 1', url: 'https://test.com/1', final_score: 0.1 },
        { relevant: false, title: 'Item 2', url: 'https://test.com/2', final_score: 0.05 },
      ],
    });

    expect(screen.getByText((content) => content.includes('ranks by what matters to you'))).toBeDefined();
    expect(screen.getByText('Add your stack to start ranking')).toBeDefined();
    expect(screen.queryByText('relevant')).toBeNull();
    expect(screen.queryByText('analyzed')).toBeNull();
    expect(screen.queryByText('2')).toBeNull();
    expect(screen.getByRole('status')).toHaveAttribute('aria-label', 'Scan complete');
  });

  it('shows profile-learning message when a profile exists but 0 relevant', async () => {
    currentUserContext = { interests: [{ topic: 'rust' }] };
    await renderThroughOwnPass({
      relevanceResults: [
        { relevant: false, title: 'Item 1', url: 'https://test.com/1', final_score: 0.1 },
        { relevant: false, title: 'Item 2', url: 'https://test.com/2', final_score: 0.05 },
      ],
    });

    expect(screen.getByText('0')).toBeDefined();
    expect(screen.getByText((content) => content.includes('Your profile is learning'))).toBeDefined();
  });

  // -------------------------------------------------------------------------
  // Fresh-profile E2E 2026-10-09: the overlay celebrated the background pass
  // that ran during onboarding (552 / 21) seconds after Finish, fell back to
  // "Matching…" when its own pass started, then celebrated again (812 / 20).
  // -------------------------------------------------------------------------
  describe('celebrates only its own pass, once', () => {
    const stale = [{ relevant: true, title: 'Stale pass item', url: 'https://stale.example', final_score: 0.9 }];

    it('does not celebrate a pass that completed before it asked for one', async () => {
      currentAppState = {
        ...defaultAppState,
        analysisComplete: true,
        lastAnalyzedAt: new Date(Date.now() - 60_000),
        relevanceResults: stale,
      };
      await act(async () => { render(overlay()); });
      await act(async () => { await vi.advanceTimersByTimeAsync(2500); });

      expect(screen.queryByText('Stale pass item')).toBeNull();
      expect(screen.queryByText('firstRun.seeBriefing')).toBeNull();
      expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'true');
      expect(mockStartAnalysis).toHaveBeenCalledTimes(1);
    });

    it('does not celebrate a mid-run background merge that sets analysisComplete while loading', async () => {
      const view = render(overlay());
      await act(async () => { await vi.advanceTimersByTimeAsync(2500); });
      await setStore(view, {
        loading: true,
        analysisComplete: true,
        lastAnalyzedAt: new Date(),
        progressStage: 'relevance',
        relevanceResults: stale,
      });

      expect(screen.queryByText('firstRun.seeBriefing')).toBeNull();
    });

    it('never regresses from celebration back to progress', async () => {
      const view = await renderThroughOwnPass({
        relevanceResults: [{ relevant: true, title: 'Own pass item', url: 'https://own.example', final_score: 0.9 }],
      });
      expect(screen.getByText('Own pass item')).toBeDefined();

      // A later pass (scheduled run, background refresh) starts…
      await setStore(view, { loading: true, analysisComplete: false, progressStage: 'fetch', progress: 0.1 });

      expect(screen.getByText('Own pass item')).toBeDefined();
      expect(screen.getByText('firstRun.seeBriefing')).toBeDefined();
      expect(screen.queryByText('firstRun.fetching')).toBeNull();
      expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'false');
    });

    it('waits for a pass already running at Finish, then starts and celebrates its own', async () => {
      // The pre-Finish background pass is still running when the hold ends.
      currentAppState = { ...defaultAppState, loading: true, progressStage: 'relevance', progress: 0.6 };
      const view = render(overlay());
      await act(async () => { await vi.advanceTimersByTimeAsync(2500); });
      expect(mockStartAnalysis).not.toHaveBeenCalled();

      // That pass finishes — its numbers must not be celebrated.
      await setStore(view, {
        analysisComplete: true,
        lastAnalyzedAt: new Date(),
        relevanceResults: stale,
        progressStage: 'complete',
      });
      expect(screen.queryByText('Stale pass item')).toBeNull();

      // Next poll sees it idle and requests our own pass.
      await act(async () => { await vi.advanceTimersByTimeAsync(3100); });
      expect(mockStartAnalysis).toHaveBeenCalledTimes(1);

      await setStore(view, { loading: true, progressStage: 'relevance', progress: 0.4 });
      await act(async () => { await vi.advanceTimersByTimeAsync(10); });
      await setStore(view, {
        analysisComplete: true,
        lastAnalyzedAt: new Date(),
        progressStage: 'complete',
        relevanceResults: [{ relevant: true, title: 'Fresh pass item', url: 'https://fresh.example', final_score: 0.9 }],
      });

      expect(screen.getByText('Fresh pass item')).toBeDefined();
      expect(screen.queryByText('Stale pass item')).toBeNull();
    });

    it('waits for a pass the backend reports running even when the UI does not know of it', async () => {
      let running = true;
      vi.mocked(invoke).mockImplementation(((command: string) =>
        Promise.resolve(command === 'get_analysis_status' ? { running } : { has_data: false })));

      await act(async () => { render(overlay()); });
      await act(async () => { await vi.advanceTimersByTimeAsync(2500); });
      expect(mockStartAnalysis).not.toHaveBeenCalled();

      running = false;
      await act(async () => { await vi.advanceTimersByTimeAsync(3100); });
      expect(mockStartAnalysis).toHaveBeenCalledTimes(1);
    });
  });
});
