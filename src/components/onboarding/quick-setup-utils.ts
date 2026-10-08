// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { cmd } from '../../lib/commands';
import { isEmbeddingModel, normalizeOllamaStatus } from '../../utils/normalize-ollama';
import type { OllamaStatus, PullProgress } from './types';

export type ProviderType = 'anthropic' | 'openai' | 'ollama' | 'openai-compatible';

/**
 * Shown in the key field after `import_env_key` stored the real key server-side.
 * It is a display marker, never a key: format checks and the live probe skip it,
 * and the backend (`IMPORTED_KEY_PLACEHOLDER` in settings_commands.rs) treats it
 * as "unchanged" so it can never overwrite the imported key.
 */
export const IMPORTED_FROM_ENV = '(imported from environment)';

/**
 * Token sent to a local OpenAI-compatible server (LM Studio, llama.cpp, Jan).
 * They ignore auth unless the user turned it on, but the provider gate treats
 * an openai-compatible provider without a key as "no LLM" — this keeps a
 * keyless local server counted as configured.
 */
export const LOCAL_SERVER_TOKEN = 'local-server';

/** A detected local server, ready to save as the provider. */
export interface LocalServerChoice {
  name: string;
  baseUrl: string;
  model: string;
}

/**
 * Turn a detected local server into a saveable choice. The LLM client appends
 * `/chat/completions` to the base URL, so OpenAI-compatible servers need the
 * `/v1` root. Returns null when the server has no model loaded — saving it
 * would leave every completion failing.
 */
export function localServerChoice(server: { name: string; base_url: string; models?: string[] }): LocalServerChoice | null {
  const model = server.models?.[0];
  if (!model) return null;
  const root = server.base_url.replace(/\/+$/, '');
  return { name: server.name, baseUrl: root.endsWith('/v1') ? root : `${root}/v1`, model };
}

export interface UseQuickSetupProps {
  isAnimating: boolean;
  onComplete: () => void;
  onBack: () => void;
}

/**
 * Download size of the default models onboarding pulls, as listed on
 * ollama.com/library (llama3.2 = 3B Q4_K_M, nomic-embed-text = v1.5 F16).
 */
const MODEL_DOWNLOAD_MB: Record<string, number> = { 'llama3.2': 2000, 'nomic-embed-text': 274 };

/**
 * Total download size for the given models, e.g. "2.3 GB". Null when any model
 * has no known size — the caller then shows no number rather than a wrong one.
 */
export function modelDownloadSize(models: readonly string[]): string | null {
  if (models.length === 0) return null;
  let total = 0;
  for (const m of models) {
    const mb = MODEL_DOWNLOAD_MB[m];
    if (mb === undefined) return null;
    total += mb;
  }
  return total >= 1000 ? `${(total / 1000).toFixed(1)} GB` : `${total} MB`;
}

/**
 * Local AI is ready when Ollama runs and has at least one chat model. No
 * Ollama embedder is needed: 4DA ships its own local embedding model.
 */
export function isOllamaReady(status: OllamaStatus | null | undefined): boolean {
  return !!status?.running && !!status.has_llm_model;
}

/**
 * Build the initial pull-progress map for models that need downloading: a chat
 * model, and only when none is installed.
 */
export function buildInitialPullProgress(status: OllamaStatus): {
  models: string[];
  initial: Record<string, PullProgress>;
} {
  const models: string[] = [];
  if (!status.has_llm_model) models.push('llama3.2');

  const initial: Record<string, PullProgress> = {};
  for (const m of models) initial[m] = { model: m, status: 'waiting', percent: 0, done: false };
  return { models, initial };
}

/**
 * Local models measured as feed judges, best first — keep in step with
 * `MEASURED_LOCAL_JUDGES` in src-tauri/src/llm_judgments.rs. Any other model
 * still works for 4DA's other lanes, but the backend will not let it judge.
 */
const MEASURED_JUDGE_MODELS = ['gemma4:26b', 'gemma4:12b', 'qwen3:14b'];

/**
 * The installed Ollama model 4DA should use: the backend's recommended judge
 * (a measured judge that fits this machine) when given, else a measured judge
 * when one is installed (same prefix match as the backend, so
 * `gemma4:12b-it-qat` counts), else the first chat model, else `llama3.2`.
 */
export function pickOllamaModel(models: readonly string[] | undefined, recommended?: string | null): string {
  const chat = (models ?? []).filter(m => !isEmbeddingModel(m));
  if (recommended && chat.includes(recommended)) return recommended;
  for (const judge of MEASURED_JUDGE_MODELS) {
    const hit = chat.find(m => m.toLowerCase().startsWith(judge));
    if (hit) return hit;
  }
  return chat[0] || 'llama3.2';
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
  if (key === IMPORTED_FROM_ENV) return true; // real key already stored server-side
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
  provider: ProviderType | null,
  apiKey: string,
): Promise<{ ok: boolean; reason?: string }> {
  // Only the two BYOK cloud providers with a non-empty key are probed.
  if (provider !== 'anthropic' && provider !== 'openai') return { ok: true };
  if (apiKey.trim().length === 0) return { ok: true };
  // The imported key was read from the environment server-side; the marker in
  // the field is not a key and would fail every format check.
  if (apiKey === IMPORTED_FROM_ENV) return { ok: true };

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

/**
 * Persist the chosen LLM provider + key to the backend. `null` means the user
 * made no choice: nothing is saved, so no provider is ever set on their behalf.
 */
export async function saveLlmProvider(
  provider: ProviderType | null,
  apiKey: string,
  ollamaStatus: OllamaStatus | null,
  localServer: LocalServerChoice | null = null,
): Promise<void> {
  if (provider === null) return;
  const noProvider = { provider: 'none', apiKey: '', model: '', baseUrl: null, openaiApiKey: null };

  if (provider === 'openai-compatible' && localServer) {
    await cmd('set_llm_provider', {
      provider: 'openai-compatible', apiKey: apiKey.trim() || LOCAL_SERVER_TOKEN, model: localServer.model,
      baseUrl: localServer.baseUrl, openaiApiKey: null,
    });
  } else if (provider === 'ollama') {
    if (ollamaStatus?.running) {
      const ollamaModel = pickOllamaModel(ollamaStatus.models, ollamaStatus.recommended_judge);
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
    // Anthropic: the brief-capable default (judges run on the Haiku sibling).
    const model = provider === 'anthropic' ? 'claude-sonnet-5' : 'gpt-4o-mini';
    await cmd('set_llm_provider', {
      provider, apiKey, model, baseUrl: null,
      openaiApiKey: provider === 'openai' ? apiKey : null,
    });
  } else {
    await cmd('set_llm_provider', noProvider);
  }
}
