import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ItemContent } from '../types';
import { getContentTypeBadge } from '../config/content-types';

interface ArticleReaderProps {
  itemId: number;
  url?: string;
  contentType?: string;
}

export function ArticleReader({ itemId, url, contentType }: ArticleReaderProps) {
  const [content, setContent] = useState<ItemContent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isOpen, setIsOpen] = useState(false);

  const loadContent = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<ItemContent>('get_item_content', { itemId });
      if (!result.content || result.content.trim().length === 0) {
        setError('No content available for this item.');
        setContent(null);
      } else {
        setContent(result);
        setIsOpen(true);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [itemId]);

  const readTime = content ? Math.max(1, Math.ceil(content.word_count / 200)) : 0;
  const badge = getContentTypeBadge(contentType);

  if (!isOpen) {
    return (
      <div className="mb-3">
        <button
          onClick={loadContent}
          disabled={loading}
          className="text-[11px] px-2.5 py-1.5 rounded border border-border text-text-secondary hover:bg-bg-tertiary transition-colors disabled:opacity-50"
        >
          {loading ? 'Loading...' : 'Read Article'}
        </button>
        {error && (
          <div className="mt-1.5 p-2 rounded border border-red-500/30 bg-red-500/5">
            <span className="text-[10px] text-red-400">{error}</span>
            <button
              onClick={loadContent}
              className="ml-2 text-[10px] text-red-300 underline hover:text-red-200"
            >
              Retry
            </button>
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="mb-3 rounded border border-border bg-bg-primary">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-border/50">
        <div className="flex items-center gap-2">
          {badge && (
            <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${badge.colorClass}`}>
              {badge.label}
            </span>
          )}
          <span className="text-[10px] text-text-muted">
            {content?.word_count.toLocaleString()} words
          </span>
          <span className="text-[10px] text-text-muted">
            ~{readTime} min read
          </span>
        </div>
        <div className="flex items-center gap-2">
          {url && (
            <button
              onClick={() => window.navigator.clipboard.writeText(url)}
              className="text-[10px] text-text-muted hover:text-text-secondary transition-colors"
            >
              Copy URL
            </button>
          )}
          <button
            onClick={() => setIsOpen(false)}
            className="text-[10px] text-text-muted hover:text-text-secondary transition-colors"
          >
            Close
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="max-h-96 overflow-y-auto px-3 py-2">
        <div className="text-xs leading-relaxed text-text-secondary whitespace-pre-wrap">
          {content?.content}
        </div>
      </div>
    </div>
  );
}
