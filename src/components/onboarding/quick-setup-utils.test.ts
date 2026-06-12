// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for the onboarding LLM-provider persistence helpers.
//
// The contract under test is the *honest-state* guarantee from the proxy-derived-state
// antibody (.ai/FAILURE_MODES.md → "Proxy-derived state claims"): the onboarding flow
// must never persist a provider the system can't actually run. In particular, a cloud
// provider with no key, or Ollama when not running, must persist `none`.

import { describe, it, expect, vi, beforeEach } from 'vitest';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

import {
  saveLlmProvider,
  validateApiKey,
  buildInitialPullProgress,
  probeKeyBeforeSave,
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

describe('probeKeyBeforeSave', () => {
  beforeEach(() => cmdMock.mockReset());

  it('skips the probe for ollama (returns ok without calling the backend)', async () => {
    const r = await probeKeyBeforeSave('ollama', '');
    expect(r).toEqual({ ok: true });
    expect(cmdMock).not.toHaveBeenCalled();
  });

  it('skips the probe for openai-compatible', async () => {
    const r = await probeKeyBeforeSave('openai-compatible', 'sk-whatever');
    expect(r).toEqual({ ok: true });
    expect(cmdMock).not.toHaveBeenCalled();
  });

  it('skips the probe for an empty key', async () => {
    const r = await probeKeyBeforeSave('anthropic', '   ');
    expect(r).toEqual({ ok: true });
    expect(cmdMock).not.toHaveBeenCalled();
  });

  it('passes when the key is valid', async () => {
    cmdMock.mockResolvedValueOnce({ valid: true, format_ok: true, connection_ok: true, error: null, model_access: [] });
    const r = await probeKeyBeforeSave('anthropic', 'sk-ant-good-key');
    expect(r.ok).toBe(true);
    expect(cmdMock).toHaveBeenCalledWith('validate_api_key', { provider: 'anthropic', key: 'sk-ant-good-key', baseUrl: null });
  });

  it('BLOCKS on a definitive format rejection (format_ok=false)', async () => {
    cmdMock.mockResolvedValueOnce({ valid: false, format_ok: false, connection_ok: true, error: 'Not an Anthropic key', model_access: [] });
    const r = await probeKeyBeforeSave('anthropic', 'sk-openai-in-wrong-field');
    expect(r.ok).toBe(false);
    expect(r.reason).toBe('Not an Anthropic key');
  });

  it('BLOCKS on a definitive auth rejection (format_ok=true, connection_ok=false)', async () => {
    cmdMock.mockResolvedValueOnce({ valid: false, format_ok: true, connection_ok: false, error: 'Provider rejected the key (401)', model_access: [] });
    const r = await probeKeyBeforeSave('openai', 'sk-revoked-but-well-formed');
    expect(r.ok).toBe(false);
    expect(r.reason).toBe('Provider rejected the key (401)');
  });

  it('PASSES (warn-and-proceed) when the probe throws — never blocks on a crash', async () => {
    cmdMock.mockRejectedValueOnce(new Error('network down'));
    const r = await probeKeyBeforeSave('anthropic', 'sk-ant-cannot-reach');
    expect(r).toEqual({ ok: true });
  });

  it('PASSES on a lenient/transient result (valid=false but connection_ok=true)', async () => {
    cmdMock.mockResolvedValueOnce({ valid: false, format_ok: true, connection_ok: true, error: null, model_access: [] });
    const r = await probeKeyBeforeSave('anthropic', 'sk-ant-rate-limited');
    expect(r.ok).toBe(true);
  });
});
