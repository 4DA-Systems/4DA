// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * The Signal tab's composition (AD-054): "Your stack" always; the reading
 * feed (Key Signals, Analysis Results, Themes and Graph) only when an
 * interest is on or reading rows are already in the list; otherwise one line
 * that says reading is off and opens Settings > Sources.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import type { SourceRelevance } from '../../types';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'en' } }),
}));
vi.mock('zustand/react/shallow', () => ({ useShallow: (fn: unknown) => fn }));

let storeState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(storeState)),
    { getState: () => storeState },
  ),
}));

let interestsCount: number | null = 0;
vi.mock('../../config/sources', () => ({
  interestsEnabledCount: () => interestsCount,
  isSourceTurnedOff: () => false,
}));

vi.mock('../ViewErrorBoundary', () => ({
  ViewErrorBoundary: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));
vi.mock('../signals/StackChangeLane', () => ({ StackChangeLane: () => <section data-testid="stack-lane" /> }));
vi.mock('../ResultsView', () => ({ ResultsView: () => <section data-testid="results-view" /> }));
vi.mock('../SignalsPanel', () => ({ SignalsPanel: () => <div data-testid="signals-panel" /> }));
vi.mock('../FeedbackLivenessBanner', () => ({ FeedbackLivenessBanner: () => null }));
vi.mock('../signals/ContentGraphView', () => ({ default: () => <div data-testid="graph-view" /> }));
vi.mock('../signals/ThemeMapView', () => ({ default: () => <div data-testid="themes-view" /> }));

import { ViewRouter } from '../ViewRouter';

const row = (id: number, source_type: string): SourceRelevance => ({
  id, title: `r${id}`, url: null, top_score: 0.8, matches: [], relevant: true, source_type,
});
const stackRows = [row(1, 'crates_io'), row(2, 'osv'), row(3, 'npm_registry')];

function setup(relevanceResults: SourceRelevance[], extra: Record<string, unknown> = {}) {
  storeState = {
    activeView: 'results',
    appState: { analysisComplete: true, relevanceResults },
    signalViewMode: 'list',
    setSignalViewMode: vi.fn(),
    sourceFilters: new Set<string>(),
    setShowSettings: vi.fn(),
    setSettingsInitialTab: vi.fn(),
    ...extra,
  };
  return render(<ViewRouter newItemIds={new Set()} focusedIndex={-1} />);
}

describe('Signal tab with every interest off', () => {
  beforeEach(() => { interestsCount = 0; });

  it('shows Your stack and one line, not an empty Analysis Results panel or duplicate advisory cards', async () => {
    setup(stackRows);
    expect(await screen.findByTestId('stack-lane')).toBeInTheDocument();
    expect(screen.getByTestId('interests-off-note')).toHaveTextContent('signals.interestsOff');
    expect(screen.queryByTestId('results-view')).toBeNull();
    expect(screen.queryByTestId('signals-panel')).toBeNull();
  });

  it('offers no Themes / Graph toggle and shows List even when Graph was last chosen', async () => {
    setup(stackRows, { signalViewMode: 'graph' });
    expect(await screen.findByTestId('stack-lane')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'signals.viewThemes' })).toBeNull();
    expect(screen.queryByTestId('signal-mode-note')).toBeNull();
    expect(screen.queryByTestId('graph-view')).toBeNull();
  });

  it('opens Settings on the Sources tab', async () => {
    setup(stackRows);
    fireEvent.click(await screen.findByRole('button', { name: 'signals.interestsOffAction' }));
    expect(storeState.setSettingsInitialTab).toHaveBeenCalledWith('sources');
    expect(storeState.setShowSettings).toHaveBeenCalledWith(true);
  });
});

describe('Signal tab with an interest on', () => {
  beforeEach(() => { interestsCount = 2; });

  it('shows the reading feed below Your stack, and the view toggle', async () => {
    setup([...stackRows, row(9, 'hackernews')]);
    expect(await screen.findByTestId('results-view')).toBeInTheDocument();
    expect(screen.getByTestId('stack-lane')).toBeInTheDocument();
    expect(screen.queryByTestId('interests-off-note')).toBeNull();
    expect(screen.getByRole('button', { name: 'signals.viewThemes' })).toBeInTheDocument();
  });

  it('shows the reading feed before anything is read, so its own empty state can speak', async () => {
    setup(stackRows);
    expect(await screen.findByTestId('results-view')).toBeInTheDocument();
  });
});

describe('Signal tab before source metadata loads', () => {
  beforeEach(() => { interestsCount = null; });

  it('says nothing about interests while it does not know', async () => {
    setup(stackRows);
    expect(await screen.findByTestId('stack-lane')).toBeInTheDocument();
    expect(screen.queryByTestId('interests-off-note')).toBeNull();
    expect(screen.queryByTestId('results-view')).toBeNull();
  });

  it('never hides reading rows already in the list', async () => {
    setup([row(9, 'reddit')]);
    expect(await screen.findByTestId('results-view')).toBeInTheDocument();
  });
});
