// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { OllamaStatus } from '../store/types';

/**
 * Embedding-model families whose names do not contain "embed". Keep in step
 * with `EMBEDDER_FAMILIES` in src-tauri/src/settings_commands_llm/ollama.rs.
 */
const EMBEDDER_FAMILIES = ['nomic-embed', 'mxbai-embed', 'all-minilm', 'snowflake-arctic-embed', 'bge', 'e5'];

/** Whether an Ollama model is an embedder (cannot chat). Mirrors the backend. */
export function isEmbeddingModel(name: string): boolean {
  const lower = name.toLowerCase();
  const base = lower.split('/').pop() ?? lower;
  return base.includes('embed') || EMBEDDER_FAMILIES.some(f => base.startsWith(f));
}

/**
 * The Rust backend returns Ollama models as objects {name, size, modified_at}
 * but the frontend OllamaStatus type expects models as string[].
 * This normalizer extracts model names safely regardless of input shape.
 *
 * Readiness flags come from the backend (`has_llm_model`, ...). An older
 * backend did not send them, and every installed model then read as missing,
 * so they are derived from the model names when absent.
 */
export function normalizeOllamaStatus(raw: Record<string, unknown>): OllamaStatus {
  const rawModels = Array.isArray(raw.models) ? raw.models : [];
  const models: string[] = rawModels
    .map((m: unknown) => typeof m === 'string' ? m : (m as Record<string, string>)?.name ?? '')
    .filter(Boolean);

  const hasLlm = typeof raw.has_llm_model === 'boolean'
    ? raw.has_llm_model
    : models.some(m => !isEmbeddingModel(m));
  const hasEmbed = typeof raw.has_embedding_model === 'boolean'
    ? raw.has_embedding_model
    : models.some(isEmbeddingModel);

  return {
    running: !!raw.running,
    version: (raw.version as string) ?? null,
    models,
    base_url: (raw.url as string) ?? (raw.base_url as string) ?? 'http://localhost:11434',
    has_embedding_model: hasEmbed,
    has_llm_model: hasLlm,
    recommended_judge: typeof raw.recommended_judge === 'string' ? raw.recommended_judge : null,
  };
}
