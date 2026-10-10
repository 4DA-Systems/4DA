// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { MAX_REFRESHES, REFRESH_DELAY_MS, useCommandSearch } from './use-command-search';

let responses: Array<{ semantic_pending: boolean; title: string }> = [];
const queried: string[] = [];

vi.mock('../../lib/commands', () => ({
  cmd: (_name: string, args: { queryText: string }) => {
    queried.push(args.queryText);
    const next = responses.shift() ?? { semantic_pending: false, title: 'final' };
    return Promise.resolve({
      items: [{ id: 1, file_path: null, file_name: next.title, preview: '', relevance: 0.9, source_type: 'hn', timestamp: null, match_reason: '', exact_match: false }],
      ghost_preview: null,
      is_pro: true,
      total_count: 1,
      semantic_pending: next.semantic_pending,
    });
  },
}));
vi.mock('../../lib/open-url', () => ({ openExternalUrl: vi.fn() }));
vi.mock('../../lib/frecency', () => ({ frecencyBoost: () => 0 }));

const deps = {
  t: (key: string, fallback?: string) => fallback ?? key,
  setActiveView: () => {},
  openPreemption: () => {},
  onAnalyze: () => {},
  onOpenSettings: () => {},
  setSearchFocusItemId: () => {},
  isItemInFeed: () => false,
};

function intelTitles(results: Array<{ group: string; title: string }>): string[] {
  return results.filter(r => r.group === 'intelligence').map(r => r.title);
}

async function flush(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  responses = [];
  queried.length = 0;
});

afterEach(() => {
  vi.useRealTimers();
});

describe('useCommandSearch provisional answers', () => {
  it('shows keyword results at once, then re-asks until the semantic half lands', async () => {
    responses = [
      { semantic_pending: true, title: 'keyword-only' },
      { semantic_pending: false, title: 'hybrid' },
    ];
    const { result } = renderHook(() => useCommandSearch(deps));
    act(() => result.current.setQuery('rusqlite'));
    await flush(200);
    expect(intelTitles(result.current.results)).toEqual(['keyword-only']);
    expect(queried).toEqual(['rusqlite']);

    await flush(REFRESH_DELAY_MS + 10);
    expect(intelTitles(result.current.results)).toEqual(['hybrid']);
    expect(queried).toEqual(['rusqlite', 'rusqlite']);

    // The final answer is cached: retyping the query does not hit the backend.
    act(() => result.current.setQuery('rusqlit'));
    act(() => result.current.setQuery('rusqlite'));
    await flush(REFRESH_DELAY_MS * 2);
    expect(queried.filter(q => q === 'rusqlite')).toHaveLength(2);
  });

  it('stops re-asking after the cap', async () => {
    responses = Array.from({ length: MAX_REFRESHES + 5 }, () => ({ semantic_pending: true, title: 'kw' }));
    const { result } = renderHook(() => useCommandSearch(deps));
    act(() => result.current.setQuery('tauri'));
    await flush(200 + (MAX_REFRESHES + 3) * (REFRESH_DELAY_MS + 10));
    expect(queried).toHaveLength(MAX_REFRESHES + 1);
  });

  it('drops a pending re-ask when the user types something else', async () => {
    responses = [{ semantic_pending: true, title: 'kw' }];
    const { result } = renderHook(() => useCommandSearch(deps));
    act(() => result.current.setQuery('tokio'));
    await flush(200);
    act(() => result.current.setQuery('axum'));
    await flush(200 + REFRESH_DELAY_MS * 2);
    expect(queried).toEqual(['tokio', 'axum']);
  });
});
