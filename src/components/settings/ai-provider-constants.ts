// SPDX-License-Identifier: FSL-1.1-Apache-2.0

import type { ModelRegistryData } from '../../store/types';

// Curated model lists — these are the models users should see.
// Registry data (LiteLLM) is only used for pricing, not for dropdown population,
// because it includes hundreds of old/deprecated model names.
export const curatedModels: Record<string, string[]> = {
  anthropic: ['claude-haiku-4-5-20251001', 'claude-sonnet-4-6', 'claude-opus-4-6'],
  openai: ['gpt-4.1-nano', 'gpt-4.1-mini', 'gpt-4.1', 'gpt-4o-mini', 'gpt-4o'],
  ollama: ['qwen3:14b', 'gemma3:12b', 'qwen3:8b', 'gemma3:4b', 'deepseek-r1', 'llama3.2', 'phi4'],
};

// Ollama models split by synthesis capability for grouped dropdown.
export const ollamaSynthesisModels = ['qwen3:14b', 'gemma3:12b', 'qwen3:8b'];
export const ollamaOtherModels = ['gemma3:4b', 'deepseek-r1', 'llama3.2', 'phi4'];

// Popular OpenAI-compatible endpoints
export const popularEndpoints: { name: string; url: string }[] = [
  { name: 'Groq', url: 'https://api.groq.com/openai/v1' },
  { name: 'Together', url: 'https://api.together.xyz/v1' },
  { name: 'DeepSeek', url: 'https://api.deepseek.com/v1' },
  { name: 'Mistral', url: 'https://api.mistral.ai/v1' },
  { name: 'OpenRouter', url: 'https://openrouter.ai/api/v1' },
  { name: 'LM Studio', url: 'http://localhost:1234/v1' },
  { name: 'llama.cpp', url: 'http://localhost:8080/v1' },
];

/** Get models for a provider. Uses curated list for known providers,
 *  registry only for openai-compatible (unknown endpoints). */
export function getProviderModels(provider: string, _registry: ModelRegistryData | null | undefined): string[] {
  return curatedModels[provider] ?? [];
}

/** Shape returned by detect_environment command */
export interface EnvDetection {
  has_anthropic_env: boolean;
  anthropic_env_preview: string;
  has_openai_env: boolean;
  openai_env_preview: string;
}

/** State for real-time API key validation */
export interface KeyValidation {
  status: 'idle' | 'checking' | 'valid' | 'invalid' | 'format_error';
  message: string;
  models: string[];
}

export const IDLE_VALIDATION: KeyValidation = { status: 'idle', message: '', models: [] };
