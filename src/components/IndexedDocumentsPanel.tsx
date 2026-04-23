// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { reportError } from '../lib/error-reporter';
import { formatLocalDate, formatRelativeDate } from '../utils/format-date';
import type {
  IndexedDocument,
  DocumentContentResponse,
  DocumentSearchResult,
  IndexedStats,
} from '../types';

// File type icons
const fileTypeIcons: Record<string, string> = {
  pdf: '📄',
  docx: '📝',
  xlsx: '📊',
  zip: '📦',
  tar: '📦',
  image: '🖼️',
  unknown: '📁',
};

// Format file size
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// Format date (locale-aware relative or absolute)
function formatDate(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffDays < 7) return formatRelativeDate(date);
  return formatLocalDate(date);
}

interface IndexedDocumentsPanelProps {
  onStatusChange?: (status: string) => void;
}

export function IndexedDocumentsPanel({ onStatusChange }: IndexedDocumentsPanelProps) {
  const { t } = useTranslation();
  const [documents, setDocuments] = useState<IndexedDocument[]>([]);
  const [stats, setStats] = useState<IndexedStats | null>(null);
  const [loading, setLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<DocumentSearchResult[] | null>(null);
  const [selectedDoc, setSelectedDoc] = useState<DocumentContentResponse | null>(null);
  const [filterType, setFilterType] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const loaded = useRef(false);

  const loadDocuments = async () => {
    setLoading(true);
    try {
      const [docsResult, statsResult] = await Promise.all([
        cmd('get_indexed_documents', {
          limit: 20,
          offset: 0,
          fileType: filterType,
        }),
        cmd('get_indexed_stats'),
      ]);
      setDocuments(docsResult.documents);
      setStats(statsResult);
      loaded.current = true;
    } catch (error) {
      reportError('IndexedDocumentsPanel.load', error);
      onStatusChange?.(`Error loading documents: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  // Search documents
  const handleSearch = async () => {
    if (!searchQuery.trim()) {
      setSearchResults(null);
      return;
    }
    setLoading(true);
    try {
      const result = await cmd('search_documents', {
        query: searchQuery,
        limit: 10,
      });
      setSearchResults(result.results);
    } catch (error) {
      reportError('IndexedDocumentsPanel.search', error);
      onStatusChange?.(`Search error: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  // Load document content
  const loadDocumentContent = async (docId: number) => {
    setLoading(true);
    try {
      const result = await cmd('get_document_content', {
        documentId: docId,
      });
      setSelectedDoc(result);
    } catch (error) {
      reportError('IndexedDocumentsPanel.loadDocument', error);
      onStatusChange?.(`Error loading document: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (expanded && loaded.current) {
      loadDocuments();
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filterType]);

  // Handle search on Enter
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSearch();
    }
  };

  // Document detail view
  if (selectedDoc) {
    return (
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <div className="flex items-center gap-3 mb-3">
          <button
            onClick={() => setSelectedDoc(null)}
            className="text-sm text-text-secondary hover:text-white transition-colors"
          >
            {t('documents.back')}
          </button>
          <h3 className="text-white font-medium truncate flex-1">
            {selectedDoc.document.file_name}
          </h3>
        </div>

        <div className="grid grid-cols-2 gap-3 mb-3">
          <div className="p-3 bg-bg-secondary rounded-lg border border-border">
            <div className="text-xs text-text-muted">{t('documents.type')}</div>
            <div className="text-sm text-white font-medium">{selectedDoc.document.file_type.toUpperCase()}</div>
          </div>
          <div className="p-3 bg-bg-secondary rounded-lg border border-border">
            <div className="text-xs text-text-muted">{t('documents.size')}</div>
            <div className="text-sm text-white font-medium">{formatFileSize(selectedDoc.document.file_size)}</div>
          </div>
          <div className="p-3 bg-bg-secondary rounded-lg border border-border">
            <div className="text-xs text-text-muted">{t('documents.words')}</div>
            <div className="text-sm text-white font-medium">{selectedDoc.document.word_count.toLocaleString()}</div>
          </div>
          <div className="p-3 bg-bg-secondary rounded-lg border border-border">
            <div className="text-xs text-text-muted">{t('documents.indexed')}</div>
            <div className="text-sm text-white font-medium">{formatDate(selectedDoc.document.indexed_at)}</div>
          </div>
        </div>

        <div className="border-t border-border pt-4">
          <div className="text-xs text-text-secondary mb-3">
            {t('documents.content', { count: selectedDoc.chunks.length })}
          </div>
          <div className="max-h-64 overflow-y-auto space-y-2">
            {selectedDoc.chunks.map((chunk, i) => (
              <div
                key={i}
                className="text-xs text-text-secondary bg-bg-secondary rounded-lg p-3 border border-border whitespace-pre-wrap break-words"
              >
                {chunk.content.slice(0, 500)}
                {chunk.content.length > 500 && '...'}
              </div>
            ))}
          </div>
        </div>

        <div className="text-xs text-text-muted truncate mt-4 pt-3 border-t border-border" title={selectedDoc.document.file_path}>
          {selectedDoc.document.file_path}
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div
        className="flex items-center justify-between cursor-pointer"
        onClick={() => {
          const willExpand = !expanded;
          setExpanded(willExpand);
          if (willExpand && !loaded.current) {
            loadDocuments();
          }
        }}
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-emerald-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
            <span className="text-emerald-400">📚</span>
          </div>
          <div>
            <h3 className="text-white font-medium">{t('documents.title')}</h3>
            <p className="text-text-muted text-sm">
              {stats ? t('documents.documentsIndexed', { count: stats.total_documents }) : t('action.loading')}
            </p>
          </div>
        </div>
        <span className="text-text-muted text-sm">{expanded ? '▼' : '▶'}</span>
      </div>

      {expanded && (
        <div className="mt-3 space-y-3">
          {/* Stats summary */}
          {stats && stats.total_documents > 0 && (
            <div className="flex flex-wrap gap-2">
              {stats.by_type.map((t) => (
                <button
                  key={t.file_type}
                  onClick={(e) => {
                    e.stopPropagation();
                    setFilterType(filterType === t.file_type ? null : t.file_type);
                  }}
                  className={`px-3 py-1.5 text-xs rounded-lg border transition-all ${
                    filterType === t.file_type
                      ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400'
                      : 'bg-bg-secondary border-border text-text-secondary hover:text-white hover:border-emerald-500/30'
                  }`}
                >
                  {fileTypeIcons[t.file_type] || fileTypeIcons.unknown} {t.file_type} ({t.count})
                </button>
              ))}
            </div>
          )}

          {/* Search */}
          <div className="flex gap-2">
            <input
              type="text"
              placeholder={t('documents.searchPlaceholder')}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={handleKeyDown}
              className="flex-1 px-3 py-2.5 text-sm bg-bg-secondary border border-border rounded-lg text-white placeholder:text-text-muted focus:outline-none focus:border-emerald-500/50 transition-colors"
            />
            <button
              onClick={handleSearch}
              disabled={loading}
              className="px-4 py-2.5 text-sm bg-emerald-500/20 border border-emerald-500/30 text-emerald-400 rounded-lg hover:bg-emerald-500/30 disabled:opacity-50 transition-all"
            >
              {loading ? '...' : t('action.search')}
            </button>
          </div>

          {/* Search results */}
          {searchResults !== null && (
            <div className="space-y-2">
              <div className="flex items-center justify-between text-xs text-text-secondary">
                <span>{t('documents.resultsFor', { count: searchResults.length, query: searchQuery })}</span>
                <button
                  onClick={() => {
                    setSearchResults(null);
                    setSearchQuery('');
                  }}
                  aria-label="Clear search results"
                  className="text-text-muted hover:text-white transition-colors"
                >
                  ✕ Clear
                </button>
              </div>
              {searchResults.map((result) => (
                <div
                  key={result.id}
                  onClick={() => loadDocumentContent(result.id)}
                  className="p-3 bg-bg-secondary rounded-lg border border-border cursor-pointer hover:border-emerald-500/30 transition-colors"
                >
                  <div className="flex items-center gap-2">
                    <span>{fileTypeIcons[result.file_type] || fileTypeIcons.unknown}</span>
                    <span className="text-sm text-white truncate flex-1">
                      {result.file_name}
                    </span>
                  </div>
                  <div className="text-xs text-text-muted mt-1 line-clamp-2">
                    {result.preview}
                  </div>
                </div>
              ))}
              {searchResults.length === 0 && (
                <div className="text-center py-4 bg-bg-secondary rounded-lg border border-border">
                  <div className="text-sm text-text-secondary">{t('documents.noResults')}</div>
                </div>
              )}
            </div>
          )}

          {/* Document list */}
          {searchResults === null && (
            <>
              {documents.length > 0 ? (
                <div className="space-y-2 max-h-48 overflow-y-auto">
                  {documents.map((doc) => (
                    <div
                      key={doc.id}
                      onClick={() => loadDocumentContent(doc.id)}
                      className="flex items-center gap-3 p-3 bg-bg-secondary rounded-lg border border-border cursor-pointer hover:border-emerald-500/30 transition-colors"
                    >
                      <span className="text-lg">
                        {fileTypeIcons[doc.file_type] || fileTypeIcons.unknown}
                      </span>
                      <div className="flex-1 min-w-0">
                        <div className="text-sm text-white truncate">
                          {doc.file_name}
                        </div>
                        <div className="text-xs text-text-muted">
                          {formatFileSize(doc.file_size)} • {doc.word_count.toLocaleString()} words
                        </div>
                      </div>
                      <div className="text-xs text-text-muted whitespace-nowrap">
                        {formatDate(doc.indexed_at)}
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="text-center py-6 bg-bg-secondary rounded-lg border border-border">
                  {loading ? (
                    <div className="text-sm text-text-secondary">{t('action.loading')}</div>
                  ) : (
                    <>
                      <div className="text-2xl mb-2">📚</div>
                      <div className="text-sm text-text-secondary">{t('documents.noDocuments')}</div>
                      <div className="text-xs text-text-muted mt-1">
                        {t('documents.addFiles')}
                      </div>
                      <div className="text-[11px] text-text-muted/70 mt-2 max-w-xs mx-auto leading-relaxed">
                        Indexes PDF, Word, and text files from your configured directories to personalise scoring.
                      </div>
                    </>
                  )}
                </div>
              )}

              {/* Stats footer */}
              {stats && stats.total_documents > 0 && (
                <div className="text-xs text-text-muted text-center pt-3 border-t border-border">
                  {t('documents.totalStats', { words: stats.total_words.toLocaleString(), chunks: stats.total_chunks })}
                </div>
              )}
            </>
          )}

          {/* Refresh button */}
          <button
            onClick={loadDocuments}
            disabled={loading}
            className="w-full px-4 py-2.5 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-emerald-500/30 disabled:opacity-50 transition-all"
          >
            {loading ? t('action.loading') : t('action.refresh')}
          </button>
        </div>
      )}
    </div>
  );
}
