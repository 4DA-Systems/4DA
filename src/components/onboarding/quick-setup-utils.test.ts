// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for the onboarding LLM-provider persistence helpers — the path that was
// shipped (commits 1f65229c "persist the built-in model as the provider" +
// ce67a49e) with only live click-through verification and no unit coverage.
//
// The contract under test is the *honest-state* guarantee from the proxy-derived-state
// antibody (.ai/FAILURE_MODES.md → "Proxy-derived state claims"): the onboarding flow
// must never persist a provider the system can't actually run. In particular, picking
// "Built-in" with no downloaded model must persist `none`, NOT a false-ready `builtin`.

import { describe, it, expect, vi, beforeEach } from 'vitest';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

import {
  saveBuiltinProvider,
  saveLlmProvider,
  validateApiKey,
  buildInitialPullProgress,
} from './quick-setup-utils';
import type { OllamaStatus } from './types';

const SET = 'set_llm_provider';
const NONE = { provider: 'none', apiKey: '', model: '', baseUrl: null, openaiApiKey: null };

/** The single set_llm_provider payload the helper persisted (the decision under test). */
function persistedProvider(): Record<string, unknown> | undefined {
  const call = cmdMock.mock.calls.find((c) => c[0] === SET);
  return call?.[1] as Record<string, unknown> | undefined;
}

function ollama(overrides: Partial<OllamaStatus> = {}): OllamaStatus {
  return {
    running: true,
    version: '0.1.0',
    models: ['llama3.2', 'nomic-embed-text'],
    base_url: 'http://localhost:11434',
    has_embedding_model: true,
    has_llm_model: true,
    ...overrides,
  };
}

describe('saveBuiltinProvider', () => {
  beforeEach(() => cmdMock.mockReset());

  it('persists provider=builtin + the downloaded model id when a model is available', async () => {
    cmdMock.mockImplementation((command: string) =>
      command === 'list_builtin_models'
        ? Promise.resolve({ models: [{ id: 'qwen3-14b-q4km', downloaded: true }] })
        : Promise.resolve(),
    );

    await saveBuiltinProvider();

    expect(cmdMock).toHaveBeenCalledWith(SET, {
      provider: 'builtin',
      apiKey: '',
      model: 'qwen3-14b-q4km',
      baseUrl: null,
      openaiApiKey: null,
    });
  });

  it('picks the FIRST downloaded model when several exist', async () => {
    cmdMock.mockImplementation((command: string) =>
      command === 'list_builtin_models'
        ? Promise.resolve({
            models: [
              { id: 'not-yet', downloaded: false },
              { id: 'ready-a', downloaded: true },
              { id: 'ready-b', downloaded: true },
            ],
          })
        : Promise.resolve(),
    );

    await saveBuiltinProvider();

    expect(persistedProvider()).toMatchObject({ provider: 'builtin', model: 'ready-a' });
  });

  it('persists honest `none` when a model exists but is NOT downloaded (false-ready guard)', async () => {
    cmdMock.mockImplementation((command: string) =>
      command === 'list_builtin_models'
        ? Promise.resolve({ models: [{ id: 'qwen3-14b-q4km', downloaded: false }] })
        : Promise.resolve(),
    );

    await saveBuiltinProvider();

    expect(persistedProvider()).toEqual(NONE);
    // The core invariant: it must NEVER claim builtin without a runnable model.
    const claimedBuiltin = cmdMock.mock.calls.some(
      (c) => c[0] === SET && (c[1] as { provider?: string })?.provider === 'builtin',
    );
    expect(claimedBuiltin).toBe(false);
  });

  it('persists `none` when the builtin model list is empty', async () => {
    cmdMock.mockImplementation((command: string) =>
      command === 'list_builtin_models' ? Promise.resolve({ models: [] }) : Promise.resolve(),
    );

    await saveBuiltinProvider();

    expect(persistedProvider()).toEqual(NONE);
  });

  it('persists `none` when list_builtin_models returns no models field', async () => {
    cmdMock.mockImplementation((command: string) =>
      command === 'list_builtin_models' ? Promise.resolve({}) : Promise.resolve(),
    );

    await saveBuiltinProvider();

    expect(persistedProvider()).toEqual(NONE);
  });

  it('falls back to `none` when the sidecar query throws (graceful, never panics)', async () => {
    cmdMock.mockImplementation((command: string) =>
      command === 'list_builtin_models'
        ? Promise.reject(new Error('sidecar unavailable'))
        : Promise.resolve(),
    );

    await saveBuiltinProvider();

    expect(persistedProvider()).toEqual(NONE);
  });
});

describe('saveLlmProvider', () => {
  beforeEach(() => cmdMock.mockReset());

  it('ollama + running → persists ollama with the first non-embedding model and base_url', async () => {
    await saveLlmProvider('ollama', '', ollama({ models: ['nomic-embed-text', 'qwen2.5-coder'] }));

    expect(persistedProvider()).toMatchObject({
      provider: 'ollama',
      model: 'qwen2.5-coder',
      baseUrl: 'http://localhost:11434',
    });
  });

  it('ollama + running but only an embedding model → falls back to the llama3.2 default', async () => {
    await saveLlmProvider('ollama', '', ollama({ models: ['nomic-embed-text'] }));
    expect(persistedProvider()).toMatchObject({ provider: 'ollama', model: 'llama3.2' });
  });

  it('ollama + NOT running → persists honest `none` (no provider it cannot reach)', async () => {
    await saveLlmProvider('ollama', '', ollama({ running: false }));
    expect(persistedProvider()).toEqual(NONE);
  });

  it('ollama selected but status null → persists `none`', async () => {
    await saveLlmProvider('ollama', '', null);
    expect(persistedProvider()).toEqual(NONE);
  });

  it('anthropic + key → persists anthropic with the haiku model, no openaiApiKey', async () => {
    await saveLlmProvider('anthropic', 'sk-ant-realkey-1234567890', null);
    expect(persistedProvider()).toMatchObject({
      provider: 'anthropic',
      apiKey: 'sk-ant-realkey-1234567890',
      model: 'claude-haiku-4-5-20251001',
      openaiApiKey: null,
    });
  });

  it('openai + key → persists openai with the mini model AND mirrors the key to openaiApiKey', async () => {
    await saveLlmProvider('openai', 'sk-openai-1234567890', null);
    expect(persistedProvider()).toMatchObject({
      provider: 'openai',
      model: 'gpt-4o-mini',
      openaiApiKey: 'sk-openai-1234567890',
    });
  });

  it('openai-compatible + key → persists the openai-compatible provider', async () => {
    await saveLlmProvider('openai-compatible', 'local-endpoint-key-123', null);
    expect(persistedProvider()).toMatchObject({ provider: 'openai-compatible', apiKey: 'local-endpoint-key-123' });
  });

  it('cloud provider with an EMPTY key → persists `none` (no false-ready cloud claim)', async () => {
    await saveLlmProvider('anthropic', '   ', null);
    expect(persistedProvider()).toEqual(NONE);
  });
});

describe('validateApiKey', () => {
  it('accepts a well-formed anthropic key and rejects malformed/short ones', () => {
    expect(validateApiKey('anthropic', 'sk-ant-1234567890123456789')).toBe(true);
    expect(validateApiKey('anthropic', 'sk-ant-short')).toBe(false);
    expect(validateApiKey('anthropic', 'no-prefix-1234567890123456')).toBe(false);
  });

  it('accepts a well-formed openai key and rejects malformed ones', () => {
    expect(validateApiKey('openai', 'sk-openai-key-1234567890')).toBe(true);
    expect(validateApiKey('openai', 'pk-openai-key-1234567890')).toBe(false);
  });

  it('accepts any sufficiently long key for openai-compatible endpoints', () => {
    expect(validateApiKey('openai-compatible', 'endpoint-token-1234')).toBe(true);
    expect(validateApiKey('openai-compatible', 'short')).toBe(false);
  });

  it('rejects empty / whitespace-only keys for every provider', () => {
    expect(validateApiKey('anthropic', '')).toBe(false);
    expect(validateApiKey('openai', '   ')).toBe(false);
    expect(validateApiKey('openai-compatible', '')).toBe(false);
  });
});

describe('buildInitialPullProgress', () => {
  it('queues only the models that are missing', () => {
    const { models, initial } = buildInitialPullProgress(
      ollama({ has_embedding_model: false, has_llm_model: true }),
    );
    expect(models).toEqual(['nomic-embed-text']);
    expect(initial['nomic-embed-text']).toMatchObject({ status: 'waiting', done: false });
  });

  it('queues both models when neither is present', () => {
    const { models } = buildInitialPullProgress(
      ollama({ has_embedding_model: false, has_llm_model: false }),
    );
    expect(models).toEqual(['nomic-embed-text', 'llama3.2']);
  });

  it('queues nothing when both models are present', () => {
    const { models } = buildInitialPullProgress(ollama());
    expect(models).toEqual([]);
  });
});
