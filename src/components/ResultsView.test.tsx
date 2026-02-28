/**
 * ResultsView component tests
 *
 * Tests rendering states (not-started, loading, empty results, with results),
 * filter/sort controls accessibility, and aria-live announcement.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

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
// Store mock — default state: analysis complete, no results
// ---------------------------------------------------------------------------
const mockSetExpandedItem = vi.fn();
const mockStartAnalysis = vi.fn();
const mockLoadContextFiles = vi.fn();
const mockClearContext = vi.fn();
const mockIndexContext = vi.fn();
const mockRecordInteraction = vi.fn();

function makeMockState(overrides: Record<string, unknown> = {}) {
  return {
    appState: {
      loading: false,
      analysisComplete: true,
      status: 'Ready',
      relevanceResults: [],
      progress: 0,
      progressStage: '',
      progressMessage: '',
      contextFiles: [],
      ...((overrides.appState as Record<string, unknown>) || {}),
    },
    feedbackGiven: {},
    discoveredContext: null,
    expandedItem: null,
    embeddingMode: null,
    setExpandedItem: mockSetExpandedItem,
    startAnalysis: mockStartAnalysis,
    loadContextFiles: mockLoadContextFiles,
    clearContext: mockClearContext,
    indexContext: mockIndexContext,
    recordInteraction: mockRecordInteraction,
    // Filter state used by useResultFilters
    sourceFilters: new Set(['hackernews']),
    sortBy: 'score' as const,
    showOnlyRelevant: false,
    showSavedOnly: false,
    searchQuery: '',
    toggleSourceFilter: vi.fn(),
    setSortBy: vi.fn(),
    setShowOnlyRelevant: vi.fn(),
    setShowSavedOnly: vi.fn(),
    setSearchQuery: vi.fn(),
    setSettingsStatus: vi.fn(),
    ...overrides,
  };
}

let currentMockState = makeMockState();

vi.mock('../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) =>
      selector(currentMockState as unknown as Record<string, unknown>),
    ),
    { getState: () => ({ setShowSettings: vi.fn() }) },
  ),
}));

vi.mock('zustand/react/shallow', () => ({
  useShallow: (fn: unknown) => fn,
}));

// ---------------------------------------------------------------------------
// Mock virtualizer — returns empty virtual items by default
// ---------------------------------------------------------------------------
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: () => ({
    getVirtualItems: () => [],
    getTotalSize: () => 0,
    measureElement: vi.fn(),
  }),
}));

// ---------------------------------------------------------------------------
// Mock child components
// ---------------------------------------------------------------------------
vi.mock('./ResultItem', () => ({
  ResultItem: ({ item }: { item: { id: number; title: string } }) => (
    <div data-testid={`result-${item.id}`}>{item.title}</div>
  ),
}));

vi.mock('./context-panel', () => ({
  ContextPanel: () => <div data-testid="context-panel" />,
}));

// ---------------------------------------------------------------------------
// Mock hooks and utils
// ---------------------------------------------------------------------------
let mockFilterState: Record<string, unknown> = {};

vi.mock('../hooks', () => ({
  useResultFilters: () => ({
    filteredResults: [],
    searchQuery: '',
    setSearchQuery: vi.fn(),
    sourceFilters: new Set<string>(),
    toggleSourceFilter: vi.fn(),
    sortBy: 'score',
    setSortBy: vi.fn(),
    showOnlyRelevant: false,
    setShowOnlyRelevant: vi.fn(),
    showSavedOnly: false,
    setShowSavedOnly: vi.fn(),
    dismissAllBelow: vi.fn(),
    saveAllAbove: vi.fn(),
    ...mockFilterState,
  }),
}));

vi.mock('../utils/score', () => ({
  formatScore: (s: number) => `${Math.round(s * 100)}%`,
  getScoreColor: () => 'text-white',
  getStageLabel: (s: string) => s || 'Ready',
}));

vi.mock('../config/sources', () => ({
  getSourceLabel: (s: string) => s,
  getSourceFullName: (s: string) => s,
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { ResultsView } from './ResultsView';

const defaultProps = {
  newItemIds: new Set<number>(),
  focusedIndex: -1,
  renderLimit: 50,
  setRenderLimit: vi.fn(),
};

describe('ResultsView', () => {
  beforeEach(() => {
    currentMockState = makeMockState();
    mockFilterState = {};
    vi.clearAllMocks();
  });

  // =========================================================================
  // 1. Basic rendering
  // =========================================================================
  it('renders without crashing', () => {
    const { unmount } = render(<ResultsView {...defaultProps} />);
    expect(screen.getByRole('region', { name: 'results.title' })).toBeInTheDocument();
    unmount();
  });

  // =========================================================================
  // 2. Not-started state (analysis not complete, not loading)
  // =========================================================================
  it('shows not-started state when analysis not complete', () => {
    currentMockState = makeMockState({
      appState: { analysisComplete: false, loading: false, status: 'Ready', relevanceResults: [], progress: 0, progressStage: '', progressMessage: '', contextFiles: [] },
    });
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByText('results.noResults')).toBeInTheDocument();
    expect(screen.getByText('results.startAnalysis')).toBeInTheDocument();
  });

  // =========================================================================
  // 3. Analyze button in not-started state
  // =========================================================================
  it('shows analyze button in not-started state', () => {
    currentMockState = makeMockState({
      appState: { analysisComplete: false, loading: false, status: 'Ready', relevanceResults: [], progress: 0, progressStage: '', progressMessage: '', contextFiles: [] },
    });
    render(<ResultsView {...defaultProps} />);
    const btn = screen.getByText('results.analyzeNow');
    expect(btn).toBeInTheDocument();
    fireEvent.click(btn);
    expect(mockStartAnalysis).toHaveBeenCalledTimes(1);
  });

  // =========================================================================
  // 4. Loading state with progress
  // =========================================================================
  it('shows loading state with progress', () => {
    currentMockState = makeMockState({
      appState: {
        analysisComplete: false,
        loading: true,
        status: 'Analyzing',
        relevanceResults: [],
        progress: 0.5,
        progressStage: 'fetch',
        progressMessage: 'Fetching sources...',
        contextFiles: [],
      },
    });
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByText('action.analyzing')).toBeInTheDocument();
    expect(screen.getByText('Fetching sources...')).toBeInTheDocument();
  });

  // =========================================================================
  // 5. Progress bar during loading
  // =========================================================================
  it('shows progress bar during loading', () => {
    currentMockState = makeMockState({
      appState: {
        analysisComplete: false,
        loading: true,
        status: 'Analyzing',
        relevanceResults: [],
        progress: 0.5,
        progressStage: 'fetch',
        progressMessage: '',
        contextFiles: [],
      },
    });
    render(<ResultsView {...defaultProps} />);
    // The progress bar is the inner div with style width 50%
    expect(screen.getByText('50%')).toBeInTheDocument();
    expect(screen.getByText('fetch')).toBeInTheDocument();
  });

  // =========================================================================
  // 6. Results count text is displayed
  // =========================================================================
  it('displays results count text', () => {
    render(<ResultsView {...defaultProps} />);
    // With analysisComplete=true, shows the itemsRelevant i18n key
    expect(screen.getByText('results.itemsRelevant')).toBeInTheDocument();
  });

  // =========================================================================
  // 7. aria-live attribute on results count
  // =========================================================================
  it('results count has aria-live="polite" attribute', () => {
    render(<ResultsView {...defaultProps} />);
    const countEl = screen.getByText('results.itemsRelevant');
    expect(countEl).toHaveAttribute('aria-live', 'polite');
  });

  // =========================================================================
  // 8. Filter/search bar is shown
  // =========================================================================
  it('shows filter/search bar when analysis is complete', () => {
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByRole('toolbar', { name: 'Filter and sort controls' })).toBeInTheDocument();
  });

  // =========================================================================
  // 9. Search input has accessible label
  // =========================================================================
  it('search input has accessible label', () => {
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByLabelText('Search results by keyword')).toBeInTheDocument();
  });

  // =========================================================================
  // 10. Source filter group is accessible
  // =========================================================================
  it('source filter group is accessible', () => {
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByRole('group', { name: 'Source filters' })).toBeInTheDocument();
  });

  // =========================================================================
  // 11. Sort buttons are accessible
  // =========================================================================
  it('sort buttons are accessible', () => {
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByRole('group', { name: 'Sort order' })).toBeInTheDocument();
  });

  // =========================================================================
  // 12. Relevance toggle is accessible
  // =========================================================================
  it('relevance toggle is accessible', () => {
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByLabelText('Toggle relevant items only')).toBeInTheDocument();
  });

  // =========================================================================
  // 13. Saved items toggle is accessible
  // =========================================================================
  it('saved items toggle is accessible', () => {
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByLabelText('Show saved items only')).toBeInTheDocument();
  });

  // =========================================================================
  // 14. No-match message when filters yield empty
  // =========================================================================
  it('shows no-match message when filters yield empty results', () => {
    currentMockState = makeMockState({
      appState: {
        analysisComplete: true,
        loading: false,
        status: 'Ready',
        relevanceResults: [{ id: 1, title: 'Test', top_score: 0.5 }],
        progress: 0,
        progressStage: '',
        progressMessage: '',
        contextFiles: [],
      },
    });
    // filteredResults is empty (via mock hook) but relevanceResults has items
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByText('results.noMatch')).toBeInTheDocument();
  });

  // =========================================================================
  // 15. Show-all button in no-match state with showOnlyRelevant
  // =========================================================================
  it('shows show-all button to reset relevance filter in no-match state', () => {
    currentMockState = makeMockState({
      appState: {
        analysisComplete: true,
        loading: false,
        status: 'Ready',
        relevanceResults: [{ id: 1, title: 'Test', top_score: 0.5 }],
        progress: 0,
        progressStage: '',
        progressMessage: '',
        contextFiles: [],
      },
    });
    mockFilterState = { showOnlyRelevant: true };
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByText('results.showAll')).toBeInTheDocument();
  });

  // =========================================================================
  // 16. Batch operations buttons are accessible
  // =========================================================================
  it('batch operations buttons are accessible', () => {
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByLabelText('Dismiss all items below 30% relevance')).toBeInTheDocument();
    expect(screen.getByLabelText('Save all items above 60% relevance')).toBeInTheDocument();
  });

  // =========================================================================
  // 17. Keyboard hint element exists
  // =========================================================================
  it('keyboard hint element exists in not-started state', () => {
    currentMockState = makeMockState({
      appState: { analysisComplete: false, loading: false, status: 'Ready', relevanceResults: [], progress: 0, progressStage: '', progressMessage: '', contextFiles: [] },
    });
    render(<ResultsView {...defaultProps} />);
    const matches = screen.getAllByText('R');
    const kbd = matches.find((el) => el.tagName === 'KBD');
    expect(kbd).toBeDefined();
    expect(kbd!.tagName).toBe('KBD');
  });
});
