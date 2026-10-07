// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { isEmbeddingModel, normalizeOllamaStatus } from './normalize-ollama';

const tag = (name: string) => ({ name, size: 1, modified_at: '' });

describe('normalizeOllamaStatus', () => {
  it('derives readiness from the models when an older backend sends no flags', () => {
    const s = normalizeOllamaStatus({ running: true, version: '0.12', models: [tag('gemma4:12b')], url: 'http://localhost:11434' });
    expect(s.models).toEqual(['gemma4:12b']);
    expect(s.has_llm_model).toBe(true);
    expect(s.has_embedding_model).toBe(false);
  });

  it('an embedder alone is not a chat model', () => {
    const s = normalizeOllamaStatus({ running: true, models: [tag('nomic-embed-text:latest')] });
    expect(s.has_llm_model).toBe(false);
    expect(s.has_embedding_model).toBe(true);
  });

  it('reports nothing installed for an empty list', () => {
    const s = normalizeOllamaStatus({ running: true, models: [] });
    expect(s.has_llm_model).toBe(false);
    expect(s.has_embedding_model).toBe(false);
  });

  it('keeps the backend flags and recommended judge when present', () => {
    const s = normalizeOllamaStatus({
      running: true,
      models: [tag('qwen3:14b'), tag('bge-m3')],
      has_llm_model: false, // backend is authoritative
      has_embedding_model: true,
      recommended_judge: 'qwen3:14b',
    });
    expect(s.has_llm_model).toBe(false);
    expect(s.recommended_judge).toBe('qwen3:14b');
  });

  it('accepts plain string model lists', () => {
    const s = normalizeOllamaStatus({ running: true, models: ['llama3.2', 'mxbai-embed-large'] });
    expect(s.has_llm_model).toBe(true);
    expect(s.has_embedding_model).toBe(true);
    expect(s.recommended_judge).toBeNull();
  });
});

describe('isEmbeddingModel', () => {
  it('matches the backend embedder families', () => {
    for (const m of ['nomic-embed-text', 'all-minilm:l6-v2', 'bge-m3', 'e5-large', 'snowflake-arctic-embed2', 'hf.co/BAAI/bge-small']) {
      expect(isEmbeddingModel(m)).toBe(true);
    }
    for (const m of ['gemma4:12b', 'qwen3:14b', 'llama3.2', 'deepseek-r1:8b']) {
      expect(isEmbeddingModel(m)).toBe(false);
    }
  });
});
