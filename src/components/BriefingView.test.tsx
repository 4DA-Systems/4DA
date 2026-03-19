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
    ...overrides,
  };
}

vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

// Mock BriefingCard to isolate BriefingView tests
vi.mock('./BriefingCard', () => ({
  BriefingCard: ({ item }: { item: { title: string } }) => (
    <div data-testid="briefing-card">{item.title}</div>
  ),
}));

// Mock SignalActionCard
vi.mock('./briefing/SignalActionCard', () => ({
  SignalActionCard: ({ item }: { item: { title: string } }) => (
    <div data-testid="signal-card">{item.title}</div>
  ),
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

  describe('content state', () => {
    const sampleContent = `## Action Required
- Update dependency X to v2.0
- Review security advisory

## Worth Knowing
- New Rust compiler feature released
- TypeScript 6.0 in beta

## Filtered Out
- Generic blog post about JavaScript basics`;

    it('renders parsed sections', () => {
      setMockState({
        aiBriefing: { content: sampleContent, loading: false, error: null, model: 'test-model', lastGenerated: new Date() },
        appState: { relevanceResults: [] },
      });
      render(<BriefingView />);
      expect(screen.getByText('Action Required')).toBeInTheDocument();
      expect(screen.getByText('Worth Knowing')).toBeInTheDocument();
      expect(screen.getByText('Filtered Out')).toBeInTheDocument();
    });

    it('renders list items within sections', () => {
      setMockState({
        aiBriefing: { content: sampleContent, loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [] },
      });
      render(<BriefingView />);
      expect(screen.getByText(/Update dependency X/)).toBeInTheDocument();
      expect(screen.getByText(/New Rust compiler/)).toBeInTheDocument();
    });

    it('shows model attribution in footer', () => {
      setMockState({
        aiBriefing: { content: '## Overview\nTest', loading: false, error: null, model: 'claude-3-5-haiku', lastGenerated: new Date() },
        appState: { relevanceResults: [] },
      });
      render(<BriefingView />);
      expect(screen.getByText('briefing.viaModel')).toBeInTheDocument();
    });

    it('shows refresh button', () => {
      const genFn = vi.fn();
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [] },
        generateBriefing: genFn,
      });
      render(<BriefingView />);
      fireEvent.click(screen.getByText('action.refresh'));
      expect(genFn).toHaveBeenCalledTimes(1);
    });

    it('renders top pick cards when high-scoring results exist', () => {
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: null },
        appState: {
          relevanceResults: [
            { id: 1, title: 'Top Item', top_score: 0.85, relevant: true },
            { id: 2, title: 'Low Item', top_score: 0.2, relevant: false },
          ],
        },
      });
      render(<BriefingView />);
      expect(screen.getByText('briefing.topPicks')).toBeInTheDocument();
      expect(screen.getByTestId('briefing-card')).toBeInTheDocument();
    });

    it('shows View All Results button', () => {
      const setView = vi.fn();
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [{ id: 1 }, { id: 2 }, { id: 3 }] },
        setActiveView: setView,
      });
      render(<BriefingView />);
      fireEvent.click(screen.getByText('briefing.viewAllResults'));
      expect(setView).toHaveBeenCalledWith('results');
    });

    it('renders signal action cards for critical items', () => {
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: null },
        appState: {
          relevanceResults: [
            { id: 1, title: 'Critical CVE', top_score: 0.9, relevant: true, signal_priority: 'critical', signal_type: 'security' },
            { id: 2, title: 'Normal Item', top_score: 0.5, relevant: true },
          ],
        },
      });
      render(<BriefingView />);
      expect(screen.getByTestId('signal-card')).toBeInTheDocument();
    });

    it('shows freshness badge when lastGenerated is set', () => {
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: new Date() },
        appState: { relevanceResults: [] },
      });
      render(<BriefingView />);
      expect(screen.getByText('Just now')).toBeInTheDocument();
    });

    it('shows stale indicator when new items arrived after briefing', () => {
      const briefingTime = new Date(Date.now() - 60000); // 1 min ago
      const bgTime = new Date(); // now
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: briefingTime },
        appState: { relevanceResults: [] },
        lastBackgroundResultsAt: bgTime,
      });
      render(<BriefingView />);
      expect(screen.getByText('briefing.staleNotice')).toBeInTheDocument();
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

    it('shows error alongside stale content when error occurs with existing briefing', () => {
      setMockState({
        aiBriefing: { content: '## Previous\n- Old content', loading: false, error: 'API key expired', model: 'test', lastGenerated: new Date() },
        appState: { relevanceResults: [] },
      });
      render(<BriefingView />);
      expect(screen.getByText('error.generic')).toBeInTheDocument();
      expect(screen.getByText('Previous')).toBeInTheDocument();
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
