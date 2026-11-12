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
  it('renders exactly 3 tabs (AD-054)', () => {
    setMockState({});
    render(<ViewTabBar />);
    const tabs = screen.getAllByRole('tab');
    expect(tabs.length).toBe(3);
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
    const preemptionTab = screen.getByText('nav.preemption.label');
    fireEvent.click(preemptionTab);
    expect(setActiveViewMock).toHaveBeenCalledWith('preemption');
  });

  // Audit 2026-11-11: the dot meant "the list is not empty", so it was
  // permanent. It now means relevant items arrived since Signal was opened.
  it('shows no dot just because the Signal list has items', () => {
    setMockState({ activeView: 'briefing', appState: { relevanceResults: [{ id: 1 }] }, lastBackgroundResultsAt: null });
    render(<ViewTabBar />);
    expect(screen.queryByLabelText(/New activity|nav.newActivity/)).not.toBeInTheDocument();
  });

  it('shows the dot when relevant items arrive after the last look', () => {
    // Arrives after the bar mounted (the app start counts as a look).
    setMockState({ activeView: 'briefing', lastBackgroundResultsAt: new Date(Date.now() + 1000) });
    render(<ViewTabBar />);
    expect(screen.getByLabelText('nav.newActivity')).toBeInTheDocument();
  });

  it('shows no dot for items that arrived before the last look', () => {
    setMockState({ activeView: 'briefing', lastBackgroundResultsAt: new Date(Date.now() - 60_000) });
    render(<ViewTabBar />);
    expect(screen.queryByLabelText(/New activity|nav.newActivity/)).not.toBeInTheDocument();
  });

  it('shows no dot on Signal itself', () => {
    setMockState({ activeView: 'results', lastBackgroundResultsAt: new Date(Date.now() + 1000) });
    render(<ViewTabBar />);
    expect(screen.queryByLabelText(/New activity|nav.newActivity/)).not.toBeInTheDocument();
  });

  it('renders nav element with accessible label', () => {
    setMockState({});
    render(<ViewTabBar />);
    expect(screen.getByLabelText('Main views')).toBeInTheDocument();
  });
});

describe('ViewTabBar roving tabindex (WAI-ARIA tabs)', () => {
  const tabIndexes = () => screen.getAllByRole('tab').map((t) => t.getAttribute('tabindex'));

  it('puts only the selected tab in the Tab order', () => {
    setMockState({ activeView: 'preemption' });
    render(<ViewTabBar />);
    expect(tabIndexes()).toEqual(['-1', '0', '-1']);
  });

  it('falls back to the first tab when no tab is selected', () => {
    setMockState({ activeView: 'settings' });
    render(<ViewTabBar />);
    expect(tabIndexes()).toEqual(['0', '-1', '-1']);
  });

  it.each([
    ['ArrowRight', 'briefing', 'preemption', 1],
    ['ArrowLeft', 'briefing', 'results', 2],
    ['ArrowRight', 'results', 'briefing', 0],
    ['Home', 'results', 'briefing', 0],
    ['End', 'briefing', 'results', 2],
  ] as const)('%s from %s moves focus to and activates %s', (key, from, to, index) => {
    setActiveViewMock.mockClear();
    setMockState({ activeView: from });
    render(<ViewTabBar />);
    const tabs = screen.getAllByRole('tab');
    const start = tabs.find((t) => t.id === `tab-${from}`)!;
    start.focus();
    fireEvent.keyDown(start, { key });
    expect(setActiveViewMock).toHaveBeenCalledWith(to);
    expect(document.activeElement).toBe(tabs[index]);
  });

  it('ignores other keys', () => {
    setActiveViewMock.mockClear();
    setMockState({ activeView: 'briefing' });
    render(<ViewTabBar />);
    fireEvent.keyDown(screen.getAllByRole('tab')[0]!, { key: 'a' });
    expect(setActiveViewMock).not.toHaveBeenCalled();
  });
});
