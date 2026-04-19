// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('settings-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has settings null', () => {
      expect(useAppStore.getState().settings).toBeNull();
    });

    it('has default settingsForm values', () => {
      const form = useAppStore.getState().settingsForm;
      expect(form.provider).toBe('anthropic');
      expect(form.apiKey).toBe('');
      expect(form.model).toBe('claude-haiku-4-5-20251001');
      expect(form.baseUrl).toBe('');
      expect(form.rerankEnabled).toBe(false);
      expect(form.maxItems).toBe(15);
      expect(form.minScore).toBe(0.25);
      expect(form.dailyTokenLimit).toBe(100000);
      expect(form.dailyCostLimit).toBe(50);
    });

    it('has empty settingsStatus', () => {
      expect(useAppStore.getState().settingsStatus).toBe('');
    });

    it('has showOnboarding false', () => {
      expect(useAppStore.getState().showOnboarding).toBe(false);
    });

    it('has ollamaStatus null', () => {
      expect(useAppStore.getState().ollamaStatus).toBeNull();
    });

    it('has empty ollamaModels array', () => {
      expect(useAppStore.getState().ollamaModels).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // setSettingsForm (partial update)
  // ---------------------------------------------------------------------------
  describe('setSettingsForm', () => {
    it('merges partial updates into settingsForm', () => {
      useAppStore.getState().setSettingsForm({ provider: 'ollama', model: 'llama3' });

      const form = useAppStore.getState().settingsForm;
      expect(form.provider).toBe('ollama');
      expect(form.model).toBe('llama3');
      // Untouched fields remain at defaults
      expect(form.apiKey).toBe('');
      expect(form.maxItems).toBe(15);
    });

    it('can update apiKey', () => {
      useAppStore.getState().setSettingsForm({ apiKey: 'sk-test-key' });
      expect(useAppStore.getState().settingsForm.apiKey).toBe('sk-test-key');
    });

    it('can update rerank settings', () => {
      useAppStore.getState().setSettingsForm({
        rerankEnabled: true,
        maxItems: 25,
        minScore: 0.5,
      });

      const form = useAppStore.getState().settingsForm;
      expect(form.rerankEnabled).toBe(true);
      expect(form.maxItems).toBe(25);
      expect(form.minScore).toBe(0.5);
    });

    it('can update token and cost limits', () => {
      useAppStore.getState().setSettingsForm({
        dailyTokenLimit: 200000,
        dailyCostLimit: 100,
      });

      const form = useAppStore.getState().settingsForm;
      expect(form.dailyTokenLimit).toBe(200000);
      expect(form.dailyCostLimit).toBe(100);
    });

    it('can update baseUrl', () => {
      useAppStore.getState().setSettingsForm({ baseUrl: 'http://localhost:11434' });
      expect(useAppStore.getState().settingsForm.baseUrl).toBe('http://localhost:11434');
    });
  });

  // ---------------------------------------------------------------------------
  // setSettingsFormFull (full replacement or updater function)
  // ---------------------------------------------------------------------------
  describe('setSettingsFormFull', () => {
    it('replaces entire settingsForm when given an object', () => {
      const newForm = {
        provider: 'openai',
        apiKey: 'sk-openai-test',
        model: 'gpt-4',
        baseUrl: '',
        rerankEnabled: true,
        maxItems: 20,
        minScore: 0.3,
        dailyTokenLimit: 50000,
        dailyCostLimit: 25,
      };

      useAppStore.getState().setSettingsFormFull(newForm);

      const form = useAppStore.getState().settingsForm;
      expect(form.provider).toBe('openai');
      expect(form.apiKey).toBe('sk-openai-test');
      expect(form.model).toBe('gpt-4');
      expect(form.maxItems).toBe(20);
    });

    it('accepts an updater function', () => {
      useAppStore.getState().setSettingsForm({ maxItems: 10 });

      useAppStore.getState().setSettingsFormFull(prev => ({
        ...prev,
        maxItems: prev.maxItems + 5,
      }));

      expect(useAppStore.getState().settingsForm.maxItems).toBe(15);
    });
  });

  // ---------------------------------------------------------------------------
  // setSettingsStatus
  // ---------------------------------------------------------------------------
  describe('setSettingsStatus', () => {
    it('sets the status string', () => {
      useAppStore.getState().setSettingsStatus('Saving...');
      expect(useAppStore.getState().settingsStatus).toBe('Saving...');
    });

    it('can clear the status', () => {
      useAppStore.getState().setSettingsStatus('Saved!');
      useAppStore.getState().setSettingsStatus('');
      expect(useAppStore.getState().settingsStatus).toBe('');
    });
  });

  // ---------------------------------------------------------------------------
  // setShowOnboarding
  // ---------------------------------------------------------------------------
  describe('setShowOnboarding', () => {
    it('sets showOnboarding to true', () => {
      useAppStore.getState().setShowOnboarding(true);
      expect(useAppStore.getState().showOnboarding).toBe(true);
    });

    it('sets showOnboarding back to false', () => {
      useAppStore.getState().setShowOnboarding(true);
      useAppStore.getState().setShowOnboarding(false);
      expect(useAppStore.getState().showOnboarding).toBe(false);
    });
  });
});
