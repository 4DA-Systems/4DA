import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BriefingView } from './BriefingView';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Configurable mock state — tests override via setMockState()
let mockState: Record<string, unknown> = {};
function setMockState(overrides: Record<string, unknown>) {
  mockState = {
    aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
    appState: { relevanceResults: [] },
    generateBriefing: vi.fn(),
    recordInteraction: vi.fn(),
    feedbackGiven: {},
    setActiveView: vi.fn(),
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
    it('shows empty prompt with generate button', () => {
      setMockState({
        aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [{ id: 1, title: 'Test', top_score: 0.5 }] },
      });
      render(<BriefingView />);
      expect(screen.getByText('Your Intelligence Briefing')).toBeInTheDocument();
      expect(screen.getByText('Generate Briefing')).toBeInTheDocument();
    });

    it('disables generate button when no results', () => {
      setMockState({
        aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [] },
      });
      render(<BriefingView />);
      const button = screen.getByText('Run Analysis First');
      expect(button).toBeDisabled();
    });

    it('shows result count when results available', () => {
      setMockState({
        aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: Array.from({ length: 15 }, (_, i) => ({ id: i })) },
      });
      render(<BriefingView />);
      expect(screen.getByText(/15 results ready/)).toBeInTheDocument();
    });

    it('calls generateBriefing when button clicked', () => {
      const genFn = vi.fn();
      setMockState({
        aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [{ id: 1 }] },
        generateBriefing: genFn,
      });
      render(<BriefingView />);
      fireEvent.click(screen.getByText('Generate Briefing'));
      expect(genFn).toHaveBeenCalledTimes(1);
    });

    it('shows "view results" link when results exist', () => {
      const setView = vi.fn();
      setMockState({
        aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [{ id: 1 }, { id: 2 }] },
        setActiveView: setView,
      });
      render(<BriefingView />);
      const link = screen.getByText(/view all 2 results/i);
      fireEvent.click(link);
      expect(setView).toHaveBeenCalledWith('results');
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

    it('shows model attribution when available', () => {
      setMockState({
        aiBriefing: { content: '## Overview\nTest', loading: false, error: null, model: 'claude-3-5-haiku', lastGenerated: null },
        appState: { relevanceResults: [] },
      });
      render(<BriefingView />);
      expect(screen.getByText('via claude-3-5-haiku')).toBeInTheDocument();
    });

    it('shows refresh button', () => {
      const genFn = vi.fn();
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: null, model: null, lastGenerated: null },
        appState: { relevanceResults: [] },
        generateBriefing: genFn,
      });
      render(<BriefingView />);
      fireEvent.click(screen.getByText('Refresh'));
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
      expect(screen.getByText('Top Picks')).toBeInTheDocument();
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
      fireEvent.click(screen.getByText(/View All 3 Results/));
      expect(setView).toHaveBeenCalledWith('results');
    });
  });

  describe('error state', () => {
    it('renders error message when briefing has error', () => {
      setMockState({
        aiBriefing: { content: '## Test\nContent', loading: false, error: 'API key invalid', model: null, lastGenerated: null },
        appState: { relevanceResults: [] },
      });
      render(<BriefingView />);
      expect(screen.getByText('API key invalid')).toBeInTheDocument();
    });
  });
});
