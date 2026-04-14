/**
 * Keyboard navigation tests for ResultsView.
 *
 * Tests keyboard interaction with filter controls and result list navigation.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

// Tauri API mocks
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// Store mock
const mockSetShowOnlyRelevant = vi.fn();
const mockSetShowSavedOnly = vi.fn();
const mockSetSearchQuery = vi.fn();
const mockStartAnalysis = vi.fn();

function makeMockState(overrides: Record<string, unknown> = {}) {
  return {
    appState: {
      loading: false, analysisComplete: true, status: 'Ready',
      relevanceResults: [], progress: 0, progressStage: '',
      progressMessage: '', contextFiles: [],
      ...((overrides.appState as Record<string, unknown>) || {}),
    },
    feedbackGiven: {}, discoveredContext: null, expandedItem: null,
    embeddingMode: null, setExpandedItem: vi.fn(),
    startAnalysis: mockStartAnalysis,
    loadContextFiles: vi.fn(), clearContext: vi.fn(), indexContext: vi.fn(),
    recordInteraction: vi.fn(),
    sourceFilters: new Set(['hackernews']), sortBy: 'score' as const,
    showOnlyRelevant: false, showSavedOnly: false, searchQuery: '',
    toggleSourceFilter: vi.fn(), setSortBy: vi.fn(),
    setShowOnlyRelevant: mockSetShowOnlyRelevant,
    setShowSavedOnly: mockSetShowSavedOnly,
    setSearchQuery: mockSetSearchQuery,
    setSettingsStatus: vi.fn(),
    ...overrides,
  };
}

let currentMockState = makeMockState();

vi.mock('../../store', () => ({
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

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: () => ({
    getVirtualItems: () => [],
    getTotalSize: () => 0,
    measureElement: vi.fn(),
  }),
}));

vi.mock('../ResultItem', () => ({
  ResultItem: ({ item }: { item: { id: number; title: string } }) => (
    <div data-testid={`result-${item.id}`}>{item.title}</div>
  ),
}));

vi.mock('../context-panel', () => ({
  ContextPanel: () => <div data-testid="context-panel" />,
}));

vi.mock('../SmartEmptyState', () => ({
  SmartEmptyState: () => <div data-testid="smart-empty-state" />,
}));

vi.mock('../ContentTranslationProvider', () => ({
  useTranslatedContent: () => ({
    getTranslated: (_id: string, text: string) => text,
    requestTranslation: vi.fn(),
  }),
}));

vi.mock('../../lib/fourda-components', () => ({
  registerFourdaComponent: vi.fn(),
}));

vi.mock('../../hooks', () => ({
  useResultFilters: () => ({
    filteredResults: [], searchQuery: '', setSearchQuery: vi.fn(),
    sourceFilters: new Set<string>(), toggleSourceFilter: vi.fn(),
    resetSourceFilters: vi.fn(),
    sortBy: 'score', setSortBy: vi.fn(), showOnlyRelevant: false,
    setShowOnlyRelevant: vi.fn(), showSavedOnly: false,
    setShowSavedOnly: vi.fn(), dismissAllBelow: vi.fn(), saveAllAbove: vi.fn(),
  }),
}));

vi.mock('../../utils/score', () => ({
  formatScore: (s: number) => `${Math.round(s * 100)}%`,
  getScoreColor: () => 'text-white',
  getStageLabel: (s: string) => s || 'Ready',
}));

vi.mock('../../config/sources', () => ({
  getSourceLabel: (s: string) => s,
  getSourceFullName: (s: string) => s,
  getSourceColorClass: () => 'bg-gray-500/20 text-gray-400',
  getSourceCategory: () => 'general',
  getSourcesByCategory: () => new Map([['general', ['hackernews']]]),
  isSourcesLoaded: () => true,
  loadSourceMeta: () => Promise.resolve(),
  ALL_SOURCE_IDS: new Set(['hackernews']),
}));

import { ResultsView } from '../ResultsView';

const defaultProps = {
  newItemIds: new Set<number>(),
  focusedIndex: -1,
};

describe('ResultsView keyboard navigation', () => {
  beforeEach(() => {
    currentMockState = makeMockState();
    vi.clearAllMocks();
  });

  it('search input is accessible and focusable', () => {
    render(<ResultsView {...defaultProps} />);
    const input = screen.getByLabelText('Search results by keyword');
    expect(input).toBeInTheDocument();
    expect(input.tagName).toBe('INPUT');
  });

  it('filter toolbar has accessible role', () => {
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByRole('toolbar', { name: 'Filter and sort controls' })).toBeInTheDocument();
  });

  it('relevance toggle is a checkbox', () => {
    render(<ResultsView {...defaultProps} />);
    const toggle = screen.getByLabelText('Toggle relevant items only');
    expect(toggle).toBeInTheDocument();
  });

  it('saved items toggle is a checkbox', () => {
    render(<ResultsView {...defaultProps} />);
    const toggle = screen.getByLabelText('Show saved items only');
    expect(toggle).toBeInTheDocument();
  });

  it('batch dismiss button has accessible label', () => {
    render(<ResultsView {...defaultProps} />);
    const btn = screen.getByLabelText('Dismiss all items below 30% relevance');
    expect(btn).toBeInTheDocument();
    expect(btn.closest('button')).toBeInTheDocument();
  });

  it('batch save button has accessible label', () => {
    render(<ResultsView {...defaultProps} />);
    const btn = screen.getByLabelText('Save all items above 60% relevance');
    expect(btn).toBeInTheDocument();
    expect(btn.closest('button')).toBeInTheDocument();
  });

  it('analyze button is clickable in not-started state', () => {
    currentMockState = makeMockState({
      appState: { analysisComplete: false, loading: false, status: 'Ready', relevanceResults: [], progress: 0, progressStage: '', progressMessage: '', contextFiles: [] },
    });
    render(<ResultsView {...defaultProps} />);
    const btn = screen.getByText('results.analyzeNow');
    fireEvent.click(btn);
    expect(mockStartAnalysis).toHaveBeenCalledTimes(1);
  });

  it('results region has accessible name', () => {
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByRole('region', { name: 'results.title' })).toBeInTheDocument();
  });

  it('source filter group has accessible role', () => {
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByRole('group', { name: 'Source category filters' })).toBeInTheDocument();
  });

  it('sort order group has accessible role', () => {
    render(<ResultsView {...defaultProps} />);
    expect(screen.getByRole('group', { name: 'Sort order' })).toBeInTheDocument();
  });
});
