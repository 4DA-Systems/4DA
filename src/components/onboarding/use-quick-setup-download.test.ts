// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Model-download lifecycle in useQuickSetup: a user's Cancel is not a failure,
// and a broken progress subscription can never leave the picker hidden.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

const safeListenMock = vi.fn();
vi.mock('../../lib/tauri-events', () => ({
  safeListen: (...args: unknown[]) => safeListenMock(...args),
}));

vi.mock('../../utils/normalize-ollama', () => ({
  normalizeOllamaStatus: (raw: unknown) => raw,
}));

import { useQuickSetup } from './use-quick-setup';

const ollamaMissingModels = {
  running: true,
  version: '0.12.0',
  models: [],
  has_embedding_model: false,
  has_llm_model: false,
  base_url: 'http://localhost:11434',
};

const props = { isAnimating: false, onComplete: vi.fn(), onBack: vi.fn() };

function installBackend(overrides: Record<string, (args?: unknown) => unknown>) {
  cmdMock.mockImplementation((command: string, args?: unknown) => {
    const override = overrides[command];
    if (override) return override(args);
    switch (command) {
      case 'check_ollama_status':
        return Promise.resolve(ollamaMissingModels);
      case 'ace_auto_discover':
        return Promise.resolve({ scan_result: { combined: { topics: [] } } });
      case 'taste_test_is_calibrated':
        return Promise.resolve(false);
      case 'ace_get_suggested_interests':
        return Promise.resolve([]);
      default:
        return Promise.resolve();
    }
  });
}

describe('useQuickSetup — model download', () => {
  beforeEach(() => {
    cmdMock.mockReset();
    safeListenMock.mockReset();
    safeListenMock.mockResolvedValue(() => {});
  });

  it('a user Cancel ends the download without reporting a failure', async () => {
    let rejectPull: (reason: unknown) => void = () => {};
    installBackend({
      pull_ollama_model: () => new Promise((_, reject) => { rejectPull = reject; }),
      cancel_ollama_pull: () => {
        rejectPull('Model download cancelled');
        return Promise.resolve('Cancellation requested');
      },
    });
    const { result } = renderHook(() => useQuickSetup(props));
    await act(async () => {});

    act(() => result.current.downloadLocalModels());
    await act(async () => {});
    expect(result.current.pullingModels).toBe(true);

    await act(async () => {
      await result.current.cancelDownload();
    });

    expect(result.current.pullingModels).toBe(false);
    expect(result.current.cancellingDownload).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('a real pull failure is reported', async () => {
    installBackend({
      pull_ollama_model: () => Promise.reject('Ollama could not pull llama3.2: disk full'),
    });
    const { result } = renderHook(() => useQuickSetup(props));
    await act(async () => {});

    await act(async () => {
      result.current.downloadLocalModels();
    });
    await act(async () => {});

    expect(result.current.pullingModels).toBe(false);
    expect(result.current.error).toBe('onboarding.setup.downloadFailed');
  });

  it('a progress subscription that throws still clears the downloading state', async () => {
    safeListenMock.mockRejectedValue(new Error('event bridge unavailable'));
    installBackend({});
    const { result } = renderHook(() => useQuickSetup(props));
    await act(async () => {});

    await act(async () => {
      result.current.downloadLocalModels();
    });
    await act(async () => {});

    expect(result.current.pullingModels).toBe(false);
    expect(cmdMock.mock.calls.some((c) => c[0] === 'pull_ollama_model')).toBe(false);
  });
});
