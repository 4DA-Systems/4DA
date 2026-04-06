/**
 * Tests for useAppBootstrap store integration.
 *
 * Since useAppBootstrap is a React hook that composes many sub-hooks,
 * we test the Zustand store state it reads and writes rather than
 * rendering the hook directly.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { invoke as _invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock('../../i18n/rtl', () => ({
  useDirection: () => 'ltr',
}));

// Must import store AFTER mocks are set up
import { useAppStore } from '../../store';

const initialState = useAppStore.getState();

describe('useAppBootstrap store integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState(initialState);
  });

  it('initial store state has expected defaults', () => {
    const state = useAppStore.getState();
    expect(state.showSettings).toBe(false);
    expect(state.activeView).toBe('briefing');
    expect(state.showOnlyRelevant).toBe(true);
    expect(state.isFirstRun).toBe(false);
    expect(state.firstRunDismissed).toBe(false);
    expect(state.showSplash).toBe(true);
  });

  it('setShowSettings toggles settings visibility', () => {
    useAppStore.getState().setShowSettings(true);
    expect(useAppStore.getState().showSettings).toBe(true);

    useAppStore.getState().setShowSettings(false);
    expect(useAppStore.getState().showSettings).toBe(false);
  });

  it('isFirstRun flag can be toggled via setIsFirstRun', () => {
    const state = useAppStore.getState();
    expect(typeof state.isFirstRun).toBe('boolean');

    state.setIsFirstRun(true);
    expect(useAppStore.getState().isFirstRun).toBe(true);

    useAppStore.getState().setIsFirstRun(false);
    expect(useAppStore.getState().isFirstRun).toBe(false);
  });

  it('setActiveView updates active view', () => {
    useAppStore.getState().setActiveView('results');
    expect(useAppStore.getState().activeView).toBe('results');

    useAppStore.getState().setActiveView('briefing');
    expect(useAppStore.getState().activeView).toBe('briefing');

    // 'saved' requires invested tier or higher
    useAppStore.setState({ viewTier: 'invested' });
    useAppStore.getState().setActiveView('saved');
    expect(useAppStore.getState().activeView).toBe('saved');
  });

  it('showOnlyRelevant toggles filter state', () => {
    const initial = useAppStore.getState().showOnlyRelevant;
    useAppStore.getState().setShowOnlyRelevant(!initial);
    expect(useAppStore.getState().showOnlyRelevant).toBe(!initial);

    useAppStore.getState().setShowOnlyRelevant(initial);
    expect(useAppStore.getState().showOnlyRelevant).toBe(initial);
  });

  it('analysis results start as empty array', () => {
    const state = useAppStore.getState();
    const results = state.appState.relevanceResults;
    expect(Array.isArray(results)).toBe(true);
    expect(results).toHaveLength(0);
  });
});
