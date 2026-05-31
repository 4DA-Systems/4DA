// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';

import { fuzzyScore } from './command-search-types';
import { buildProviders, type ProviderDeps } from './command-search-providers';
import type { CommandResult } from './command-search-types';
import { openExternalUrl } from '../../lib/open-url';

// Route `cmd` through a plain, swappable implementation rather than a vi.fn.
// Vitest 3.x's vi.fn tracks the settled result of returned promises, which
// surfaces a spurious "unhandled rejection" when a mock returns a rejected
// promise — even when the consumer awaits and catches it. A plain function
// avoids that tracking entirely; we count calls ourselves.
let cmdImpl: (name: string, args: unknown) => Promise<unknown>;
const cmdCalls: Array<[string, unknown]> = [];

vi.mock('../../lib/commands', () => ({
  cmd: (name: string, args: unknown) => {
    cmdCalls.push([name, args]);
    return cmdImpl(name, args);
  },
}));

vi.mock('../../lib/open-url', () => ({ openExternalUrl: vi.fn() }));

const t = (key: string, fallback?: string) => fallback ?? key;

function deps(overrides: Partial<ProviderDeps> = {}): ProviderDeps {
  return {
    t,
    setActiveView: vi.fn(),
    onAnalyze: vi.fn(),
    onOpenSettings: vi.fn(),
    setSearchFocusItemId: vi.fn(),
    isItemInFeed: () => false,
    ...overrides,
  };
}

function run(provider: { query: (c: { query: string; signal: AbortSignal }) => unknown }, query: string) {
  return provider.query({ query, signal: new AbortController().signal });
}

beforeEach(() => {
  cmdCalls.length = 0;
  vi.mocked(openExternalUrl).mockClear();
  cmdImpl = () => Promise.resolve({ items: [], ghost_preview: null, is_pro: true, total_count: 0 });
});

describe('fuzzyScore', () => {
  it('ranks exact > prefix > substring > no-match', () => {
    expect(fuzzyScore('signal', 'Signal')).toBe(1);
    expect(fuzzyScore('sig', 'Signal')).toBeGreaterThan(0.9);
    expect(fuzzyScore('nal', 'Signal')).toBeGreaterThan(0);
    expect(fuzzyScore('xyz', 'Signal')).toBe(-1);
  });

  it('treats an empty query as a weak match (launcher mode)', () => {
    expect(fuzzyScore('', 'anything')).toBe(0.5);
  });
});

describe('navigation provider', () => {
  it('surfaces the Signal view for "signal" and runs setActiveView', () => {
    const setActiveView = vi.fn();
    const nav = buildProviders(deps({ setActiveView })).find(p => p.id === 'nav')!;
    const out = run(nav, 'signal') as CommandResult[];
    const signalRow = out.find(r => r.id === 'goto-results');
    expect(signalRow).toBeDefined();
    signalRow!.run();
    expect(setActiveView).toHaveBeenCalledWith('results');
  });

  it('returns every view in launcher mode (empty query)', () => {
    const nav = buildProviders(deps()).find(p => p.id === 'nav')!;
    const out = run(nav, '') as CommandResult[];
    // 5 views + settings
    expect(out.length).toBeGreaterThanOrEqual(6);
  });
});

describe('intelligence provider', () => {
  it('does not query the backend for sub-threshold queries', async () => {
    const intel = buildProviders(deps()).find(p => p.id === 'intelligence')!;
    const out = await run(intel, 'a');
    expect(out).toEqual([]);
    expect(cmdCalls.length).toBe(0);
  });

  it('maps items to results and appends a ghost upsell row', async () => {
    cmdImpl = () => Promise.resolve({
      items: [
        { id: 1, file_path: null, file_name: 'tokio 1.40', preview: 'async runtime', relevance: 0.92, source_type: 'hn', timestamp: null, match_reason: 'matches async' },
      ],
      ghost_preview: { total_results: 13, hidden_results: 12, decision_count: 0, gap_count: 0, synthesis_available: false },
      is_pro: false,
      total_count: 13,
    });
    const intel = buildProviders(deps()).find(p => p.id === 'intelligence')!;
    const out = await run(intel, 'async runtime') as CommandResult[];
    expect(cmdCalls[0]).toEqual(['natural_language_query', { queryText: 'async runtime' }]);
    expect(out[0]!.title).toBe('tokio 1.40');
    expect(out[0]!.badge).toBe('0.92');
    expect(out.some(r => r.id === 'nlq-ghost')).toBe(true);
  });

  it('deep-links an in-feed pick to the Signal view', async () => {
    cmdImpl = () => Promise.resolve({
      items: [{ id: 42, file_path: null, file_name: 'X', preview: '', relevance: 0.5, source_type: 'hn', timestamp: null, match_reason: '' }],
      ghost_preview: null, is_pro: true, total_count: 1,
    });
    const setActiveView = vi.fn();
    const setSearchFocusItemId = vi.fn();
    const intel = buildProviders(deps({ setActiveView, setSearchFocusItemId, isItemInFeed: () => true }))
      .find(p => p.id === 'intelligence')!;
    const out = await run(intel, 'thing') as CommandResult[];
    out[0]!.run();
    expect(setSearchFocusItemId).toHaveBeenCalledWith(42);
    expect(setActiveView).toHaveBeenCalledWith('results');
  });

  it('opens the source URL for an off-feed pick (the inspect-bug fix)', async () => {
    cmdImpl = () => Promise.resolve({
      items: [{ id: 7, file_path: 'https://example.com/article', file_name: 'X', preview: '', relevance: 0.6, source_type: 'hn', timestamp: null, match_reason: '' }],
      ghost_preview: null, is_pro: true, total_count: 1,
    });
    const setActiveView = vi.fn();
    const setSearchFocusItemId = vi.fn();
    const intel = buildProviders(deps({ setActiveView, setSearchFocusItemId, isItemInFeed: () => false }))
      .find(p => p.id === 'intelligence')!;
    const out = await run(intel, 'thing') as CommandResult[];
    out[0]!.run();
    expect(openExternalUrl).toHaveBeenCalledWith('https://example.com/article');
    expect(setSearchFocusItemId).not.toHaveBeenCalled();
  });

  it('degrades to empty (never throws) when the backend errors', async () => {
    cmdImpl = () => {
      const p = Promise.reject(new Error('requires Signal'));
      p.catch(() => { /* pre-handled; provider also awaits + catches */ });
      return p;
    };
    const intel = buildProviders(deps()).find(p => p.id === 'intelligence')!;
    const out = await run(intel, 'anything');
    expect(out).toEqual([]);
  });
});
