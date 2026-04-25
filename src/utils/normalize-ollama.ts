// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { OllamaStatus } from '../store/types';

/**
 * The Rust backend returns Ollama models as objects {name, size, modified_at}
 * but the frontend OllamaStatus type expects models as string[].
 * This normalizer extracts model names safely regardless of input shape.
 */
export function normalizeOllamaStatus(raw: Record<string, unknown>): OllamaStatus {
  const rawModels = Array.isArray(raw.models) ? raw.models : [];
  const models: string[] = rawModels
    .map((m: unknown) => typeof m === 'string' ? m : (m as Record<string, string>)?.name ?? '')
    .filter(Boolean);

  return {
    running: !!raw.running,
    version: (raw.version as string) ?? null,
    models,
    base_url: (raw.url as string) ?? (raw.base_url as string) ?? 'http://localhost:11434',
    has_embedding_model: raw.has_embedding_model as boolean | undefined,
    has_llm_model: raw.has_llm_model as boolean | undefined,
  };
}
