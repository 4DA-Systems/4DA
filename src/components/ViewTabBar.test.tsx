import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

// Configurable mock state
let mockState: Record<string, unknown> = {};
const setActiveViewMock = vi.fn();

function setMockState(overrides: Record<string, unknown>) {
  mockState = {
    activeView: 'briefing',
    appState: { relevanceResults: [] },
    decisionWindows: [],
    unifiedProfile: null,
    channels: [],
    viewTier: 'power' as const,
    showAllViews: true,
    feedbackGiven: {},
    setActiveView: setActiveViewMock,
    ...overrides,
  };
}

vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

vi.mock('zustand/react/shallow', () => ({
  useShallow: vi.fn((fn: unknown) => fn),
}));

vi.mock('../hooks/use-telemetry', () => ({
  trackEvent: vi.fn(),
}));

import { ViewTabBar } from './ViewTabBar';

describe('ViewTabBar', () => {
  it('renders all tab buttons when showAllViews is true and tier is power', () => {
    setMockState({});
    render(<ViewTabBar />);
    expect(screen.getByRole('tablist')).toBeInTheDocument();
    // Tabs: briefing, chapters, results, playbook, channels, insights, saved, profile, console, toolkit, calibrate
    const tabs = screen.getAllByRole('tab');
    expect(tabs.length).toBe(11);
  });

  it('marks the active view tab as selected', () => {
    setMockState({ activeView: 'results' });
    render(<ViewTabBar />);
    const resultsTab = screen.getByRole('tab', { selected: true });
    expect(resultsTab).toHaveTextContent('nav.results');
  });

  it('calls setActiveView when a tab is clicked', () => {
    setMockState({});
    render(<ViewTabBar />);

    const channelsTab = screen.getByText('nav.channels');
    fireEvent.click(channelsTab);
    expect(setActiveViewMock).toHaveBeenCalledWith('channels');
  });

  it('shows only core tabs when viewTier is core and showAllViews is false', () => {
    setMockState({ viewTier: 'core', showAllViews: false });
    render(<ViewTabBar />);
    const tabs = screen.getAllByRole('tab');
    // core tier: briefing, chapters, results, playbook
    expect(tabs.length).toBe(4);
  });

  it('shows explorer tabs when viewTier is explorer and showAllViews is false', () => {
    setMockState({ viewTier: 'explorer', showAllViews: false });
    render(<ViewTabBar />);
    const tabs = screen.getAllByRole('tab');
    // explorer tier: briefing, chapters, results, playbook, channels, insights
    expect(tabs.length).toBe(6);
  });

  it('shows badge indicator when results have items', () => {
    setMockState({
      activeView: 'channels', // badge only shows when NOT on that view
      appState: { relevanceResults: [{ id: 1 }] },
    });
    render(<ViewTabBar />);
    // results tab should have a badge (dot indicator)
    const badge = screen.getByLabelText('New activity');
    expect(badge).toBeInTheDocument();
  });

  it('does not show badge on active view', () => {
    setMockState({
      activeView: 'results',
      appState: { relevanceResults: [{ id: 1 }] },
    });
    render(<ViewTabBar />);
    // Badge should NOT show on the currently active tab
    expect(screen.queryByLabelText('New activity')).not.toBeInTheDocument();
  });

  it('renders nav element with accessible label', () => {
    setMockState({});
    render(<ViewTabBar />);
    expect(screen.getByLabelText('Main views')).toBeInTheDocument();
  });
});
