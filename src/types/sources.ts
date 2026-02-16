// Source and document types

export interface IndexedDocument {
  id: number;
  file_path: string;
  file_name: string;
  file_type: string;
  file_size: number;
  word_count: number;
  extraction_confidence: number;
  indexed_at: string;
}

export interface DocumentChunk {
  index: number;
  content: string;
  word_count: number;
}

export interface IndexedDocumentsResponse {
  documents: IndexedDocument[];
  total: number;
  limit: number;
  offset: number;
}

export interface DocumentContentResponse {
  document: IndexedDocument;
  chunks: DocumentChunk[];
}

export interface DocumentSearchResult {
  id: number;
  file_path: string;
  file_name: string;
  file_type: string;
  word_count: number;
  indexed_at: string;
  preview: string;
}

export interface SourceHealthStatus {
  source_type: string;
  status: 'healthy' | 'error' | 'circuit_open' | 'unknown';
  last_success_relative: string | null;
  items_fetched: number;
  gap_message: string | null;
}

export interface SavedItem {
  item_id: number;
  title: string;
  url: string | null;
  source_type: string;
  saved_at: string;
  summary: string | null;
  content_preview: string | null;
}

export interface ItemContent {
  content: string;
  source_type: string;
  word_count: number;
  has_summary: boolean;
  summary: string | null;
}

export interface ItemSummary {
  summary: string;
  cached: boolean;
}
