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
// Store mock
// ---------------------------------------------------------------------------
vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
      const mockState: Record<string, unknown> = {
        embeddingMode: null,
        setShowSettings: vi.fn(),
      };
      return selector(mockState);
    }),
    {
      getState: () => ({
        setShowSettings: vi.fn(),
      }),
    },
  ),
}));

// Mock score utils (getStageLabel used by ActionBar)
vi.mock('../../utils/score', () => ({
  formatScore: (s: number) => `${Math.round(s * 100)}%`,
  getScoreColor: () => 'text-white',
  getStageLabel: (s: string) => s || 'Ready',
}));

// Mock AudioBriefing and ContextHandoff children
vi.mock('../AudioBriefing', () => ({
  AudioBriefing: () => <button data-testid="audio-briefing">Audio</button>,
}));

vi.mock('../ContextHandoff', () => ({
  ContextHandoff: () => <button data-testid="context-handoff">Handoff</button>,
}));

// Mock game components
vi.mock('../../lib/game-components', () => ({
  registerGameComponent: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { ActionBar } from '../ActionBar';
import { makeSettings } from '../../test/factories';

function makeState(overrides = {}) {
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

describe('ActionBar', () => {
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

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders without crash', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.getByRole('region')).toBeInTheDocument();
  });

  it('has accessible region label', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.getByRole('region')).toHaveAttribute('aria-label', 'Analysis controls');
  });

  it('shows ready state text when not loading', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.getByText('action.ready')).toBeInTheDocument();
  });

  it('shows the Refresh button', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.getByText('action.refresh')).toBeInTheDocument();
  });

  it('calls onAnalyze when Refresh button is clicked', () => {
    const onAnalyze = vi.fn();
    render(<ActionBar {...defaultProps} onAnalyze={onAnalyze} />);
    fireEvent.click(screen.getByText('action.refresh'));
    expect(onAnalyze).toHaveBeenCalledTimes(1);
  });

  it('shows analyzing state when loading', () => {
    render(
      <ActionBar
        {...defaultProps}
        state={makeState({ loading: true })}
      />,
    );
    // "action.analyzing" appears in both status text and button label
    const analyzingElements = screen.getAllByText('action.analyzing');
    expect(analyzingElements.length).toBeGreaterThanOrEqual(1);
  });

  it('shows cancel button when loading', () => {
    render(
      <ActionBar
        {...defaultProps}
        state={makeState({ loading: true })}
      />,
    );
    expect(screen.getByText('action.cancel')).toBeInTheDocument();
  });

  it('disables refresh button when loading', () => {
    render(
      <ActionBar
        {...defaultProps}
        state={makeState({ loading: true })}
      />,
    );
    // The refresh button contains "action.analyzing" text and is disabled
    const buttons = screen.getAllByText('action.analyzing');
    const refreshBtn = buttons.find((el) => el.closest('button'))?.closest('button');
    expect(refreshBtn).toBeDisabled();
  });

  it('shows progress bar when loading with progress', () => {
    render(
      <ActionBar
        {...defaultProps}
        state={makeState({
          loading: true,
          progress: 0.5,
          progressStage: 'Fetching sources',
        })}
      />,
    );
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '50');
  });

  it('does not show progress bar when not loading', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.queryByRole('progressbar')).not.toBeInTheDocument();
  });

  it('shows analysis complete state', () => {
    render(
      <ActionBar
        {...defaultProps}
        state={makeState({ analysisComplete: true })}
      />,
    );
    expect(screen.getByText('action.analysisComplete')).toBeInTheDocument();
  });

  it('shows AI briefing error alert', () => {
    render(
      <ActionBar
        {...defaultProps}
        aiBriefing={{ loading: false, error: 'API quota exceeded' }}
      />,
    );
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByText('API quota exceeded')).toBeInTheDocument();
  });

  it('does not show error alert when no error', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows LLM badge when rerank is enabled with API key', () => {
    render(
      <ActionBar
        {...defaultProps}
        settings={makeSettings({
          rerank: {
            enabled: true,
            max_items_per_batch: 10,
            min_embedding_score: 0.3,
            daily_token_limit: 100000,
            daily_cost_limit_cents: 50,
          },
          llm: {
            provider: 'openai',
            model: 'gpt-4o',
            has_api_key: true,
            base_url: null,
          },
        })}
      />,
    );
    expect(screen.getByText('LLM')).toBeInTheDocument();
  });

  it('does not show LLM badge when rerank is disabled', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.queryByText('LLM')).not.toBeInTheDocument();
  });

  it('shows summary badges when provided', () => {
    render(
      <ActionBar
        {...defaultProps}
        summaryBadges={{ relevantCount: 15, topCount: 3, total: 42 }}
      />,
    );
    expect(screen.getByText('42')).toBeInTheDocument();
    expect(screen.getByText('15 rel')).toBeInTheDocument();
    expect(screen.getByText('3 top')).toBeInTheDocument();
  });

  it('does not show top count badge when topCount is 0', () => {
    render(
      <ActionBar
        {...defaultProps}
        summaryBadges={{ relevantCount: 5, topCount: 0, total: 20 }}
      />,
    );
    expect(screen.getByText('20')).toBeInTheDocument();
    expect(screen.getByText('5 rel')).toBeInTheDocument();
    expect(screen.queryByText('0 top')).not.toBeInTheDocument();
  });

  it('has an overflow menu button', () => {
    render(<ActionBar {...defaultProps} />);
    expect(screen.getByLabelText('More actions')).toBeInTheDocument();
  });

  it('opens overflow menu on click', () => {
    render(<ActionBar {...defaultProps} />);
    fireEvent.click(screen.getByLabelText('More actions'));
    expect(screen.getByText('action.regenerateBriefing')).toBeInTheDocument();
    expect(screen.getByText('action.autoBriefing')).toBeInTheDocument();
  });

  it('shows auto briefing toggle state in overflow menu', () => {
    render(
      <ActionBar {...defaultProps} autoBriefingEnabled={true} />,
    );
    fireEvent.click(screen.getByLabelText('More actions'));
    expect(screen.getByText('ON')).toBeInTheDocument();
  });

  it('shows OFF state for auto briefing when disabled', () => {
    render(
      <ActionBar {...defaultProps} autoBriefingEnabled={false} />,
    );
    fireEvent.click(screen.getByLabelText('More actions'));
    expect(screen.getByText('OFF')).toBeInTheDocument();
  });

  it('calls onGenerateBriefing from overflow menu', () => {
    const onGenerateBriefing = vi.fn();
    render(
      <ActionBar
        {...defaultProps}
        onGenerateBriefing={onGenerateBriefing}
        state={makeState({ relevanceResults: [{ id: 1 }] })}
      />,
    );
    fireEvent.click(screen.getByLabelText('More actions'));
    fireEvent.click(screen.getByText('action.regenerateBriefing'));
    expect(onGenerateBriefing).toHaveBeenCalledTimes(1);
  });

  it('calls onToggleAutoBriefing from overflow menu', () => {
    const onToggleAutoBriefing = vi.fn();
    render(
      <ActionBar {...defaultProps} onToggleAutoBriefing={onToggleAutoBriefing} />,
    );
    fireEvent.click(screen.getByLabelText('More actions'));
    fireEvent.click(screen.getByText('action.autoBriefing'));
    expect(onToggleAutoBriefing).toHaveBeenCalledTimes(1);
  });

  it('shows up-to-date state when recently analyzed', () => {
    render(
      <ActionBar
        {...defaultProps}
        state={makeState({
          lastAnalyzedAt: new Date(), // just now
        })}
      />,
    );
    expect(screen.getByText('action.upToDate')).toBeInTheDocument();
  });

  it('shows last analysis time', () => {
    const time = new Date();
    render(
      <ActionBar
        {...defaultProps}
        state={makeState({
          analysisComplete: true,
          lastAnalyzedAt: time,
        })}
      />,
    );
    // The time should be displayed in HH:MM format
    const expectedTime = time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    expect(screen.getByText(new RegExp(expectedTime))).toBeInTheDocument();
  });

  it('shows export options in overflow when analysis is complete', () => {
    render(
      <ActionBar
        {...defaultProps}
        state={makeState({ analysisComplete: true })}
      />,
    );
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
