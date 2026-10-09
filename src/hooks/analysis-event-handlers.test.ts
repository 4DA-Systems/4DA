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

function row(id: number, relevant: boolean, excluded?: boolean): SourceRelevance {
  return { id, relevant, excluded, top_score: relevant ? 0.8 : 0.1 } as unknown as SourceRelevance;
}

afterEach(() => vi.restoreAllMocks());

describe('handleAnalysisComplete — relevant count', () => {
  it('counts only surfaced rows, not exclusion-demoted ones', () => {
    const t = vi.spyOn(i18n, 't').mockImplementation(((key: string) => key) as typeof i18n.t);
    const addToast = vi.fn();
    vi.spyOn(useAppStore, 'getState').mockReturnValue({
      ...useAppStore.getState(),
      addToast,
      setAppStateFull: vi.fn(),
    });
    const results = [row(1, true), row(2, true), row(3, true, true), row(4, false)];

    handleAnalysisComplete({ payload: results } as Event<SourceRelevance[]>);

    expect(t).toHaveBeenCalledWith('analysis.complete', { count: 2 });
    expect(addToast).toHaveBeenCalledTimes(1);
  });
});
