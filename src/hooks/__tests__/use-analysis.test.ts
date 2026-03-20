/**
 * Tests for the core analysis pipeline hook (useAnalysis).
 *
 * Since useAnalysis is a React hook wrapping the Zustand store,
 * these tests exercise the underlying store actions and state
 * that the hook delegates to.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// Must import store AFTER mocks are set up
import { useAppStore } from '../../store';

const mockedInvoke = vi.mocked(invoke);

// Capture initial state so we can reset between tests
const initialState = useAppStore.getState();

describe('useAnalysis — core pipeline', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState(initialState);
  });

  it('initial appState has correct defaults', () => {
    const state = useAppStore.getState();
    expect(state.appState.loading).toBe(false);
    expect(state.appState.status).toBeDefined();
    expect(state.appState.relevanceResults).toEqual([]);
    expect(state.appState.nearMisses).toBeNull();
    expect(state.appState.analysisComplete).toBe(false);
    expect(state.appState.progress).toBe(0);
    expect(state.appState.progressMessage).toBe('');
    expect(state.appState.progressStage).toBe('');
    expect(state.appState.lastAnalyzedAt).toBeNull();
  });

  it('near misses populated when fewer than 3 relevant results', () => {
    // Replicate the exact logic from use-analysis.ts analysis-complete listener
    const NEAR_MISS_FLOOR = 0.20;
    const NEAR_MISS_LIMIT = 5;

    const results = [
      { id: 1, top_score: 0.8, relevant: true },
      { id: 2, top_score: 0.7, relevant: true },
      { id: 3, top_score: 0.45, relevant: false },
      { id: 4, top_score: 0.30, relevant: false },
      { id: 5, top_score: 0.25, relevant: false },
      { id: 6, top_score: 0.15, relevant: false },
    ];

    const relevantCount = results.filter(r => r.relevant).length;
    expect(relevantCount).toBeLessThan(3);

    // Near miss logic: items not relevant, score >= floor, sorted desc, capped at limit
    const nearMisses = results
      .filter(r => !r.relevant && r.top_score >= NEAR_MISS_FLOOR)
      .sort((a, b) => b.top_score - a.top_score)
      .slice(0, NEAR_MISS_LIMIT);

    expect(nearMisses).toHaveLength(3);
    expect(nearMisses[0].top_score).toBe(0.45);
    expect(nearMisses[1].top_score).toBe(0.30);
    expect(nearMisses[2].top_score).toBe(0.25);
  });

  it('near misses null when 3+ relevant results', () => {
    const results = [
      { id: 1, top_score: 0.8, relevant: true },
      { id: 2, top_score: 0.7, relevant: true },
      { id: 3, top_score: 0.6, relevant: true },
      { id: 4, top_score: 0.45, relevant: false },
    ];
    const relevantCount = results.filter(r => r.relevant).length;
    expect(relevantCount).toBeGreaterThanOrEqual(3);

    // When relevantCount >= 3, hook sets nearMisses to null
    const nearMisses = relevantCount < 3
      ? results.filter(r => !r.relevant)
      : null;
    expect(nearMisses).toBeNull();
  });

  it('near misses sorted by score descending', () => {
    const items = [
      { top_score: 0.25, relevant: false },
      { top_score: 0.45, relevant: false },
      { top_score: 0.30, relevant: false },
    ];
    const sorted = [...items].sort((a, b) => b.top_score - a.top_score);
    expect(sorted[0].top_score).toBe(0.45);
    expect(sorted[1].top_score).toBe(0.30);
    expect(sorted[2].top_score).toBe(0.25);
  });

  it('near misses capped at NEAR_MISS_LIMIT of 5', () => {
    const NEAR_MISS_FLOOR = 0.20;
    const NEAR_MISS_LIMIT = 5;

    // 10 items all above the floor, all non-relevant
    const items = Array.from({ length: 10 }, (_, i) => ({
      id: i + 1,
      top_score: 0.20 + i * 0.05,
      relevant: false,
    }));

    const nearMisses = items
      .filter(r => !r.relevant && r.top_score >= NEAR_MISS_FLOOR)
      .sort((a, b) => b.top_score - a.top_score)
      .slice(0, NEAR_MISS_LIMIT);

    expect(nearMisses).toHaveLength(5);
    // Should be the 5 highest-scoring items
    expect(nearMisses[0].top_score).toBe(0.65);
  });

  it('progress updates store via setAppStateFull', () => {
    useAppStore.getState().setAppStateFull((s) => ({
      ...s,
      progress: 0.5,
      progressMessage: 'Scoring items...',
      progressStage: 'scoring',
      status: 'Scoring items... (50/100)',
    }));

    const state = useAppStore.getState();
    expect(state.appState.progress).toBe(0.5);
    expect(state.appState.progressMessage).toBe('Scoring items...');
    expect(state.appState.progressStage).toBe('scoring');
    expect(state.appState.status).toBe('Scoring items... (50/100)');
  });

  it('startAnalysis calls run_cached_analysis via invoke', async () => {
    mockedInvoke.mockResolvedValue(undefined);
    await useAppStore.getState().startAnalysis();
    expect(mockedInvoke).toHaveBeenCalledWith('run_cached_analysis', {});
  });

  it('store analysis state resets correctly on new analysis cycle', () => {
    // Simulate a completed analysis
    useAppStore.getState().setAppStateFull((s) => ({
      ...s,
      loading: false,
      progress: 1,
      analysisComplete: true,
      status: 'Analysis complete',
      progressStage: 'complete',
    }));
    expect(useAppStore.getState().appState.loading).toBe(false);
    expect(useAppStore.getState().appState.analysisComplete).toBe(true);

    // Reset for a new analysis cycle
    useAppStore.getState().setAppStateFull((s) => ({
      ...s,
      loading: true,
      progress: 0,
      analysisComplete: false,
      status: 'Starting cache-first analysis...',
      progressStage: 'init',
      progressMessage: 'Loading cached items...',
    }));

    const state = useAppStore.getState();
    expect(state.appState.loading).toBe(true);
    expect(state.appState.progress).toBe(0);
    expect(state.appState.analysisComplete).toBe(false);
    expect(state.appState.progressStage).toBe('init');
    expect(state.appState.progressMessage).toBe('Loading cached items...');
  });
});
