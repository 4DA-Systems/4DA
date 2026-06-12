// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';
import { formatLocalDate, formatRelativeDate } from '../../utils/format-date';
import type { DocumentContentResponse } from '../../types';

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

interface DocumentDetailProps {
  document: DocumentContentResponse;
  onBack: () => void;
}

export function DocumentDetail({ document, onBack }: DocumentDetailProps) {
  const { t } = useTranslation();
  const doc = document.document;

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div className="flex items-center gap-3 mb-3">
        <button
          onClick={onBack}
          className="text-sm text-text-secondary hover:text-text-primary transition-colors"
        >
          {t('documents.back')}
        </button>
        <h3 className="text-text-primary font-medium truncate flex-1">
          {doc.file_name}
        </h3>
      </div>

      <div className="grid grid-cols-2 gap-3 mb-3">
        <div className="p-3 bg-bg-secondary rounded-lg border border-border">
          <div className="text-xs text-text-muted">{t('documents.type')}</div>
          <div className="text-sm text-text-primary font-medium">{doc.file_type.toUpperCase()}</div>
        </div>
        <div className="p-3 bg-bg-secondary rounded-lg border border-border">
          <div className="text-xs text-text-muted">{t('documents.size')}</div>
          <div className="text-sm text-text-primary font-medium">{formatFileSize(doc.file_size)}</div>
        </div>
        <div className="p-3 bg-bg-secondary rounded-lg border border-border">
          <div className="text-xs text-text-muted">{t('documents.words')}</div>
          <div className="text-sm text-text-primary font-medium">{doc.word_count.toLocaleString()}</div>
        </div>
        <div className="p-3 bg-bg-secondary rounded-lg border border-border">
          <div className="text-xs text-text-muted">{t('documents.indexed')}</div>
          <div className="text-sm text-text-primary font-medium">{formatDate(doc.indexed_at)}</div>
        </div>
      </div>

      <div className="border-t border-border pt-4">
        <div className="text-xs text-text-secondary mb-3">
          {t('documents.content', { count: document.chunks.length })}
        </div>
        <div className="max-h-64 overflow-y-auto space-y-2">
          {document.chunks.map((chunk, i) => (
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

      <div className="text-xs text-text-muted truncate mt-4 pt-3 border-t border-border" title={doc.file_path}>
        {doc.file_path}
      </div>
    </div>
  );
}
