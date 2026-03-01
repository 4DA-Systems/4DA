export interface ApiKeyState {
  anthropic: string;
  openai: string;
  xApiKey: string;
  provider: 'anthropic' | 'openai' | 'ollama';
}

export interface OllamaStatus {
  running: boolean;
  version: string | null;
  models: string[];
  base_url: string;
  error?: string;
  has_embedding_model?: boolean;
  has_llm_model?: boolean;
}

export interface PullProgress {
  model: string;
  status: string;
  percent: number;
  done: boolean;
}

export interface ScanProgress {
  phase: 'fetching' | 'scoring' | 'done' | 'error';
  message: string;
  results?: Array<{ title: string; score: number; source: string }>;
  total?: number;
  relevant?: number;
}

export type Step = 'welcome' | 'taste' | 'setup';
