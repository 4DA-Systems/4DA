// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('ui-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has showSettings false', () => {
      expect(useAppStore.getState().showSettings).toBe(false);
    });

    it('has showSplash true', () => {
      expect(useAppStore.getState().showSplash).toBe(true);
    });

    it('has activeView set to briefing', () => {
      expect(useAppStore.getState().activeView).toBe('briefing');
    });

    it('has isFirstRun false', () => {
      expect(useAppStore.getState().isFirstRun).toBe(false);
    });

    it('has firstRunDismissed false', () => {
      expect(useAppStore.getState().firstRunDismissed).toBe(false);
    });

    it('has embeddingMode null', () => {
      expect(useAppStore.getState().embeddingMode).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // setShowSettings
  // ---------------------------------------------------------------------------
  describe('setShowSettings', () => {
    it('sets showSettings to true', () => {
      useAppStore.getState().setShowSettings(true);
      expect(useAppStore.getState().showSettings).toBe(true);
    });

    it('sets showSettings back to false', () => {
      useAppStore.getState().setShowSettings(true);
      useAppStore.getState().setShowSettings(false);
      expect(useAppStore.getState().showSettings).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // setShowSplash
  // ---------------------------------------------------------------------------
  describe('setShowSplash', () => {
    it('sets showSplash to false', () => {
      useAppStore.getState().setShowSplash(false);
      expect(useAppStore.getState().showSplash).toBe(false);
    });

    it('sets showSplash back to true', () => {
      useAppStore.getState().setShowSplash(false);
      useAppStore.getState().setShowSplash(true);
      expect(useAppStore.getState().showSplash).toBe(true);
    });
  });

  // ---------------------------------------------------------------------------
  // setActiveView
  // ---------------------------------------------------------------------------
  describe('setActiveView', () => {
    it('changes active view to results', () => {
      useAppStore.getState().setActiveView('results');
      expect(useAppStore.getState().activeView).toBe('results');
    });

    it('changes active view to saved', () => {
      useAppStore.setState({ viewTier: 'invested' });
      useAppStore.getState().setActiveView('saved');
      expect(useAppStore.getState().activeView).toBe('saved');
    });

    it('changes active view to preemption', () => {
      useAppStore.setState({ viewTier: 'explorer' });
      useAppStore.getState().setActiveView('preemption');
      expect(useAppStore.getState().activeView).toBe('preemption');
    });

    it('changes active view to toolkit', () => {
      useAppStore.setState({ viewTier: 'power' });
      useAppStore.getState().setActiveView('toolkit');
      expect(useAppStore.getState().activeView).toBe('toolkit');
    });

    it('changes active view to playbook', () => {
      useAppStore.getState().setActiveView('playbook');
      expect(useAppStore.getState().activeView).toBe('playbook');
    });

    it('can switch back to briefing', () => {
      useAppStore.getState().setActiveView('results');
      useAppStore.getState().setActiveView('briefing');
      expect(useAppStore.getState().activeView).toBe('briefing');
    });

    it('blocks navigation to views above current tier', () => {
      useAppStore.setState({ viewTier: 'core', activeView: 'briefing' });
      useAppStore.getState().setActiveView('preemption');
      expect(useAppStore.getState().activeView).toBe('briefing');

      useAppStore.setState({ viewTier: 'explorer', activeView: 'briefing' });
      useAppStore.getState().setActiveView('saved');
      expect(useAppStore.getState().activeView).toBe('briefing');
    });

    it('allows all views when showAllViews is true', () => {
      useAppStore.setState({ viewTier: 'core', showAllViews: true });
      useAppStore.getState().setActiveView('toolkit');
      expect(useAppStore.getState().activeView).toBe('toolkit');
    });
  });

  // ---------------------------------------------------------------------------
  // setIsFirstRun / setFirstRunDismissed
  // ---------------------------------------------------------------------------
  describe('setIsFirstRun', () => {
    it('sets isFirstRun to true', () => {
      useAppStore.getState().setIsFirstRun(true);
      expect(useAppStore.getState().isFirstRun).toBe(true);
    });

    it('sets isFirstRun back to false', () => {
      useAppStore.getState().setIsFirstRun(true);
      useAppStore.getState().setIsFirstRun(false);
      expect(useAppStore.getState().isFirstRun).toBe(false);
    });
  });

  describe('setFirstRunDismissed', () => {
    it('sets firstRunDismissed to true', () => {
      useAppStore.getState().setFirstRunDismissed(true);
      expect(useAppStore.getState().firstRunDismissed).toBe(true);
    });
  });

  // ---------------------------------------------------------------------------
  // setEmbeddingMode
  // ---------------------------------------------------------------------------
  describe('setEmbeddingMode', () => {
    it('sets embedding mode to semantic', () => {
      useAppStore.getState().setEmbeddingMode('semantic');
      expect(useAppStore.getState().embeddingMode).toBe('semantic');
    });

    it('sets embedding mode to keyword-only', () => {
      useAppStore.getState().setEmbeddingMode('keyword-only');
      expect(useAppStore.getState().embeddingMode).toBe('keyword-only');
    });

    it('can clear embedding mode back to null', () => {
      useAppStore.getState().setEmbeddingMode('semantic');
      useAppStore.getState().setEmbeddingMode(null);
      expect(useAppStore.getState().embeddingMode).toBeNull();
    });
  });
});
