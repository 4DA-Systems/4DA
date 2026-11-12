// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

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

let briefingError: string | null = null;
let resultRows: Array<{ id: number }> = [];

vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
    const mockState: Record<string, unknown> = {
      appState: {
        loading: false,
        analysisComplete: false,
        status: 'Ready',
        relevanceResults: resultRows,
        progress: 0,
        progressStage: '',
      },
      aiBriefing: {
        content: null,
        loading: false,
        error: briefingError,
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
    resultRows = [{ id: 1 }];
    render(<BriefingReadyState />);
    expect(screen.getByText('briefing.resultsAnalyzed')).toBeInTheDocument();
    resultRows = [];
  });

  // An offline first run finishes with no rows; the facts brief needs none,
  // so the screen must not say "0 results analyzed".
  it('says the brief needs no articles when the analysis found none', () => {
    render(<BriefingReadyState />);
    expect(screen.getByText('briefing.factsNeedNoArticles')).toBeInTheDocument();
    expect(screen.queryByText('briefing.resultsAnalyzed')).not.toBeInTheDocument();
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

describe('BriefingReadyState — failure recovery', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    briefingError = null;
  });

  it('shows a failed generation and lets the user try again', async () => {
    mockGenerateBriefing.mockResolvedValue(undefined);
    briefingError = 'provider returned 529';
    render(<BriefingReadyState />);

    expect(screen.getByRole('alert')).toHaveTextContent('briefing.generateFailed');
    const button = screen.getByLabelText('briefing.generateAria');
    await act(async () => {
      fireEvent.click(button);
    });
    await act(async () => {
      fireEvent.click(button);
    });
    // Not stuck disabled after the first attempt settles
    expect(mockGenerateBriefing).toHaveBeenCalledTimes(2);
  });

});

// AI briefings run on the user's own AI provider and are not Signal-gated
// (gating.rs free-floor exceptions, AD-025). This screen used to tell free
// users "AI briefings are a Signal feature. Start a free trial to try it."
// beside a Generate button that already worked.
describe('BriefingReadyState — nothing here is gated', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    briefingError = null;
  });

  it('offers one Generate button and no Signal note or trial offer', () => {
    render(<BriefingReadyState />);

    expect(screen.getAllByRole('button')).toHaveLength(1);
    expect(screen.getByRole('button')).toHaveAccessibleName('briefing.generateAria');
    expect(screen.queryByText('briefing.signalFeatureNote')).not.toBeInTheDocument();
    expect(screen.queryByText(/Signal feature/i)).not.toBeInTheDocument();
    expect(screen.queryByText('pro.startTrial')).not.toBeInTheDocument();
  });

  it('generates without consulting the licence or the trial', async () => {
    mockGenerateBriefing.mockResolvedValue(undefined);
    render(<BriefingReadyState />);

    await act(async () => {
      fireEvent.click(screen.getByRole('button'));
    });

    expect(mockGenerateBriefing).toHaveBeenCalledTimes(1);
  });
});
