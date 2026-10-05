// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Signal lanes in ResultsView: lane assignment, the Lane 1 cap and its
 * "Show all N" control, Lane 2 collapsed by default (Decision 6), the collapsed Lane 3, hidden empty lanes, the flat
 * fallback for non-score sorts and cold start, the keyboard display order,
 * deep links into collapsed lanes, and axe.
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
import { STACK_LANE_CAP, WORTH_LANE_SIZE } from '../signals/signal-lanes';

let nextId = 1;
const mk = (title: string, top: number, sb: Record<string, unknown>): SourceRelevance => ({
  id: nextId++, title, url: null, top_score: top, matches: [], relevant: true, score_breakdown: sb as never,
});
const stack = (n: number) => Array.from({ length: n }, (_, i) =>
  mk(`stack-${i}`, 0.8, { strongly_grounded: true, dependency_event: true, necessity_category: 'ecosystem_shift' }));
const news = (n: number) => Array.from({ length: n }, (_, i) => mk(`news-${i}`, 0.9 - i / 1000, { domain_relevance: 0.85 }));

function setup(results: SourceRelevance[], filters: Record<string, unknown> = {}, store: Record<string, unknown> = {}) {
  filterState = { filteredResults: results, ...filters };
  storeState = baseState(store);
  return render(<ResultsView newItemIds={new Set()} focusedIndex={-1} />);
}
const titles = (el: HTMLElement) => within(el).queryAllByTestId('row').map((r) => r.textContent);
const lane = (container: HTMLElement, key: string) => container.querySelector<HTMLElement>(`[data-lane="${key}"]`);

describe('ResultsView signal lanes', () => {
  beforeEach(() => {
    nextId = 1;
    useSignalDisplayOrder.setState({ visible: null, stackExpanded: false, worthExpanded: false, moreExpanded: false });
  });

  it('renders Lane 1 and Lane 2 as headed sections, stack items first', () => {
    useSignalDisplayOrder.setState({ worthExpanded: true });
    const results = [...news(3), ...stack(2)];
    const { container } = setup(results);
    expect(screen.getByRole('heading', { level: 3, name: 'signals.laneStack' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 3, name: 'signals.laneWorth' })).toBeInTheDocument();
    expect(titles(lane(container, 'stack')!)).toEqual(['stack-0', 'stack-1']);
    expect(titles(lane(container, 'worth')!)).toEqual(['news-0', 'news-1', 'news-2']);
    expect(screen.getByRole('listbox', { name: 'signals.laneStack' })).toBeInTheDocument();
  });

  it('collapses Lane 2 by default behind an explicit "Show N worth knowing" control (Decision 6)', () => {
    const { container } = setup([...stack(2), ...news(3)]);
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

  it(`caps Lane 1 at ${STACK_LANE_CAP} with an explicit "Show all N" control`, () => {
    const { container } = setup([...stack(STACK_LANE_CAP + 7), ...news(2)]);
    expect(titles(lane(container, 'stack')!)).toHaveLength(STACK_LANE_CAP);
    const btn = screen.getByRole('button', { name: `signals.laneShowAll:${STACK_LANE_CAP + 7}` });
    expect(btn).toHaveAttribute('aria-expanded', 'false');
    expect(btn).toHaveAttribute('aria-controls', 'signal-lane-stack-list');
    fireEvent.click(btn);
    expect(titles(lane(container, 'stack')!)).toHaveLength(STACK_LANE_CAP + 7);
    const fewer = screen.getByRole('button', { name: 'signals.laneShowFewer' });
    expect(fewer).toHaveAttribute('aria-expanded', 'true');
  });

  it('has no Lane 1 control when the lane fits under the cap', () => {
    setup([...stack(STACK_LANE_CAP), ...news(1)]);
    expect(screen.queryByRole('button', { name: /signals.laneShowAll/ })).toBeNull();
  });

  it(`shows ${WORTH_LANE_SIZE} in Lane 2 and collapses the rest behind "Show N more"`, () => {
    useSignalDisplayOrder.setState({ worthExpanded: true });
    const { container } = setup([...stack(1), ...news(WORTH_LANE_SIZE + 6)]);
    expect(titles(lane(container, 'worth')!)).toHaveLength(WORTH_LANE_SIZE);
    expect(titles(lane(container, 'more')!)).toHaveLength(0);
    const btn = screen.getByRole('button', { name: 'signals.laneShowMore:6' });
    expect(btn).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(btn);
    expect(titles(lane(container, 'more')!)).toEqual(['news-10', 'news-11', 'news-12', 'news-13', 'news-14', 'news-15']);
    expect(screen.getByRole('button', { name: 'signals.laneHideMore' })).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('listbox', { name: 'signals.laneMore' })).toBeInTheDocument();
  });

  it('hides empty lanes', () => {
    const a = setup(news(4));
    expect(lane(a.container, 'stack')).toBeNull();
    expect(lane(a.container, 'more')).toBeNull();
    expect(lane(a.container, 'worth')).not.toBeNull();
    a.unmount();
    const b = setup(stack(3));
    expect(lane(b.container, 'worth')).toBeNull();
    expect(lane(b.container, 'more')).toBeNull();
  });

  it('keeps the flat list for non-score sorts (no lane headings, incoming order)', () => {
    const results = [...news(2), ...stack(2)];
    const { container } = setup(results, { sortBy: 'freshness' });
    expect(screen.queryByRole('heading', { level: 3 })).toBeNull();
    expect(titles(container)).toEqual(['news-0', 'news-1', 'stack-0', 'stack-1']);
  });

  it('keeps the flat "fresh picks" list at cold start', () => {
    const { container } = setup([...news(2), ...stack(1)], { profileEmpty: true });
    expect(screen.queryByRole('heading', { level: 3 })).toBeNull();
    expect(screen.getByText('results.freshPicksGroup')).toBeInTheDocument();
    expect(titles(container)).toEqual(['news-0', 'news-1', 'stack-0']);
  });

  it('publishes the on-screen order for keyboard shortcuts (collapsed rows excluded)', () => {
    const a = setup([...news(WORTH_LANE_SIZE + 2), ...stack(2)]);
    expect(useSignalDisplayOrder.getState().visible!.map((r) => r.title)).toEqual(['stack-0', 'stack-1']);
    a.unmount();
    useSignalDisplayOrder.setState({ worthExpanded: true });
    setup([...news(WORTH_LANE_SIZE + 2), ...stack(2)]);
    const visible = useSignalDisplayOrder.getState().visible!.map((r) => r.title);
    expect(visible.slice(0, 3)).toEqual(['stack-0', 'stack-1', 'news-0']);
    expect(visible).toHaveLength(2 + WORTH_LANE_SIZE);
  });

  it('marks the focused row using the global on-screen index across lanes', () => {
    useSignalDisplayOrder.setState({ worthExpanded: true });
    filterState = { filteredResults: [...news(3), ...stack(2)] };
    storeState = baseState();
    render(<ResultsView newItemIds={new Set()} focusedIndex={2} />);
    expect(screen.getByText('news-0')).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('listbox', { name: 'signals.laneWorth' })).toHaveAttribute('aria-activedescendant', 'result-item-1');
  });

  it('opens a collapsed lane when a deep link targets an item inside it', () => {
    const results = [...stack(1), ...news(WORTH_LANE_SIZE + 3)];
    const target = results[results.length - 1]!;
    act(() => { setup(results, {}, { searchFocusItemId: target.id }); });
    expect(useSignalDisplayOrder.getState().moreExpanded).toBe(true);
  });

  it('opens the collapsed Lane 2 when a deep link targets an item inside it', () => {
    const results = [...stack(1), ...news(3)];
    const target = results[2]!;
    act(() => { setup(results, {}, { searchFocusItemId: target.id }); });
    expect(useSignalDisplayOrder.getState().worthExpanded).toBe(true);
  });

  it('has no axe violations with all three lanes rendered', async () => {
    useSignalDisplayOrder.setState({ worthExpanded: true, moreExpanded: true });
    const { container } = setup([...stack(STACK_LANE_CAP + 2), ...news(WORTH_LANE_SIZE + 2)]);
    expect(await axe(container)).toHaveNoViolations();
  });
});
