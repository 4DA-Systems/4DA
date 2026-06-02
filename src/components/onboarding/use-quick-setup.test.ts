// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for the useQuickSetup hook — provider selection and handleContinue routing
// to the correct persistence helper for the chosen provider.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

// Keep normalize-ollama a transparent passthrough so the hook's branch logic
// (not the normaliser) is what's under test.
vi.mock('../../utils/normalize-ollama', () => ({
  normalizeOllamaStatus: (raw: unknown) => raw,
}));

import { useQuickSetup } from './use-quick-setup';

const SET = 'set_llm_provider';

interface OllamaStub {
  running: boolean;
  models: string[];
  has_embedding_model: boolean;
  has_llm_model: boolean;
  base_url: string;
  version: string | null;
}

const ollamaOffline: OllamaStub = {
  running: false,
  models: [],
  has_embedding_model: false,
  has_llm_model: false,
  base_url: 'http://localhost:11434',
  version: null,
};

// Default backend: no provider, no projects, no taste profile, no builtin model.
function installDefaultBackend(overrides: Record<string, unknown> = {}) {
  cmdMock.mockImplementation((command: string) => {
    if (command in overrides) {
      const v = overrides[command];
      return Promise.resolve(typeof v === 'function' ? (v as () => unknown)() : v);
    }
    switch (command) {
      case 'check_ollama_status':
        return Promise.resolve(ollamaOffline);
      case 'ace_auto_discover':
        return Promise.resolve({ scan_result: { combined: { topics: [] } } });
      case 'taste_test_is_calibrated':
        return Promise.resolve(false);
      case 'taste_test_get_profile':
        return Promise.resolve(null);
      case 'ace_get_suggested_interests':
        return Promise.resolve([]);
      case 'list_builtin_models':
        return Promise.resolve({ models: [] });
      default:
        return Promise.resolve();
    }
  });
}

const props = { isAnimating: false, onComplete: vi.fn(), onBack: vi.fn() };

function setLlmCalls() {
  return cmdMock.mock.calls.filter((c) => c[0] === SET);
}

describe('useQuickSetup — provider selection', () => {
  beforeEach(() => {
    cmdMock.mockReset();
    installDefaultBackend();
  });

  it('handleProviderChange switches the selected provider', async () => {
    const { result } = renderHook(() => useQuickSetup(props));
    await act(async () => {});

    act(() => result.current.handleProviderChange('anthropic'));
    expect(result.current.provider).toBe('anthropic');
  });
});

describe('useQuickSetup — handleContinue routes to the correct persistence helper', () => {
  beforeEach(() => cmdMock.mockReset());

  it('BYOK provider with a key → persists that cloud provider', async () => {
    installDefaultBackend();
    const onComplete = vi.fn();
    const { result } = renderHook(() => useQuickSetup({ ...props, onComplete }));
    await act(async () => {});

    act(() => result.current.handleProviderChange('anthropic'));
    act(() => result.current.handleApiKeyChange('sk-ant-realkey-1234567890'));
    await act(async () => {
      await result.current.handleContinue();
    });

    const persisted = setLlmCalls().map((c) => c[1] as { provider?: string });
    expect(persisted.some((p) => p.provider === 'anthropic')).toBe(true);
    expect(onComplete).toHaveBeenCalled();
  });
});
