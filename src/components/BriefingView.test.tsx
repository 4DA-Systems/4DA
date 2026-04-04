import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BriefingView } from './BriefingView';

// Mock Tauri API (no longer needed for pulse — now via store)
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({}),
}));

// Mock useLicense hook
vi.mock('../hooks/use-license', () => ({
  useLicense: () => ({ isPro: true, trialStatus: null }),
}));

// Mock EngagementPulse (calls invoke on mount)
vi.mock('./EngagementPulse', () => ({
  EngagementPulse: () => null,
}));

// Configurable mock state — tests override via setMockState()
let mockState: Record<string, unknown> = {};
function setMockState(overrides: Record<string, unknown>) {
  mockState = {
    aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
    appState: { relevanceResults: [], loading: false, analysisComplete: false },
    generateBriefing: vi.fn(),
    recordInteraction: vi.fn(),
    feedbackGiven: {},
    setActiveView: vi.fn(),
    lastBackgroundResultsAt: null,
    sourceHealth: [],
    addToast: vi.fn(),
    freeBriefing: null,
    freeBriefingLoading: false,
    generateFreeBriefing: vi.fn(),
    intelligencePulse: null,
    intelligencePulseLoading: false,
    loadIntelligencePulse: vi.fn(),
    decisionWindows: [],
    decisionWindowsLoading: false,
    loadDecisionWindows: vi.fn(),
    actOnWindow: vi.fn(),
    closeWindow: vi.fn(),
    compoundAdvantage: null,
    loadCompoundAdvantage: vi.fn(),
    // AWE slice
    aweSummary: null,
    awePatterns: null,
    awePendingDecisions: [],
    aweGrowthTrajectory: null,
    aweWisdomWell: null,
    aweBehavioralContext: null,
    aweWisdomSynthesis: null,
    aweLoading: false,
    aweLastSync: null,
    loadAweSummary: vi.fn(),
    loadAwePatterns: vi.fn(),
    loadAwePendingDecisions: vi.fn(),
    loadAweGrowthTrajectory: vi.fn(),
    loadAweWisdomWell: vi.fn(),
    loadBehavioralContext: vi.fn(),
    synthesizeWisdom: vi.fn(),
    submitAweBatchFeedback: vi.fn(),
    runAweAutoFeedback: vi.fn(),
    ...overrides,
  };
}

vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

describe('BriefingView', () => {
  describe('loading state', () => {
    it('renders loading skeleton', () => {
      setMockState({
        aiBriefing: { content: null, loading: true, error: null, model: null, lastGenerated: null },
      });
      const { container } = render(<BriefingView />);
      // Loading skeleton has animate-pulse elements
      const pulseElements = container.querySelectorAll('.animate-pulse');
      expect(pulseElements.length).toBeGreaterThan(0);
    });

    it('shows spinner in loading skeleton', () => {
      setMockState({
        aiBriefing: { content: null, loading: true, error: null, model: null, lastGenerated: null },
      });
      const { container } = render(<BriefingView />);
      // Spinner element with animate-spin
      const spinners = container.querySelectorAll('.animate-spin');
      expect(spinners.length).toBeGreaterThan(0);
    });
  });

  describe('empty state', () => {
    it('shows "Gathering Intelligence" when analysis is loading', () => {
      setMockState({
        aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [], loading: true, analysisComplete: false },
      });
      render(<BriefingView />);
      expect(screen.getByText('briefing.gatheringIntelligence')).toBeInTheDocument();
    });

    it('shows "Briefing Ready to Generate" when analysis done with results', () => {
      setMockState({
        aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: Array.from({ length: 15 }, (_, i) => ({ id: i })), loading: false, analysisComplete: true },
      });
      render(<BriefingView />);
      expect(screen.getByText('briefing.readyToGenerate')).toBeInTheDocument();
      expect(screen.getByText('briefing.resultsAnalyzed')).toBeInTheDocument();
    });

    it('shows warmup title when no analysis has run', () => {
      setMockState({
        aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [], loading: false, analysisComplete: false },
      });
      render(<BriefingView />);
      expect(screen.getByText('briefing.warmup.title')).toBeInTheDocument();
    });

    it('shows "Activate Intelligence" button in warmup state', () => {
      const startFn = vi.fn();
      setMockState({
        aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [], loading: false, analysisComplete: false },
        startAnalysis: startFn,
      });
      render(<BriefingView />);
      fireEvent.click(screen.getByText('briefing.warmup.activate'));
      expect(startFn).toHaveBeenCalledTimes(1);
    });
  });

  describe('content state (3-zone Intelligence Hierarchy)', () => {
    it('renders pulse summary with item counts', () => {
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: null },
        appState: {
          relevanceResults: [
            { id: 1, title: 'Item A', top_score: 0.6, relevant: true },
            { id: 2, title: 'Item B', top_score: 0.3, relevant: false },
            { id: 3, title: 'Item C', top_score: 0.7, relevant: true },
          ],
        },
      });
      render(<BriefingView />);
      // PulseSummary shows "X items analyzed, Y relevant to you."
      expect(screen.getByText(/pulse\.itemsAnalyzed/)).toBeInTheDocument();
    });

    it('renders attention cards for critical signal items', () => {
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: null },
        appState: {
          relevanceResults: [
            { id: 1, title: 'Critical CVE', top_score: 0.9, relevant: true, signal_priority: 'critical', signal_type: 'security', url: 'https://example.com' },
            { id: 2, title: 'Normal Item', top_score: 0.5, relevant: true },
          ],
        },
      });
      render(<BriefingView />);
      // AttentionCards renders the critical signal item's action or title
      expect(screen.getByText('Critical CVE')).toBeInTheDocument();
    });

    it('shows view all link when many relevant results', () => {
      const setView = vi.fn();
      // Need >15 relevant items (excluding signals) to trigger "View all" button
      const manyResults = Array.from({ length: 20 }, (_, i) => ({
        id: i + 1,
        title: `Item ${i + 1}`,
        top_score: 0.6,
        relevant: true,
        source_type: 'hackernews',
      }));
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: manyResults },
        setActiveView: setView,
      });
      render(<BriefingView />);
      // IntelligenceFeed shows "View all" button when totalRelevant > 15
      const viewAllButton = screen.getByText('briefing.viewAllResults');
      expect(viewAllButton).toBeInTheDocument();
      fireEvent.click(viewAllButton);
      expect(setView).toHaveBeenCalledWith('results');
    });

    it('renders feed items in intelligence feed', () => {
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: null },
        appState: {
          relevanceResults: [
            { id: 1, title: 'Relevant Article', top_score: 0.4, relevant: true, source_type: 'hackernews' },
            { id: 2, title: 'Another Article', top_score: 0.35, relevant: true, source_type: 'reddit' },
          ],
        },
      });
      render(<BriefingView />);
      // IntelligenceFeed renders relevant items (scores < 0.5 so not in AttentionCards topItems)
      expect(screen.getByText('Relevant Article')).toBeInTheDocument();
      expect(screen.getByText('Another Article')).toBeInTheDocument();
    });

    it('shows freshness timestamp when lastGenerated is set', () => {
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: new Date() },
        appState: {
          relevanceResults: [
            { id: 1, title: 'Item', top_score: 0.6, relevant: true, source_type: 'hackernews' },
          ],
        },
      });
      render(<BriefingView />);
      // PulseSummary shows RelativeTimestamp when lastGenerated exists
      expect(screen.getByText('Just now')).toBeInTheDocument();
    });
  });

  describe('error state', () => {
    it('renders error with retry button when briefing has error', () => {
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: 'API key invalid', model: null, lastGenerated: null },
        appState: { relevanceResults: [] },
      });
      render(<BriefingView />);
      expect(screen.getByText('error.generic')).toBeInTheDocument();
      expect(screen.getByText('action.retry')).toBeInTheDocument();
    });

    it('falls back to warmup state when error occurs with no prior content', () => {
      setMockState({
        aiBriefing: { content: null, loading: false, error: 'Network timeout', model: null, lastGenerated: null },
        appState: { relevanceResults: [], loading: false, analysisComplete: false },
      });
      render(<BriefingView />);
      // Error display is only inside the content section; with null content, warmup shows
      expect(screen.getByText('briefing.warmup.title')).toBeInTheDocument();
    });

    it('calls generateBriefing when retry button is clicked on error with existing content', () => {
      const genFn = vi.fn();
      setMockState({
        aiBriefing: { content: '## Stale\n- Old data', loading: false, error: 'LLM rate limited', model: null, lastGenerated: null },
        appState: { relevanceResults: [] },
        generateBriefing: genFn,
      });
      render(<BriefingView />);
      fireEvent.click(screen.getByText('action.retry'));
      expect(genFn).toHaveBeenCalledTimes(1);
    });

    it('shows error alongside content panel when error occurs with existing briefing', () => {
      setMockState({
        aiBriefing: { content: '## Previous\n- Old content', loading: false, error: 'API key expired', model: 'test', lastGenerated: new Date() },
        appState: { relevanceResults: [] },
      });
      render(<BriefingView />);
      // Error alert is shown alongside the 3-zone content panel
      expect(screen.getByText('error.generic')).toBeInTheDocument();
      // PulseSummary renders (even with no results it shows noData message)
      expect(screen.getByText('pulse.noData')).toBeInTheDocument();
    });

    it('calls generateBriefing on retry and transitions to loading state', () => {
      const genFn = vi.fn();
      setMockState({
        aiBriefing: { content: '## Stale\n- Data', loading: false, error: 'LLM service unavailable', model: null, lastGenerated: null },
        appState: { relevanceResults: [] },
        generateBriefing: genFn,
      });
      const { unmount } = render(<BriefingView />);

      // Error is visible with retry button
      expect(screen.getByText('error.generic')).toBeInTheDocument();
      expect(screen.getByText('action.retry')).toBeInTheDocument();

      // Click retry
      fireEvent.click(screen.getByText('action.retry'));
      expect(genFn).toHaveBeenCalledTimes(1);

      unmount();

      // Re-render with loading state (simulating store update after retry)
      setMockState({
        aiBriefing: { content: null, loading: true, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [] },
        generateBriefing: genFn,
      });
      const { container } = render(<BriefingView />);

      // Error should be gone, loading skeleton should show
      expect(screen.queryByText('error.generic')).not.toBeInTheDocument();
      expect(screen.queryByText('action.retry')).not.toBeInTheDocument();
      const pulseElements = container.querySelectorAll('.animate-pulse');
      expect(pulseElements.length).toBeGreaterThan(0);
    });

    it('error alert has role="alert" for screen reader announcement', () => {
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: 'Timeout after 30s', model: null, lastGenerated: null },
        appState: { relevanceResults: [] },
      });
      render(<BriefingView />);
      const alert = screen.getByRole('alert');
      expect(alert).toBeInTheDocument();
    });
  });
});
