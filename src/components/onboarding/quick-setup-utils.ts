// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { cmd } from '../../lib/commands';
import { normalizeOllamaStatus } from '../../utils/normalize-ollama';
import type { OllamaStatus, PullProgress } from './types';

export type ProviderType = 'anthropic' | 'openai' | 'ollama' | 'openai-compatible';

export interface UseQuickSetupProps {
  isAnimating: boolean;
  onComplete: () => void;
  onBack: () => void;
}

/** Build the initial pull-progress map for models that need downloading. */
export function buildInitialPullProgress(status: OllamaStatus): {
  models: string[];
  initial: Record<string, PullProgress>;
} {
  const models: string[] = [];
  if (!status.has_embedding_model) models.push('nomic-embed-text');
  if (!status.has_llm_model) models.push('llama3.2');

  const initial: Record<string, PullProgress> = {};
  for (const m of models) initial[m] = { model: m, status: 'waiting', percent: 0, done: false };
  return { models, initial };
}

/** Re-check Ollama status after a model pull, retrying up to 5 times. */
export async function refreshOllamaAfterPull(): Promise<OllamaStatus | null> {
  for (let attempt = 0; attempt < 5; attempt++) {
    await new Promise(r => setTimeout(r, attempt === 0 ? 2000 : 3000));
    try {
      const raw = await cmd('check_ollama_status', { baseUrl: null }) as unknown as Record<string, unknown>;
      const s = normalizeOllamaStatus(raw);
      if (s.running && s.models.length > 0) return s;
    } catch { /* retry */ }
  }
  return null;
}

/** Validate an API key format for the given provider. Returns true if acceptable. */
export function validateApiKey(provider: ProviderType, key: string): boolean {
  const trimmed = key.trim();
  if (trimmed.length === 0) return false;
  if (provider === 'anthropic') return key.startsWith('sk-ant-') && key.length > 20;
  if (provider === 'openai') return key.startsWith('sk-') && key.length > 20;
  return trimmed.length > 10;
}

/**
 * Live pre-flight probe of an API key before saving, run during onboarding.
 *
 * Policy: warn-and-proceed. Only block on a DEFINITIVE rejection (a wrong
 * format, or the provider returning 401/403). Network blips, rate limits
 * (429), and server errors (5xx) are lenient passes — the backend
 * `validate_api_key` command already returns connection_ok=true for those.
 *
 * Skipped entirely for ollama / openai-compatible / empty keys.
 */
export async function probeKeyBeforeSave(
  provider: ProviderType,
  apiKey: string,
): Promise<{ ok: boolean; reason?: string }> {
  // Only the two BYOK cloud providers with a non-empty key are probed.
  if (provider !== 'anthropic' && provider !== 'openai') return { ok: true };
  if (apiKey.trim().length === 0) return { ok: true };

  try {
    const result = await cmd('validate_api_key', { provider, key: apiKey, baseUrl: null });

    // Key works (or backend was lenient on a transient issue) -> proceed.
    if (result.valid === true) return { ok: true };

    // Definitive format rejection -> block.
    if (result.format_ok === false) {
      return { ok: false, reason: result.error || 'That key format looks wrong for this provider.' };
    }

    // Format is fine but the provider definitively rejected it (401/403) -> block.
    if (result.format_ok === true && result.connection_ok === false) {
      return { ok: false, reason: result.error || 'Your provider rejected this key. Check it and try again.' };
    }

    // Anything else (lenient pass on a network/transient issue) -> proceed.
    return { ok: true };
  } catch {
    // Never block onboarding on a probe crash.
    return { ok: true };
  }
}

/** Persist the chosen LLM provider + key to the backend. */
export async function saveLlmProvider(
  provider: ProviderType,
  apiKey: string,
  ollamaStatus: OllamaStatus | null,
): Promise<void> {
  const noProvider = { provider: 'none', apiKey: '', model: '', baseUrl: null, openaiApiKey: null };

  if (provider === 'ollama') {
    if (ollamaStatus?.running) {
      const ollamaModel = ollamaStatus.models?.find(m => !m.startsWith('nomic-embed-text')) || 'llama3.2';
      await cmd('set_llm_provider', {
        provider: 'ollama', apiKey: '', model: ollamaModel,
        baseUrl: ollamaStatus.base_url || 'http://localhost:11434', openaiApiKey: null,
      });
    } else {
      await cmd('set_llm_provider', noProvider);
    }
  } else if (provider === 'openai-compatible' && apiKey.trim()) {
    await cmd('set_llm_provider', {
      provider: 'openai-compatible', apiKey, model: '',
      baseUrl: null, openaiApiKey: null,
    });
  } else if (apiKey.trim()) {
    const model = provider === 'anthropic' ? 'claude-haiku-4-5-20251001' : 'gpt-4o-mini';
    await cmd('set_llm_provider', {
      provider, apiKey, model, baseUrl: null,
      openaiApiKey: provider === 'openai' ? apiKey : null,
    });
  } else {
    await cmd('set_llm_provider', noProvider);
  }
}
