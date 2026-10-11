// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * The Signal source chips list only the sources the user reads (AD-054):
 * stack sources plus the interests turned on in Settings. Turning an interest
 * on or off in Settings → Sources updates the chips with no reload.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { SourceCategoryFilter } from './SourceCategoryFilter';
import { SourceToggles } from './settings/SourceToggles';
import { ENABLED_SOURCE_IDS, getSourcesByCategory, isSourceTurnedOff, loadSourceMeta } from '../config/sources';
import { useAppStore } from '../store';
import { makeItem } from '../test/factories';
import type { SourceRelevance } from '../types';

const cmdMock = vi.fn((..._a: unknown[]): Promise<unknown> => Promise.resolve(undefined));
vi.mock('../lib/commands', () => ({ cmd: (...a: unknown[]) => cmdMock(...(a as [string, unknown])) }));

interface Row { type: string; name: string; category: string; class: 'stack' | 'interest'; enabled: boolean }

function source(r: Row) {
  return { ...r, label: r.name, color_hint: 'gray', max_items: 10, fetch_interval_secs: 60, default_content_type: 'article' };
}

/** Every interest off: the state a fresh install and the schema-126 migration leave. */
const ALL_INTERESTS_OFF: Row[] = [
  { type: 'osv', name: 'OSV', category: 'security', class: 'stack', enabled: true },
  { type: 'crates_io', name: 'crates.io', category: 'package_registry', class: 'stack', enabled: true },
  { type: 'hackernews', name: 'Hacker News', category: 'news', class: 'interest', enabled: false },
  { type: 'reddit', name: 'Reddit', category: 'community', class: 'interest', enabled: false },
];

function settingsRows(rows: Row[]) {
  return rows.map(r => ({ source_type: r.type, name: r.name, class: r.class, enabled: r.enabled, last_fetch: null }));
}

function mockBackend(rows: Row[]) {
  cmdMock.mockImplementation((command: unknown, params: unknown) => {
    if (command === 'get_sources') return Promise.resolve(rows.map(source));
    if (command === 'get_source_settings') return Promise.resolve(settingsRows(rows));
    if (command === 'set_source_enabled') {
      const p = params as { sourceType: string; enabled: boolean };
      const row = rows.find(r => r.type === p.sourceType)!;
      return Promise.resolve({ ...settingsRows([row])[0], enabled: p.enabled });
    }
    return Promise.resolve(undefined);
  });
}

/** The chips wired to the store, as ResultsView wires them. */
function Chips() {
  const sourceFilters = useAppStore(s => s.sourceFilters);
  const toggle = useAppStore(s => s.toggleSourceFilter);
  const reset = useAppStore(s => s.resetSourceFilters);
  return (
    <SourceCategoryFilter
      sourceFilters={sourceFilters}
      sourcesWithResults={new Set(['osv'])}
      onToggle={toggle}
      onReset={reset}
    />
  );
}

function item(id: number, source_type: string): SourceRelevance {
  return makeItem({ id, source_type });
}

const initialStore = useAppStore.getState();

async function boot(rows: Row[]) {
  mockBackend(rows);
  await loadSourceMeta();
  useAppStore.getState().resetSourceFilters();
}

beforeEach(() => {
  cmdMock.mockReset();
  useAppStore.setState(initialStore, true);
});

describe('source metadata', () => {
  it('offers only enabled sources, but keeps labels for every source', async () => {
    await boot(ALL_INTERESTS_OFF);
    expect([...ENABLED_SOURCE_IDS].sort()).toEqual(['crates_io', 'osv']);
    expect([...getSourcesByCategory().keys()].sort()).toEqual(['package_registry', 'security']);
    expect(isSourceTurnedOff('hackernews')).toBe(true);
    expect(isSourceTurnedOff('osv')).toBe(false);
    // Unknown to the cache: not claimed to be off (fail open).
    expect(isSourceTurnedOff('never_registered')).toBe(false);
  });
});

describe('SourceCategoryFilter', () => {
  it('lists only stack sources when every interest is off', async () => {
    await boot(ALL_INTERESTS_OFF);
    render(<Chips />);
    expect(screen.getByRole('button', { name: /Security/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Packages/ })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /News/ })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Community/ })).not.toBeInTheDocument();
    // Nothing filtered out of what the user reads: no Reset offered.
    expect(screen.queryByRole('button', { name: /Reset/ })).not.toBeInTheDocument();
  });

  it('never lists a turned-off source inside an open category', async () => {
    await boot([
      ...ALL_INTERESTS_OFF,
      { type: 'lobsters', name: 'Lobsters', category: 'community', class: 'interest', enabled: true },
    ]);
    render(<Chips />);
    fireEvent.click(screen.getByRole('button', { name: /Community/ }));
    expect(screen.getByText('Lobsters')).toBeInTheDocument();
    expect(screen.queryByText('Reddit')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Community/ })).toHaveTextContent('1/1');
  });

  it('renders no chip row at all, not an empty one, when no source is on', async () => {
    await boot(ALL_INTERESTS_OFF.map(r => ({ ...r, enabled: false })));
    const { container } = render(<Chips />);
    expect(container).toBeEmptyDOMElement();
  });

  it('follows a Settings toggle on and off without a reload', async () => {
    await boot(ALL_INTERESTS_OFF);
    render(
      <>
        <Chips />
        <SourceToggles onStatusChange={() => {}} />
      </>,
    );
    await waitFor(() => expect(screen.getAllByRole('switch')).toHaveLength(2));
    expect(screen.queryByRole('button', { name: /News/ })).not.toBeInTheDocument();

    // Turn Hacker News on: its chip appears and joins the (unfiltered) view.
    fireEvent.click(screen.getAllByRole('switch')[0]!); // Hacker News (first interest row)
    await waitFor(() => expect(screen.getByRole('button', { name: /News/ })).toBeInTheDocument());
    expect(useAppStore.getState().sourceFilters.has('hackernews')).toBe(true);
    expect(screen.getByRole('button', { name: /News/ })).toHaveTextContent('1/1');

    // And off again: the chip goes, and so does the filter entry.
    fireEvent.click(screen.getAllByRole('switch')[0]!); // Hacker News (first interest row)
    await waitFor(() => expect(screen.queryByRole('button', { name: /News/ })).not.toBeInTheDocument());
    expect(useAppStore.getState().sourceFilters.has('hackernews')).toBe(false);
  });
});

describe('applySourceEnabled', () => {
  it('drops a turned-off source from the displayed results and near misses', async () => {
    await boot([
      ...ALL_INTERESTS_OFF.slice(0, 2),
      { type: 'hackernews', name: 'Hacker News', category: 'news', class: 'interest', enabled: true },
    ]);
    act(() => {
      useAppStore.setState(s => ({
        appState: { ...s.appState, relevanceResults: [item(1, 'osv'), item(2, 'hackernews')], nearMisses: [item(3, 'hackernews')] },
      }));
    });
    act(() => { useAppStore.getState().applySourceEnabled('hackernews', false); });
    const { appState, sourceFilters } = useAppStore.getState();
    expect(appState.relevanceResults.map(r => r.id)).toEqual([1]);
    expect(appState.nearMisses).toEqual([]);
    expect(sourceFilters.has('hackernews')).toBe(false);
    expect(isSourceTurnedOff('hackernews')).toBe(true);
  });

  it('does not widen a view the user narrowed by hand when a source is turned on', async () => {
    await boot(ALL_INTERESTS_OFF);
    act(() => { useAppStore.getState().toggleSourceFilter('crates_io'); });
    expect([...useAppStore.getState().sourceFilters]).toEqual(['osv']);
    act(() => { useAppStore.getState().applySourceEnabled('hackernews', true); });
    expect([...useAppStore.getState().sourceFilters]).toEqual(['osv']);
    expect(ENABLED_SOURCE_IDS.has('hackernews')).toBe(true);
  });

  it('falls back to every enabled source when the last filtered source is turned off', async () => {
    await boot([
      ...ALL_INTERESTS_OFF.slice(0, 2),
      { type: 'hackernews', name: 'Hacker News', category: 'news', class: 'interest', enabled: true },
    ]);
    act(() => {
      useAppStore.getState().toggleSourceFilter('osv');
      useAppStore.getState().toggleSourceFilter('crates_io');
    });
    expect([...useAppStore.getState().sourceFilters]).toEqual(['hackernews']);
    act(() => { useAppStore.getState().applySourceEnabled('hackernews', false); });
    expect([...useAppStore.getState().sourceFilters].sort()).toEqual(['crates_io', 'osv']);
  });
});
