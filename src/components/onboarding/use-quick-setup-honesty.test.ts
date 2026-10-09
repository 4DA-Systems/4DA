// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Fresh-profile E2E findings (2026-10-09): taste-test guesses became explicit
// full-weight interests, Enter fanned out 21 IPC calls and froze ~30 s, and a
// ready Ollama in a collapsed section was saved with a model nobody saw.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));
vi.mock('../../lib/tauri-events', () => ({
  safeListen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock('../../utils/normalize-ollama', async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
  normalizeOllamaStatus: (raw: unknown) => raw,
}));

import { resetDiscoveryForTests } from '../../lib/discovery';
import { useQuickSetup } from './use-quick-setup';
import { SECTION_KEY } from './onboarding-constants';

const PROFILE = {
  dominantPersonaName: 'Python ML Engineer',
  dominantPersonaDescription: '',
  confidence: 0.67,
  itemsShown: 11,
  personaWeights: [],
  topInterests: ['SQLite', 'vector search', 'deep learning'],
  likedInterests: ['SQLite', 'vector search'],
  guessedInterests: ['deep learning'],
  personaContradicted: true,
};

const READY_OLLAMA = {
  running: true, version: '0.12', base_url: 'http://localhost:11434',
  models: ['gemma4:12b', 'nomic-embed-text'], has_llm_model: true, recommended_judge: 'gemma4:12b',
};

function backend(overrides: Record<string, unknown> = {}) {
  cmdMock.mockImplementation((command: string) => {
    if (command in overrides) return Promise.resolve(overrides[command]);
    switch (command) {
      case 'check_ollama_status': return Promise.resolve({ running: false, models: [], base_url: '' });
      case 'taste_test_is_calibrated': return Promise.resolve(false);
      case 'ace_get_suggested_interests': return Promise.resolve([]);
      case 'ace_preview_discovery_dirs': return Promise.resolve([]);
      default: return Promise.resolve();
    }
  });
}

const props = { isAnimating: false, onComplete: vi.fn(), onBack: vi.fn() };
const calls = (name: string) => cmdMock.mock.calls.filter(c => c[0] === name);
const savedProviders = () => calls('set_llm_provider').map(c => (c[1] as { provider: string; model: string }));
const contextSave = () => (calls('save_onboarding_context')[0]?.[1] as { save: Record<string, unknown> }).save;

beforeEach(() => {
  cmdMock.mockReset();
  resetDiscoveryForTests();
  try { localStorage.clear(); } catch { /* noop */ }
  backend();
});

describe('useQuickSetup — taste-test interests', () => {
  it('pre-selects only what the user liked; guesses are offered, not chosen', async () => {
    backend({ taste_test_is_calibrated: true, taste_test_get_profile: PROFILE });
    const { result } = renderHook(() => useQuickSetup(props));
    await act(async () => {});

    expect(result.current.interests).toEqual(['SQLite', 'vector search']);
    expect(result.current.guessedInterests).toEqual(['deep learning']);
  });

  it('Enter re-saves no like, promotes no guess, and saves everything in ONE call', async () => {
    backend({ taste_test_is_calibrated: true, taste_test_get_profile: PROFILE });
    const onComplete = vi.fn();
    const { result } = renderHook(() => useQuickSetup({ ...props, onComplete }));
    await act(async () => {});

    await act(async () => { await result.current.handleContinue(); });

    expect(calls('save_onboarding_context')).toHaveLength(1);
    expect(calls('add_interest')).toHaveLength(0);
    expect(calls('add_tech_stack')).toHaveLength(0);
    expect(contextSave().addInterests).toEqual([]);
    expect(contextSave().removeInterests).toEqual([]);
    expect(onComplete).toHaveBeenCalled();
  });

  it('a guess the user taps is saved as theirs; a like they remove is removed', async () => {
    backend({ taste_test_is_calibrated: true, taste_test_get_profile: PROFILE });
    const { result } = renderHook(() => useQuickSetup(props));
    await act(async () => {});

    act(() => result.current.toggleInterest('deep learning'));
    act(() => result.current.toggleInterest('SQLite'));
    await act(async () => { await result.current.handleContinue(); });

    expect(contextSave().addInterests).toEqual(['deep learning']);
    expect(contextSave().removeInterests).toEqual(['SQLite']);
  });
});

describe('useQuickSetup — AI provider is never chosen for the user', () => {
  it('a ready Ollama stays in an OPEN section and is saved with its model once seen', async () => {
    backend({ check_ollama_status: READY_OLLAMA });
    const { result } = renderHook(() => useQuickSetup(props));
    await act(async () => {});

    expect(result.current.aiOpen).toBe(true);
    expect(result.current.provider).toBe('ollama');
    await act(async () => { await result.current.handleContinue(); });
    expect(savedProviders()).toEqual([expect.objectContaining({ provider: 'ollama', model: 'gemma4:12b' })]);
  });

  it('an auto-selected Ollama in a section that was never open is not saved', async () => {
    localStorage.setItem(SECTION_KEY, JSON.stringify({ aiOpen: false }));
    backend({ check_ollama_status: READY_OLLAMA });
    const { result } = renderHook(() => useQuickSetup(props));
    await act(async () => {});

    expect(result.current.aiOpen).toBe(false);
    await act(async () => { await result.current.handleContinue(); });
    expect(savedProviders()).toEqual([]);
  });

  it('"Skip — no AI for now" is an explicit choice that saves no provider', async () => {
    backend({ check_ollama_status: READY_OLLAMA });
    const { result } = renderHook(() => useQuickSetup(props));
    await act(async () => {});

    act(() => result.current.handleProviderChange('none'));
    expect(result.current.aiConfigured).toBe(true);
    await act(async () => { await result.current.handleContinue(); });
    expect(savedProviders()).toEqual([expect.objectContaining({ provider: 'none' })]);
  });
});

describe('useQuickSetup — Enter shows its progress', () => {
  it('reports each save stage while the save runs', async () => {
    let release: () => void = () => {};
    backend({ taste_test_is_calibrated: false });
    cmdMock.mockImplementation((command: string) => {
      if (command === 'save_onboarding_context') return new Promise<void>(r => { release = r; });
      if (command === 'check_ollama_status') return Promise.resolve({ running: false, models: [], base_url: '' });
      if (command === 'ace_get_suggested_interests' || command === 'ace_preview_discovery_dirs') return Promise.resolve([]);
      return Promise.resolve(false);
    });
    const { result } = renderHook(() => useQuickSetup(props));
    await act(async () => {});

    let pending: Promise<void> = Promise.resolve();
    await act(async () => { pending = result.current.handleContinue(); });
    expect(result.current.isSaving).toBe(true);
    expect(result.current.saveProgress).toEqual({ done: 3, total: 3 });

    await act(async () => { release(); await pending; });
    expect(result.current.isSaving).toBe(false);
    expect(result.current.saveProgress).toBeNull();
  });
});
