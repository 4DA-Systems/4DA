// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * The reading feed's lanes in ResultsView (Lane 1, the stack-change stream,
 * is StackChangeLane — see signals/StackChangeLane.test.tsx): registry and
 * advisory rows left out, Lane 2 collapsed by default (Decision 6), the
 * collapsed Lane 3, hidden empty lanes, the flat fallback for non-score sorts
 * and cold start, the keyboard display order, deep links into collapsed
 * lanes, and axe.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, within, act } from '@testing-library/react';
import { axe, toHaveNoViolations } from 'jest-axe';
import type { SourceRelevance } from '../../types';

expect.extend(toHaveNoViolations);

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.resolve({})) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})), emit: vi.fn() }));

// Interpolating t() so counts on controls are asserted, not assumed.
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: unknown) =>
      opts && typeof opts === 'object' && 'count' in opts ? `${key}:${(opts as { count: number }).count}` : key,
    i18n: { language: 'en', changeLanguage: vi.fn() },
  }),
}));

let storeState: Record<string, unknown> = {};
function baseState(overrides: Record<string, unknown> = {}) {
  return {
    appState: {
      loading: false, analysisComplete: true, status: 'Ready', relevanceResults: [] as SourceRelevance[],
      progress: 0, progressStage: '', progressMessage: '', contextFiles: [], nearMisses: null,
    },
    feedbackGiven: {}, discoveredContext: null, expandedItem: null, searchFocusItemId: null,
    setExpandedItem: vi.fn(), setSearchFocusItemId: vi.fn(), startAnalysis: vi.fn(),
    loadContextFiles: vi.fn(), clearContext: vi.fn(), indexContext: vi.fn(), recordInteraction: vi.fn(),
    ...overrides,
  };
}
vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(storeState)),
    { getState: () => storeState },
  ),
}));
vi.mock('zustand/react/shallow', () => ({ useShallow: (fn: unknown) => fn }));

// Virtualizer that renders every row, so lane contents are observable.
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: (opts: { count: number }) => ({
    getVirtualItems: () => Array.from({ length: opts.count }, (_, i) => ({ index: i, start: i * 120, key: i })),
    getTotalSize: () => opts.count * 120,
    measureElement: vi.fn(),
    scrollToIndex: vi.fn(),
  }),
}));

vi.mock('../ResultItem', () => ({
  ResultItem: ({ item, isFocused }: { item: SourceRelevance; isFocused?: boolean }) => (
    <div id={`result-item-${item.id}`} role="option" aria-selected={!!isFocused} data-testid="row">{item.title}</div>
  ),
}));
vi.mock('../context-panel', () => ({ ContextPanel: () => <div /> }));
vi.mock('../search/ResultFiltersBar', () => ({ ResultFiltersBar: () => <div /> }));
vi.mock('../ContentTranslationProvider', () => ({
  useTranslatedContent: () => ({ getTranslated: (_: string, t: string) => t, requestTranslation: vi.fn() }),
}));

let filterState: Record<string, unknown> = {};
// Stable function identities, like the real store's setters (effects depend on them).
const stableFilterFns = {
  setSearchQuery: vi.fn(), toggleSourceFilter: vi.fn(), resetSourceFilters: vi.fn(), setSortBy: vi.fn(),
  setShowOnlyRelevant: vi.fn(), setShowSavedOnly: vi.fn(), dismissAllBelow: vi.fn(), saveAllAbove: vi.fn(),
};
const emptySources = new Set<string>();
vi.mock('../../hooks', () => ({
  useResultFilters: () => ({
    filteredResults: [], searchQuery: '', sourceFilters: emptySources, sortBy: 'score',
    showOnlyRelevant: true, showSavedOnly: false, profileEmpty: false,
    ...stableFilterFns,
    ...filterState,
  }),
}));

import { ResultsView } from '../ResultsView';
import { useSignalDisplayOrder } from '../signals/signal-display-order';
import { WORTH_LANE_SIZE } from '../signals/signal-lanes';

let nextId = 1;
const mk = (title: string, top: number, sb: Record<string, unknown>, source_type = 'hackernews'): SourceRelevance => ({
  id: nextId++, title, url: null, top_score: top, matches: [], relevant: true, source_type, score_breakdown: sb as never,
});
// Registry release rows: Lane 1 (the stack-change stream) states these facts.
const releases = (n: number) => Array.from({ length: n }, (_, i) =>
  mk(`release-${i}`, 0.95, { strongly_grounded: true, dependency_event: true }, 'crates_io'));
const news = (n: number) => Array.from({ length: n }, (_, i) => mk(`news-${i}`, 0.9 - i / 1000, { domain_relevance: 0.85 }));

function setup(results: SourceRelevance[], filters: Record<string, unknown> = {}, store: Record<string, unknown> = {}) {
  filterState = { filteredResults: results, ...filters };
  storeState = baseState(store);
  return render(<ResultsView newItemIds={new Set()} focusedIndex={-1} />);
}
const titles = (el: HTMLElement) => within(el).queryAllByTestId('row').map((r) => r.textContent);
const lane = (container: HTMLElement, key: string) => container.querySelector<HTMLElement>(`[data-lane="${key}"]`);

describe('ResultsView reading-feed lanes', () => {
  beforeEach(() => {
    nextId = 1;
    useSignalDisplayOrder.setState({ visible: null, worthExpanded: false, moreExpanded: false });
  });

  it('never renders a feed-sliced "Your stack" lane, and leaves registry rows out of the lanes', () => {
    useSignalDisplayOrder.setState({ worthExpanded: true });
    const { container } = setup([...releases(2), ...news(3)]);
    expect(lane(container, 'stack')).toBeNull();
    expect(screen.queryByRole('heading', { name: 'signals.laneStack' })).toBeNull();
    expect(titles(lane(container, 'worth')!)).toEqual(['news-0', 'news-1', 'news-2']);
  });

  it('collapses Lane 2 by default behind an explicit "Show N worth knowing" control (Decision 6)', () => {
    const { container } = setup([...releases(2), ...news(3)]);
    expect(titles(lane(container, 'worth')!)).toEqual([]);
    expect(screen.getByRole('heading', { level: 3, name: 'signals.laneWorth' })).toBeInTheDocument();
    const btn = screen.getByRole('button', { name: 'signals.laneShowWorth:3' });
    expect(btn).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(btn);
    expect(titles(lane(container, 'worth')!)).toEqual(['news-0', 'news-1', 'news-2']);
    const hide = screen.getByRole('button', { name: 'signals.laneHideWorth' });
    expect(hide).toHaveAttribute('aria-expanded', 'true');
    expect(hide).toHaveAttribute('aria-controls', 'signal-lane-worth-list');
  });

  it(`shows ${WORTH_LANE_SIZE} in Lane 2 and collapses the rest behind "Show N more"`, () => {
    useSignalDisplayOrder.setState({ worthExpanded: true });
    const { container } = setup([...releases(1), ...news(WORTH_LANE_SIZE + 6)]);
    expect(titles(lane(container, 'worth')!)).toHaveLength(WORTH_LANE_SIZE);
    expect(titles(lane(container, 'more')!)).toHaveLength(0);
    const btn = screen.getByRole('button', { name: 'signals.laneShowMore:6' });
    expect(btn).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(btn);
    expect(titles(lane(container, 'more')!)).toEqual(['news-10', 'news-11', 'news-12', 'news-13', 'news-14', 'news-15']);
    expect(screen.getByRole('button', { name: 'signals.laneHideMore' })).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('listbox', { name: 'signals.laneMore' })).toBeInTheDocument();
  });

  it('hides empty lanes, and says so plainly when the feed holds only stack facts', () => {
    const a = setup(news(4));
    expect(lane(a.container, 'more')).toBeNull();
    expect(lane(a.container, 'worth')).not.toBeNull();
    a.unmount();
    const b = setup(releases(3));
    expect(lane(b.container, 'worth')).toBeNull();
    expect(lane(b.container, 'more')).toBeNull();
    expect(screen.getByText('signals.stack.feedOnlyStack')).toBeInTheDocument();
  });

  it('keeps the flat list for non-score sorts (no lane headings, incoming order)', () => {
    const results = [...news(2), ...releases(2)];
    const { container } = setup(results, { sortBy: 'freshness' });
    expect(screen.queryByRole('heading', { level: 3 })).toBeNull();
    expect(titles(container)).toEqual(['news-0', 'news-1', 'release-0', 'release-1']);
  });

  it('keeps the flat "fresh picks" list at cold start', () => {
    const { container } = setup([...news(2), ...releases(1)], { profileEmpty: true });
    expect(screen.queryByRole('heading', { level: 3 })).toBeNull();
    expect(screen.getByText('results.freshPicksGroup')).toBeInTheDocument();
    expect(titles(container)).toEqual(['news-0', 'news-1', 'release-0']);
  });

  it('publishes the on-screen order for keyboard shortcuts (collapsed rows excluded)', () => {
    const a = setup([...news(WORTH_LANE_SIZE + 2), ...releases(2)]);
    expect(useSignalDisplayOrder.getState().visible).toEqual([]);
    a.unmount();
    useSignalDisplayOrder.setState({ worthExpanded: true });
    setup([...news(WORTH_LANE_SIZE + 2), ...releases(2)]);
    const visible = useSignalDisplayOrder.getState().visible!.map((r) => r.title);
    expect(visible[0]).toBe('news-0');
    expect(visible).toHaveLength(WORTH_LANE_SIZE);
  });

  it('marks the focused row using the global on-screen index across lanes', () => {
    useSignalDisplayOrder.setState({ worthExpanded: true });
    filterState = { filteredResults: [...news(3), ...releases(2)] };
    storeState = baseState();
    render(<ResultsView newItemIds={new Set()} focusedIndex={0} />);
    expect(screen.getByText('news-0')).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('listbox', { name: 'signals.laneWorth' })).toHaveAttribute('aria-activedescendant', 'result-item-1');
  });

  it('opens a collapsed lane when a deep link targets an item inside it', () => {
    const results = [...releases(1), ...news(WORTH_LANE_SIZE + 3)];
    const target = results[results.length - 1]!;
    act(() => { setup(results, {}, { searchFocusItemId: target.id }); });
    expect(useSignalDisplayOrder.getState().moreExpanded).toBe(true);
  });

  it('opens the collapsed Lane 2 when a deep link targets an item inside it', () => {
    const results = [...releases(1), ...news(3)];
    const target = results[2]!;
    act(() => { setup(results, {}, { searchFocusItemId: target.id }); });
    expect(useSignalDisplayOrder.getState().worthExpanded).toBe(true);
  });

  it('has no axe violations with both lanes rendered', async () => {
    useSignalDisplayOrder.setState({ worthExpanded: true, moreExpanded: true });
    const { container } = setup([...releases(3), ...news(WORTH_LANE_SIZE + 2)]);
    expect(await axe(container)).toHaveNoViolations();
  });
});
