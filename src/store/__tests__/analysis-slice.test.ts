import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';

// Mock @tauri-apps/api/core since store slices import invoke from there
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Capture initial state before any test mutations
const initialState = useAppStore.getState();

describe('analysis-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has loading false', () => {
      expect(useAppStore.getState().appState.loading).toBe(false);
    });

    it('has empty relevanceResults', () => {
      expect(useAppStore.getState().appState.relevanceResults).toEqual([]);
    });

    it('has empty contextFiles', () => {
      expect(useAppStore.getState().appState.contextFiles).toEqual([]);
    });

    it('has analysisComplete false', () => {
      expect(useAppStore.getState().appState.analysisComplete).toBe(false);
    });

    it('has progress at 0', () => {
      expect(useAppStore.getState().appState.progress).toBe(0);
    });

    it('has empty progressMessage', () => {
      expect(useAppStore.getState().appState.progressMessage).toBe('');
    });

    it('has empty progressStage', () => {
      expect(useAppStore.getState().appState.progressStage).toBe('');
    });

    it('has status "Ready to analyze"', () => {
      expect(useAppStore.getState().appState.status).toBe('Ready to analyze');
    });

    it('has lastAnalyzedAt null', () => {
      expect(useAppStore.getState().appState.lastAnalyzedAt).toBeNull();
    });

    it('has expandedItem null', () => {
      expect(useAppStore.getState().expandedItem).toBeNull();
    });

    it('has isBrowserMode false', () => {
      expect(useAppStore.getState().isBrowserMode).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // setAppState (partial update)
  // ---------------------------------------------------------------------------
  describe('setAppState', () => {
    it('merges partial state into appState', () => {
      useAppStore.getState().setAppState({ loading: true, status: 'Analyzing...' });

      const { appState } = useAppStore.getState();
      expect(appState.loading).toBe(true);
      expect(appState.status).toBe('Analyzing...');
      // Other fields remain unchanged
      expect(appState.analysisComplete).toBe(false);
      expect(appState.progress).toBe(0);
    });

    it('can update progress fields', () => {
      useAppStore.getState().setAppState({
        progress: 50,
        progressMessage: 'Halfway there',
        progressStage: 'embedding',
      });

      const { appState } = useAppStore.getState();
      expect(appState.progress).toBe(50);
      expect(appState.progressMessage).toBe('Halfway there');
      expect(appState.progressStage).toBe('embedding');
    });

    it('can mark analysis complete', () => {
      const now = new Date();
      useAppStore.getState().setAppState({
        analysisComplete: true,
        loading: false,
        lastAnalyzedAt: now,
      });

      const { appState } = useAppStore.getState();
      expect(appState.analysisComplete).toBe(true);
      expect(appState.loading).toBe(false);
      expect(appState.lastAnalyzedAt).toBe(now);
    });

    it('preserves relevanceResults when updating other fields', () => {
      const mockResults = [
        { id: 1, title: 'Test', url: null, top_score: 0.9, matches: [], relevant: true },
      ];
      useAppStore.getState().setAppState({ relevanceResults: mockResults as never[] });
      useAppStore.getState().setAppState({ status: 'Updated' });

      expect(useAppStore.getState().appState.relevanceResults).toHaveLength(1);
      expect(useAppStore.getState().appState.status).toBe('Updated');
    });
  });

  // ---------------------------------------------------------------------------
  // setAppStateFull (full replacement or updater function)
  // ---------------------------------------------------------------------------
  describe('setAppStateFull', () => {
    it('replaces the entire appState when given an object', () => {
      const newState = {
        contextFiles: [],
        relevanceResults: [],
        nearMisses: null,
        status: 'Brand new state',
        loading: false,
        analysisComplete: true,
        progress: 100,
        progressMessage: 'Done',
        progressStage: 'complete',
        lastAnalyzedAt: new Date(),
      };

      useAppStore.getState().setAppStateFull(newState);

      const { appState } = useAppStore.getState();
      expect(appState.status).toBe('Brand new state');
      expect(appState.progress).toBe(100);
      expect(appState.analysisComplete).toBe(true);
    });

    it('accepts an updater function', () => {
      useAppStore.getState().setAppState({ progress: 25 });

      useAppStore.getState().setAppStateFull(prev => ({
        ...prev,
        progress: prev.progress + 25,
        progressMessage: `Progress: ${prev.progress + 25}%`,
      }));

      const { appState } = useAppStore.getState();
      expect(appState.progress).toBe(50);
      expect(appState.progressMessage).toBe('Progress: 50%');
    });
  });

  // ---------------------------------------------------------------------------
  // setExpandedItem
  // ---------------------------------------------------------------------------
  describe('setExpandedItem', () => {
    it('sets the expanded item id', () => {
      useAppStore.getState().setExpandedItem(42);
      expect(useAppStore.getState().expandedItem).toBe(42);
    });

    it('clears expanded item when set to null', () => {
      useAppStore.getState().setExpandedItem(42);
      useAppStore.getState().setExpandedItem(null);
      expect(useAppStore.getState().expandedItem).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // clearResults-like behavior via setAppState
  // ---------------------------------------------------------------------------
  describe('clearing results', () => {
    it('can clear relevanceResults and reset analysis state', () => {
      // Set up some results first
      useAppStore.getState().setAppState({
        relevanceResults: [
          { id: 1, title: 'Item', url: null, top_score: 0.8, matches: [], relevant: true },
        ] as never[],
        analysisComplete: true,
        progress: 100,
      });

      // Clear them
      useAppStore.getState().setAppState({
        relevanceResults: [],
        analysisComplete: false,
        progress: 0,
        status: 'Ready to analyze',
      });

      const { appState } = useAppStore.getState();
      expect(appState.relevanceResults).toEqual([]);
      expect(appState.analysisComplete).toBe(false);
      expect(appState.progress).toBe(0);
    });
  });

  // ---------------------------------------------------------------------------
  // Progress updates
  // ---------------------------------------------------------------------------
  describe('progress updates', () => {
    it('tracks progress through multiple stages', () => {
      const { setAppState } = useAppStore.getState();

      setAppState({ progress: 0, progressStage: 'init', progressMessage: 'Starting...' });
      expect(useAppStore.getState().appState.progressStage).toBe('init');

      setAppState({ progress: 30, progressStage: 'fetching', progressMessage: 'Fetching items...' });
      expect(useAppStore.getState().appState.progress).toBe(30);
      expect(useAppStore.getState().appState.progressStage).toBe('fetching');

      setAppState({ progress: 70, progressStage: 'scoring', progressMessage: 'Computing scores...' });
      expect(useAppStore.getState().appState.progress).toBe(70);

      setAppState({ progress: 100, progressStage: 'done', progressMessage: 'Complete', analysisComplete: true });
      expect(useAppStore.getState().appState.progress).toBe(100);
      expect(useAppStore.getState().appState.analysisComplete).toBe(true);
    });
  });
});
