// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { GhostPreviewData } from './GhostPreview';

export interface QueryResultItem {
  id: number;
  file_path: string | null;
  file_name: string | null;
  preview: string;
  relevance: number;
  source_type: string;
  timestamp: string | null;
  match_reason: string;
}

export interface QueryResult {
  query: string;
  intent: string;
  items: QueryResultItem[];
  total_count: number;
  execution_ms: number;
  summary: string | null;
  parsed: {
    keywords: string[];
    entities: string[];
    time_range: { start: string; end: string; relative: string | null } | null;
    file_types: string[];
    sentiment: string | null;
    confidence: number;
  };
  stack_context: { name: string; category: string; relevant: boolean }[];
  related_decisions: { id: number; subject: string; decision: string; relation: string }[];
  knowledge_gaps: { technology: string; days_stale: number; severity: string }[];
  ghost_preview: GhostPreviewData | null;
  is_pro: boolean;
}

export const intentLabels: Record<string, string> = {
  Find: 'Find',
  Summarize: 'Summarize',
  Compare: 'Compare',
  Timeline: 'Timeline',
  Count: 'Count',
};

export const sourceLabels: Record<string, string> = {
  pdf: 'PDF',
  docx: 'DOC',
  xlsx: 'XLS',
  image: 'IMG',
  context: 'CTX',
};
