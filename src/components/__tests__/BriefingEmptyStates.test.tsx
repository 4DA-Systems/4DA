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
const mockStartAnalysis = vi.fn();
const mockGenerateBriefing = vi.fn();
const mockSetActiveView = vi.fn();

vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
    const mockState: Record<string, unknown> = {
      appState: {
        loading: false,
        analysisComplete: false,
        status: 'Ready',
        relevanceResults: [],
        progress: 0,
        progressStage: '',
      },
      aiBriefing: {
        content: null,
        loading: false,
        error: null,
        model: null,
      },
      startAnalysis: mockStartAnalysis,
      generateBriefing: mockGenerateBriefing,
      setActiveView: mockSetActiveView,
    };
    return selector(mockState);
  }),
}));

// ---------------------------------------------------------------------------
// Components under test
// ---------------------------------------------------------------------------
import {
  BriefingLoadingState,
  BriefingReadyState,
  BriefingNoDataState,
} from '../BriefingEmptyStates';

describe('BriefingLoadingState', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders without crash', () => {
    render(<BriefingLoadingState />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('has aria-busy true while loading', () => {
    render(<BriefingLoadingState />);
    expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'true');
  });

  it('shows gathering intelligence heading', () => {
    render(<BriefingLoadingState />);
    expect(screen.getByText('briefing.gatheringIntelligence')).toBeInTheDocument();
  });

  it('shows analysis running message', () => {
    render(<BriefingLoadingState />);
    expect(screen.getByText('briefing.loadingStageInit')).toBeInTheDocument();
  });

  it('does not show browse results when no results exist', () => {
    render(<BriefingLoadingState />);
    expect(screen.queryByText(/briefing\.browseResults/)).not.toBeInTheDocument();
  });
});

describe('BriefingReadyState', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders without crash', () => {
    render(<BriefingReadyState />);
    expect(screen.getByText('briefing.readyToGenerate')).toBeInTheDocument();
  });

  it('shows results analyzed count', () => {
    render(<BriefingReadyState />);
    expect(screen.getByText('briefing.resultsAnalyzed')).toBeInTheDocument();
  });

  it('shows generate briefing button', () => {
    render(<BriefingReadyState />);
    expect(screen.getByText('briefing.generate')).toBeInTheDocument();
  });

  it('calls generateBriefing when button is clicked', () => {
    render(<BriefingReadyState />);
    fireEvent.click(screen.getByText('briefing.generate'));
    expect(mockGenerateBriefing).toHaveBeenCalledTimes(1);
  });
});

describe('BriefingNoDataState', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders without crash', () => {
    render(<BriefingNoDataState />);
    expect(screen.getByText('briefing.noIntelligence')).toBeInTheDocument();
  });

  it('shows run analysis message', () => {
    render(<BriefingNoDataState />);
    expect(screen.getByText('briefing.runAnalysis')).toBeInTheDocument();
  });

  it('shows analyze now button', () => {
    render(<BriefingNoDataState />);
    expect(screen.getByText('results.analyzeNow')).toBeInTheDocument();
  });

  it('calls startAnalysis when button is clicked', () => {
    render(<BriefingNoDataState />);
    fireEvent.click(screen.getByText('results.analyzeNow'));
    expect(mockStartAnalysis).toHaveBeenCalledTimes(1);
  });

  it('shows keyboard shortcut hint', () => {
    render(<BriefingNoDataState />);
    expect(screen.getByText('briefing.orPress')).toBeInTheDocument();
    expect(screen.getByText('R')).toBeInTheDocument();
  });
});
