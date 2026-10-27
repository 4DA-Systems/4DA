// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Live 2026-10-10: the "Analysis complete" toast said 303 relevant above a
// header chip that said 280 — the toast counted exclusion-demoted rows the
// list never shows. Every count of "relevant" must match `isSurfacedSignal`.

import type { Event } from '@tauri-apps/api/event';
import i18n from 'i18next';
import { describe, it, expect, vi, afterEach } from 'vitest';

import type { SourceRelevance } from '../types';
import { useAppStore } from '../store';
import { handleAnalysisComplete } from './analysis-event-handlers';

vi.mock('../lib/commands', () => ({ cmd: vi.fn(() => Promise.resolve()) }));

function row(id: number, relevant: boolean, excluded?: boolean, source_type = 'hackernews'): SourceRelevance {
  return { id, relevant, excluded, source_type, top_score: relevant ? 0.8 : 0.1 } as unknown as SourceRelevance;
}

afterEach(() => vi.restoreAllMocks());

function spyStore() {
  const t = vi.spyOn(i18n, 't').mockImplementation(((key: string) => key) as typeof i18n.t);
  const addToast = vi.fn();
  vi.spyOn(useAppStore, 'getState').mockReturnValue({
    ...useAppStore.getState(),
    addToast,
    setAppStateFull: vi.fn(),
  });
  return { t, addToast };
}

describe('handleAnalysisComplete — relevant count', () => {
  it('counts only surfaced rows, not exclusion-demoted ones', () => {
    const { t, addToast } = spyStore();
    const results = [row(1, true), row(2, true), row(3, true, true), row(4, false)];

    handleAnalysisComplete({ payload: results } as Event<SourceRelevance[]>);

    expect(t).toHaveBeenCalledWith('analysis.complete', { count: 2 });
    expect(addToast).toHaveBeenCalledTimes(1);
  });

  // Live 2026-10-27, every interest off: "Analysis complete: 38 relevant
  // items found" over a feed that lists none of them — all 38 were registry
  // releases and advisories, the "Your stack" lane's facts.
  it('counts reading rows only, and stays quiet when there are none', () => {
    const { t, addToast } = spyStore();
    const results = [row(1, true, false, 'crates_io'), row(2, true, false, 'osv'), row(3, true, false, 'cve')];

    handleAnalysisComplete({ payload: results } as Event<SourceRelevance[]>);

    expect(t).not.toHaveBeenCalledWith('analysis.complete', expect.anything());
    expect(addToast).not.toHaveBeenCalled();
  });

  it('leaves stack facts out of a mixed run', () => {
    const { t } = spyStore();
    const results = [row(1, true, false, 'npm_registry'), row(2, true), row(3, true, false, 'reddit')];

    handleAnalysisComplete({ payload: results } as Event<SourceRelevance[]>);

    expect(t).toHaveBeenCalledWith('analysis.complete', { count: 2 });
  });
});
