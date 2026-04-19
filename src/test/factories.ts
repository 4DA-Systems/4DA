// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Shared test factories for 4DA frontend tests.
 *
 * Provides consistent default objects for SourceRelevance, ScoreBreakdown,
 * and Settings — reducing boilerplate across test files.
 */
import type { SourceRelevance, ScoreBreakdown } from '../types';
import type { Settings } from '../types/settings';

/**
 * Create a SourceRelevance with sensible defaults.
 * Override any field via the `overrides` parameter.
 */
export function makeItem(overrides: Partial<SourceRelevance> = {}): SourceRelevance {
  return {
    id: 1,
    title: 'Test Article Title',
    url: 'https://example.com/article',
    top_score: 0.42,
    matches: [
      {
        source_file: 'src/main.rs',
        matched_text: 'Relevant code snippet',
        similarity: 0.85,
      },
    ],
    relevant: true,
    explanation: 'This is relevant because it matches your context.',
    source_type: 'hackernews',
    ...overrides,
  };
}

/**
 * Create a ScoreBreakdown with sensible defaults.
 */
export function makeBreakdown(overrides: Partial<ScoreBreakdown> = {}): ScoreBreakdown {
  return {
    context_score: 0.5,
    interest_score: 0.3,
    ace_boost: 0.1,
    affinity_mult: 1.0,
    anti_penalty: 0.0,
    confidence_by_signal: {},
    ...overrides,
  };
}

/**
 * Create a Settings object with sensible defaults.
 */
export function makeSettings(overrides: Partial<Settings> = {}): Settings {
  return {
    llm: {
      provider: 'ollama',
      model: 'llama3.2',
      has_api_key: false,
      base_url: null,
    },
    rerank: {
      enabled: false,
      max_items_per_batch: 10,
      min_embedding_score: 0.3,
      daily_token_limit: 100000,
      daily_cost_limit_cents: 50,
    },
    usage: {
      tokens_today: 0,
      cost_today_cents: 0,
      tokens_total: 0,
      items_reranked: 0,
    },
    embedding_threshold: 0.35,
    license: {
      tier: 'free',
      has_key: false,
      activated_at: null,
    },
    ...overrides,
  };
}
