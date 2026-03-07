import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ActionBar } from './ActionBar';
import type { Settings, SourceRelevance } from '../types';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

// Mock the Zustand store
vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
    const mockState: Record<string, unknown> = {
      embeddingMode: null,
      setShowSettings: vi.fn(),
    };
    return selector(mockState);
  }),
}));

function makeState(overrides: Partial<{
  loading: boolean;
  analysisComplete: boolean;
  status: string;
  lastAnalyzedAt: Date | null;
  progress: number;
  progressStage: string;
  relevanceResults: SourceRelevance[];
}> = {}) {
  return {
    loading: false,
    analysisComplete: false,
    status: 'Ready',
    lastAnalyzedAt: null,
    progress: 0,
    progressStage: '',
    relevanceResults: [],
    ...overrides,
  };
}

function makeSettings(overrides: Partial<Settings> = {}): Settings {
  return {
    llm: { provider: 'anthropic', model: 'claude-3-5-haiku-20241022', has_api_key: false, base_url: null },
    rerank: { enabled: false, max_items_per_batch: 10, min_embedding_score: 0.1, daily_token_limit: 100000, daily_cost_limit_cents: 50 },
    usage: { tokens_today: 0, cost_today_cents: 0, tokens_total: 0, items_reranked: 0 },
    embedding_threshold: 0.25,
    ...overrides,
  } as Settings;
}

const defaultProps = {
  state: makeState(),
  settings: makeSettings(),
  aiBriefing: { loading: false, error: null },
  autoBriefingEnabled: false,
  summaryBadges: null,
  onAnalyze: vi.fn(),
  onGenerateBriefing: vi.fn(),
  onToggleAutoBriefing: vi.fn(),
  onToast: vi.fn(),
};

describe('ActionBar', () => {
  it('renders the analysis controls region', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.getByRole('region', { name: 'Analysis controls' })).toBeInTheDocument();
  });

  it('renders the refresh button with "action.refresh" text when idle', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.getByText('action.refresh')).toBeInTheDocument();
  });

  it('shows "action.analyzing" when loading', () => {
    const props = { ...defaultProps, state: makeState({ loading: true }) };
    render(<ActionBar {...props} />);
    // The status text and button both show "action.analyzing"
    const analyzingElements = screen.getAllByText('action.analyzing');
    expect(analyzingElements.length).toBeGreaterThanOrEqual(1);
  });

  it('shows "action.analysisComplete" when analysis is done', () => {
    const props = { ...defaultProps, state: makeState({ analysisComplete: true }) };
    render(<ActionBar {...props} />);
    expect(screen.getByText('action.analysisComplete')).toBeInTheDocument();
  });

  it('calls onAnalyze when the refresh button is clicked', () => {
    const onAnalyze = vi.fn();
    render(<ActionBar {...defaultProps} onAnalyze={onAnalyze} />);
    fireEvent.click(screen.getByText('action.refresh'));
    expect(onAnalyze).toHaveBeenCalledTimes(1);
  });

  it('disables refresh button while loading', () => {
    const props = { ...defaultProps, state: makeState({ loading: true }) };
    render(<ActionBar {...props} />);
    const analyzingButtons = screen.getAllByText('action.analyzing');
    // The button variant should be disabled
    const button = analyzingButtons.find(el => el.closest('button'));
    expect(button?.closest('button')).toBeDisabled();
  });

  it('shows cancel button when loading', () => {
    const props = { ...defaultProps, state: makeState({ loading: true }) };
    render(<ActionBar {...props} />);
    expect(screen.getByText('action.cancel')).toBeInTheDocument();
  });

  it('does not show cancel button when not loading', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.queryByText('action.cancel')).not.toBeInTheDocument();
  });

  it('renders the overflow/more actions button', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.getByLabelText('More actions')).toBeInTheDocument();
  });

  it('opens overflow menu on click and shows menu items', () => {
    render(<ActionBar {...defaultProps} />);
    fireEvent.click(screen.getByLabelText('More actions'));
    expect(screen.getByText('action.regenerateBriefing')).toBeInTheDocument();
    expect(screen.getByText('action.autoBriefing')).toBeInTheDocument();
  });

  it('sets aria-expanded on overflow button', () => {
    render(<ActionBar {...defaultProps} />);
    const moreButton = screen.getByLabelText('More actions');
    expect(moreButton).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(moreButton);
    expect(moreButton).toHaveAttribute('aria-expanded', 'true');
  });

  it('shows auto-briefing toggle state as OFF', () => {
    render(<ActionBar {...defaultProps} autoBriefingEnabled={false} />);
    fireEvent.click(screen.getByLabelText('More actions'));
    expect(screen.getByText('OFF')).toBeInTheDocument();
  });

  it('shows auto-briefing toggle state as ON', () => {
    render(<ActionBar {...defaultProps} autoBriefingEnabled={true} />);
    fireEvent.click(screen.getByLabelText('More actions'));
    expect(screen.getByText('ON')).toBeInTheDocument();
  });

  it('calls onGenerateBriefing from overflow menu', () => {
    const onGenerateBriefing = vi.fn();
    const props = {
      ...defaultProps,
      onGenerateBriefing,
      state: makeState({ relevanceResults: [{ id: 1, title: 'Test', url: '', top_score: 0.5, matches: [], relevant: true }] as SourceRelevance[] }),
    };
    render(<ActionBar {...props} />);
    fireEvent.click(screen.getByLabelText('More actions'));
    fireEvent.click(screen.getByText('action.regenerateBriefing'));
    expect(onGenerateBriefing).toHaveBeenCalledTimes(1);
  });

  it('calls onToggleAutoBriefing from overflow menu', () => {
    const onToggleAutoBriefing = vi.fn();
    render(<ActionBar {...defaultProps} onToggleAutoBriefing={onToggleAutoBriefing} />);
    fireEvent.click(screen.getByLabelText('More actions'));
    fireEvent.click(screen.getByText('action.autoBriefing'));
    expect(onToggleAutoBriefing).toHaveBeenCalledTimes(1);
  });

  it('renders summary badges when provided', () => {
    const props = {
      ...defaultProps,
      summaryBadges: { relevantCount: 5, topCount: 2, total: 10 },
    };
    render(<ActionBar {...props} />);
    expect(screen.getByText('10')).toBeInTheDocument();
    expect(screen.getByText('5 rel')).toBeInTheDocument();
    expect(screen.getByText('2 top')).toBeInTheDocument();
  });

  it('does not render summary badges when null', () => {
    render(<ActionBar {...defaultProps} summaryBadges={null} />);
    expect(screen.queryByText('rel')).not.toBeInTheDocument();
  });

  it('shows LLM badge when rerank enabled and API key present', () => {
    const props = {
      ...defaultProps,
      settings: makeSettings({
        rerank: { enabled: true, max_items_per_batch: 10, min_embedding_score: 0.1, daily_token_limit: 100000, daily_cost_limit_cents: 50 },
        llm: { provider: 'anthropic', model: 'claude-3-5-haiku-20241022', has_api_key: true, base_url: null },
      } as Partial<Settings>),
    };
    render(<ActionBar {...props} />);
    expect(screen.getByText('LLM')).toBeInTheDocument();
  });

  it('shows progress bar when loading with progress > 0', () => {
    const props = {
      ...defaultProps,
      state: makeState({ loading: true, progress: 0.45, progressStage: 'fetch' }),
    };
    render(<ActionBar {...props} />);
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
    expect(screen.getByText('45%')).toBeInTheDocument();
  });

  it('does not show progress bar when not loading', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.queryByRole('progressbar')).not.toBeInTheDocument();
  });

  it('shows AI briefing error when present', () => {
    const props = {
      ...defaultProps,
      aiBriefing: { loading: false, error: 'Something went wrong' },
    };
    render(<ActionBar {...props} />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByText('Something went wrong')).toBeInTheDocument();
  });

  it('does not show error alert when no error', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows export options in overflow menu when analysis is complete', () => {
    const props = {
      ...defaultProps,
      state: makeState({ analysisComplete: true }),
    };
    render(<ActionBar {...props} />);
    fireEvent.click(screen.getByLabelText('More actions'));
    expect(screen.getByText('action.exportMarkdown')).toBeInTheDocument();
    expect(screen.getByText('action.exportDigest')).toBeInTheDocument();
  });

  it('does not show export options when analysis is not complete', () => {
    render(<ActionBar {...defaultProps} />);
    fireEvent.click(screen.getByLabelText('More actions'));
    expect(screen.queryByText('action.exportMarkdown')).not.toBeInTheDocument();
    expect(screen.queryByText('action.exportDigest')).not.toBeInTheDocument();
  });
});
