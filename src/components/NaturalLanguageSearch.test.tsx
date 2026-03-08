import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { NaturalLanguageSearch } from './NaturalLanguageSearch';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => mockInvoke(...args) }));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (opts) {
        let r = key;
        for (const [k, v] of Object.entries(opts)) r = r.replace(`{{${k}}}`, String(v));
        return r;
      }
      return key;
    },
  }),
}));

let mockState: Record<string, unknown> = {};
function setMockState(overrides: Record<string, unknown> = {}) {
  mockState = { appState: { analysisComplete: true, lastAnalyzedAt: '2026-01-01' }, ...overrides };
}
vi.mock('../store', () => ({
  useAppStore: vi.fn((sel: (s: Record<string, unknown>) => unknown) => sel(mockState)),
}));

let mockIsPro = false;
vi.mock('../hooks', () => ({ useLicense: () => ({ isPro: mockIsPro, trialStatus: null }) }));

vi.mock('./search/StackHealthBar', () => ({
  StackHealthBar: ({ onSuggestedQuery }: { onSuggestedQuery: (q: string) => void }) => (
    <div data-testid="stack-health-bar">
      <button data-testid="suggested-query-btn" onClick={() => onSuggestedQuery('suggested query')}>suggest</button>
    </div>
  ),
}));
vi.mock('./search/SynthesisPanel', () => ({
  SynthesisPanel: ({ synthesis, loading }: { synthesis: string | null; loading: boolean }) => (
    <div data-testid="synthesis-panel">
      {loading && <span data-testid="synthesis-loading">loading</span>}
      {synthesis && <span data-testid="synthesis-text">{synthesis}</span>}
    </div>
  ),
}));
vi.mock('./search/GhostPreview', () => ({ GhostPreview: () => <div data-testid="ghost-preview" /> }));
vi.mock('./search/StandingQueries', () => ({ StandingQueries: () => <div data-testid="standing-queries" /> }));

// --- Helpers ---

function makeResult(overrides: Record<string, unknown> = {}) {
  return {
    query: 'test query', intent: 'Find',
    items: [{
      id: 1, file_path: '/path/to/file.rs', file_name: 'file.rs', preview: 'A Rust source file',
      relevance: 0.85, source_type: 'context', timestamp: '2026-01-15T10:00:00Z', match_reason: 'keyword match',
    }],
    total_count: 1, execution_ms: 42, summary: null,
    parsed: { keywords: ['test'], entities: [], time_range: null, file_types: [], sentiment: null, confidence: 0.92 },
    stack_context: [], related_decisions: [], knowledge_gaps: [],
    ghost_preview: null, is_pro: false, ...overrides,
  };
}

/** Sets up mockInvoke to route commands. Override specific commands via the map. */
function setupInvoke(routes: Record<string, unknown> = {}) {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd in routes) return Promise.resolve(routes[cmd]);
    if (cmd === 'get_stack_health') return Promise.resolve(null);
    return Promise.resolve(null);
  });
}

/** Sets up mockInvoke so a specific command rejects. */
function setupInvokeReject(cmd: string, error: string) {
  mockInvoke.mockImplementation((c: string) => {
    if (c === cmd) return Promise.reject(error);
    if (c === 'get_stack_health') return Promise.resolve(null);
    return Promise.resolve(null);
  });
}

/** Types into the search input and clicks the search button. */
async function searchFor(query: string) {
  fireEvent.change(screen.getByLabelText('Natural language search query'), { target: { value: query } });
  fireEvent.click(screen.getByText('action.search'));
}

beforeEach(() => {
  vi.clearAllMocks();
  mockIsPro = false;
  setMockState();
  setupInvoke();
});

// --- Tests ---

describe('NaturalLanguageSearch', () => {
  describe('rendering and expand/collapse', () => {
    it('renders the section header with title and subtitle', () => {
      render(<NaturalLanguageSearch />);
      expect(screen.getByText('search.title')).toBeInTheDocument();
      expect(screen.getByText('search.subtitle')).toBeInTheDocument();
    });

    it('renders expanded by default with search input visible', () => {
      render(<NaturalLanguageSearch />);
      expect(screen.getByRole('search')).toBeInTheDocument();
      expect(screen.getByLabelText('Natural language search query')).toBeInTheDocument();
    });

    it('collapses when header is clicked', () => {
      render(<NaturalLanguageSearch />);
      fireEvent.click(screen.getByRole('button', { name: /search\.collapsePanel/i }));
      expect(screen.queryByRole('search')).not.toBeInTheDocument();
    });

    it('starts collapsed when defaultExpanded is false', () => {
      render(<NaturalLanguageSearch defaultExpanded={false} />);
      expect(screen.queryByRole('search')).not.toBeInTheDocument();
    });
  });

  describe('first-run capability preview', () => {
    it('shows capability preview when no analysis has run', () => {
      setMockState({ appState: { analysisComplete: false, lastAnalyzedAt: null } });
      render(<NaturalLanguageSearch />);
      expect(screen.getByText('search.noAnalysisTitle')).toBeInTheDocument();
      expect(screen.getByText('search.capabilityStack')).toBeInTheDocument();
      expect(screen.getByText('search.capabilityDecisions')).toBeInTheDocument();
      expect(screen.getByText('search.capabilityGaps')).toBeInTheDocument();
      expect(screen.getByText('search.capabilitySynthesis')).toBeInTheDocument();
    });

    it('does not show capability preview when analysis is complete', () => {
      render(<NaturalLanguageSearch />);
      expect(screen.queryByText('search.noAnalysisTitle')).not.toBeInTheDocument();
    });
  });

  describe('stack health on mount', () => {
    it('calls get_stack_health and renders StackHealthBar', () => {
      render(<NaturalLanguageSearch />);
      expect(mockInvoke).toHaveBeenCalledWith('get_stack_health');
      expect(screen.getByTestId('stack-health-bar')).toBeInTheDocument();
    });
  });

  describe('search submission and results', () => {
    it('submits query on button click and displays results', async () => {
      setupInvoke({ natural_language_query: makeResult() });
      render(<NaturalLanguageSearch />);
      await searchFor('test query');
      await waitFor(() => expect(screen.getByText('file.rs')).toBeInTheDocument());
      expect(mockInvoke).toHaveBeenCalledWith('natural_language_query', { queryText: 'test query' });
      expect(screen.getByText('85%')).toBeInTheDocument();
      expect(screen.getByText('keyword match')).toBeInTheDocument();
    });

    it('submits query on Enter key', async () => {
      setupInvoke({ natural_language_query: makeResult() });
      render(<NaturalLanguageSearch />);
      const input = screen.getByLabelText('Natural language search query');
      fireEvent.change(input, { target: { value: 'enter query' } });
      fireEvent.keyDown(input, { key: 'Enter' });
      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('natural_language_query', { queryText: 'enter query' }));
    });

    it('does not submit empty query', () => {
      render(<NaturalLanguageSearch />);
      fireEvent.click(screen.getByText('action.search'));
      expect(mockInvoke).not.toHaveBeenCalledWith('natural_language_query', expect.anything());
    });

    it('calls onStatusChange with result stats', async () => {
      const statusFn = vi.fn();
      setupInvoke({ natural_language_query: makeResult({ total_count: 5, execution_ms: 100 }) });
      render(<NaturalLanguageSearch onStatusChange={statusFn} />);
      await searchFor('status test');
      await waitFor(() => expect(statusFn).toHaveBeenCalledWith('Found 5 results in 100ms'));
    });

    it('shows no-results message when items array is empty', async () => {
      setupInvoke({ natural_language_query: makeResult({ items: [], total_count: 0 }) });
      render(<NaturalLanguageSearch />);
      await searchFor('empty results');
      await waitFor(() => {
        expect(screen.getByText('search.noResults')).toBeInTheDocument();
        expect(screen.getByText('search.tryDifferent')).toBeInTheDocument();
      });
    });

    it('shows query parsing info with intent and keywords', async () => {
      const parsed = { keywords: ['rust', 'async'], entities: [], time_range: null, file_types: [], sentiment: null, confidence: 0.88 };
      setupInvoke({ natural_language_query: makeResult({ intent: 'Summarize', parsed }) });
      render(<NaturalLanguageSearch />);
      await searchFor('q');
      await waitFor(() => {
        expect(screen.getByText('Summarize')).toBeInTheDocument();
        expect(screen.getByText('rust, async')).toBeInTheDocument();
      });
    });

    it('clears results when clear button is clicked', async () => {
      setupInvoke({ natural_language_query: makeResult() });
      render(<NaturalLanguageSearch />);
      await searchFor('q');
      await waitFor(() => expect(screen.getByText('file.rs')).toBeInTheDocument());
      fireEvent.click(screen.getByLabelText('Clear search results'));
      expect(screen.queryByText('file.rs')).not.toBeInTheDocument();
    });
  });

  describe('error handling', () => {
    it('displays error alert when search fails', async () => {
      setupInvokeReject('natural_language_query', 'Something broke');
      render(<NaturalLanguageSearch />);
      await searchFor('fail');
      await waitFor(() => {
        expect(screen.getByRole('alert')).toBeInTheDocument();
        expect(screen.getByText('Something broke')).toBeInTheDocument();
      });
    });

    it('shows indexFirst message for "No context" errors', async () => {
      setupInvokeReject('natural_language_query', 'No context available');
      render(<NaturalLanguageSearch />);
      await searchFor('q');
      await waitFor(() => expect(screen.getByText('search.indexFirst')).toBeInTheDocument());
    });

    it('dismisses error when dismiss button is clicked', async () => {
      setupInvokeReject('natural_language_query', 'Oops');
      render(<NaturalLanguageSearch />);
      await searchFor('q');
      await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
      fireEvent.click(screen.getByLabelText('Dismiss error'));
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });
  });

  describe('free vs Pro tiering', () => {
    it('shows GhostPreview for free users when ghost_preview data exists', async () => {
      const ghost = { total_results: 10, hidden_results: 5, decision_count: 2, gap_count: 1, synthesis_available: true };
      setupInvoke({ natural_language_query: makeResult({ is_pro: false, ghost_preview: ghost }) });
      render(<NaturalLanguageSearch />);
      await searchFor('q');
      await waitFor(() => expect(screen.getByTestId('ghost-preview')).toBeInTheDocument());
    });

    it('does not show GhostPreview for Pro users', async () => {
      const ghost = { total_results: 10, hidden_results: 5, decision_count: 2, gap_count: 1, synthesis_available: true };
      setupInvoke({ natural_language_query: makeResult({ is_pro: true, ghost_preview: ghost }), synthesize_search: 'syn' });
      render(<NaturalLanguageSearch />);
      await searchFor('q');
      await waitFor(() => expect(screen.getByTestId('synthesis-panel')).toBeInTheDocument());
      expect(screen.queryByTestId('ghost-preview')).not.toBeInTheDocument();
    });

    it('triggers synthesis fetch for Pro results', async () => {
      setupInvoke({ natural_language_query: makeResult({ is_pro: true }), synthesize_search: 'AI synthesis' });
      render(<NaturalLanguageSearch />);
      await searchFor('pro query');
      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('synthesize_search', { queryText: 'pro query' }));
    });

    it('does not trigger synthesis for free results', async () => {
      setupInvoke({ natural_language_query: makeResult({ is_pro: false }) });
      render(<NaturalLanguageSearch />);
      await searchFor('q');
      await waitFor(() => expect(screen.getByText('file.rs')).toBeInTheDocument());
      expect(mockInvoke).not.toHaveBeenCalledWith('synthesize_search', expect.anything());
    });

    it('shows StandingQueries only for Pro users', () => {
      mockIsPro = true;
      render(<NaturalLanguageSearch />);
      expect(screen.getByTestId('standing-queries')).toBeInTheDocument();
    });

    it('hides StandingQueries for free users', () => {
      render(<NaturalLanguageSearch />);
      expect(screen.queryByTestId('standing-queries')).not.toBeInTheDocument();
    });

    it('shows related decisions for Pro users with results', async () => {
      mockIsPro = true;
      const decisions = [{ id: 1, subject: 'Auth Architecture', decision: 'Use JWT tokens', relation: 'direct' }];
      setupInvoke({ natural_language_query: makeResult({ related_decisions: decisions }) });
      render(<NaturalLanguageSearch />);
      await searchFor('q');
      await waitFor(() => {
        expect(screen.getByText('search.relatedDecisions')).toBeInTheDocument();
        expect(screen.getByText('Auth Architecture')).toBeInTheDocument();
      });
    });

    it('shows knowledge gaps for Pro users with results', async () => {
      mockIsPro = true;
      const gaps = [{ technology: 'React Server Components', days_stale: 45, severity: 'high' }];
      setupInvoke({ natural_language_query: makeResult({ knowledge_gaps: gaps }) });
      render(<NaturalLanguageSearch />);
      await searchFor('q');
      await waitFor(() => {
        expect(screen.getByText('search.knowledgeGaps')).toBeInTheDocument();
        expect(screen.getByText('React Server Components')).toBeInTheDocument();
      });
    });
  });

  describe('watch button (standing queries)', () => {
    it('shows watch button for Pro users after search', async () => {
      mockIsPro = true;
      setupInvoke({ natural_language_query: makeResult() });
      render(<NaturalLanguageSearch />);
      await searchFor('watch me');
      await waitFor(() => expect(screen.getByTitle('search.watchThis')).toBeInTheDocument());
    });

    it('calls create_standing_query when watch button is clicked', async () => {
      mockIsPro = true;
      setupInvoke({ natural_language_query: makeResult(), create_standing_query: null });
      render(<NaturalLanguageSearch />);
      await searchFor('watch me');
      await waitFor(() => expect(screen.getByTitle('search.watchThis')).toBeInTheDocument());
      fireEvent.click(screen.getByTitle('search.watchThis'));
      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('create_standing_query', { queryText: 'watch me' }));
    });

    it('does not show watch button for free users', async () => {
      setupInvoke({ natural_language_query: makeResult() });
      render(<NaturalLanguageSearch />);
      await searchFor('q');
      await waitFor(() => expect(screen.getByText('file.rs')).toBeInTheDocument());
      expect(screen.queryByTitle('search.watchThis')).not.toBeInTheDocument();
    });
  });

  describe('suggested queries and stack context', () => {
    it('populates search input when suggested query is clicked from StackHealthBar', () => {
      render(<NaturalLanguageSearch />);
      fireEvent.click(screen.getByTestId('suggested-query-btn'));
      const input = screen.getByLabelText('Natural language search query') as HTMLInputElement;
      expect(input.value).toBe('suggested query');
    });

    it('shows example query buttons when no results are displayed', () => {
      render(<NaturalLanguageSearch />);
      expect(screen.getByText('search.tryThese')).toBeInTheDocument();
      expect(screen.getByText('show me files about authentication')).toBeInTheDocument();
    });

    it('shows relevant stack technologies in results', async () => {
      const ctx = [{ name: 'Rust', category: 'language', relevant: true }, { name: 'Python', category: 'language', relevant: false }];
      setupInvoke({ natural_language_query: makeResult({ stack_context: ctx }) });
      render(<NaturalLanguageSearch />);
      await searchFor('q');
      await waitFor(() => {
        expect(screen.getByText('Rust')).toBeInTheDocument();
        expect(screen.getByText('search.yourStack:')).toBeInTheDocument();
      });
    });
  });
});
