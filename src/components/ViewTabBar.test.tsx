// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

let mockState: Record<string, unknown> = {};
const setActiveViewMock = vi.fn();

function setMockState(overrides: Record<string, unknown>) {
  mockState = {
    activeView: 'briefing',
    appState: { relevanceResults: [] },
    decisionWindows: [],
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
  it('renders exactly 5 tabs', () => {
    setMockState({});
    render(<ViewTabBar />);
    const tabs = screen.getAllByRole('tab');
    expect(tabs.length).toBe(5);
  });

  it('marks the active view tab as selected', () => {
    setMockState({ activeView: 'results' });
    render(<ViewTabBar />);
    const resultsTab = screen.getByRole('tab', { selected: true });
    expect(resultsTab).toHaveTextContent('nav.signal.label');
  });

  it('calls setActiveView when a tab is clicked', () => {
    setMockState({});
    render(<ViewTabBar />);
    const playbookTab = screen.getByText('nav.playbook');
    fireEvent.click(playbookTab);
    expect(setActiveViewMock).toHaveBeenCalledWith('playbook');
  });

  it('shows badge indicator when results have items', () => {
    setMockState({
      activeView: 'briefing',
      appState: { relevanceResults: [{ id: 1 }] },
    });
    render(<ViewTabBar />);
    const badge = screen.getByLabelText('New activity');
    expect(badge).toBeInTheDocument();
  });

  it('does not show badge on active view', () => {
    setMockState({
      activeView: 'results',
      appState: { relevanceResults: [{ id: 1 }] },
    });
    render(<ViewTabBar />);
    expect(screen.queryByLabelText('New activity')).not.toBeInTheDocument();
  });

  it('renders nav element with accessible label', () => {
    setMockState({});
    render(<ViewTabBar />);
    expect(screen.getByLabelText('Main views')).toBeInTheDocument();
  });
});
