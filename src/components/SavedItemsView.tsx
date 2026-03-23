import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import type { SavedItem } from '../types';
import { getSourceLabel, getSourceColorClass } from '../config/sources';
import { useAppStore } from '../store';
import { translateError } from '../utils/error-messages';

export function SavedItemsView() {
  const { t } = useTranslation();
  const [items, setItems] = useState<SavedItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const addToast = useAppStore(s => s.addToast);
  const setFeedbackGivenFull = useAppStore(s => s.setFeedbackGivenFull);

  const loadItems = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await cmd('get_saved_items');
      setItems(result);
    } catch (e) {
      setError(translateError(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadItems(); }, [loadItems]);

  const handleRemove = useCallback(async (itemId: number) => {
    // Optimistic UI: remove immediately
    setItems(prev => prev.filter(i => i.item_id !== itemId));
    // Clear feedback state so save button reappears in results
    setFeedbackGivenFull(prev => {
      const next = { ...prev };
      delete next[itemId];
      return next;
    });

    try {
      await cmd('remove_saved_item', { itemId });
      addToast('success', t('saved.itemRemoved'));
    } catch (e) {
      // Revert on failure
      loadItems();
      addToast('error', `Failed to remove: ${translateError(e)}`);
    }
  }, [addToast, setFeedbackGivenFull, loadItems, t]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12" role="status" aria-busy="true" aria-label="Loading saved items">
        <div className="w-5 h-5 border-2 border-gray-600 border-t-white rounded-full animate-spin" />
      </div>
    );
  }

  if (error) {
    return (
      <div role="alert" className="bg-bg-secondary rounded-lg border border-red-500/30 p-6 text-center">
        <p className="text-sm text-red-400 mb-3">{error}</p>
        <button
          onClick={loadItems}
          className="px-3 py-1.5 text-xs bg-red-500/10 text-red-400 rounded hover:bg-red-500/20 transition-colors"
        >
          {t('action.retry')}
        </button>
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 text-center">
        <p className="text-sm text-text-secondary mb-2">{t('saved.empty.title')}</p>
        <p className="text-xs text-text-muted">
          {t('saved.empty.subtitle')}
        </p>
      </div>
    );
  }

  return (
    <section aria-label={t('saved.title', { defaultValue: 'Saved items' })}>
      <div className="flex items-center justify-between mb-4">
        <span className="text-xs text-text-muted">{t('saved.count', { count: items.length })}</span>
        <button
          onClick={loadItems}
          className="text-[10px] text-text-muted hover:text-text-secondary transition-colors"
        >
          {t('action.refresh')}
        </button>
      </div>

      <div className="space-y-2">
        {items.map(item => (
          <div
            key={item.item_id}
            className="bg-bg-tertiary rounded-lg border border-border p-3 hover:border-[#3A3A3A] transition-colors"
          >
            <div className="flex items-start gap-3">
              {/* Source badge */}
              <span className={`flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium ${getSourceColorClass(item.source_type)}`}>
                {getSourceLabel(item.source_type)}
              </span>

              {/* Content */}
              <div className="flex-1 min-w-0">
                {item.url ? (
                  <a
                    href={item.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-sm text-white hover:text-orange-400 hover:underline decoration-orange-400/50 font-medium transition-colors"
                  >
                    {item.title}
                  </a>
                ) : (
                  <p className="text-sm text-white font-medium">{item.title}</p>
                )}

                {item.summary ? (
                  <p className="text-xs text-text-secondary mt-1 leading-relaxed">{item.summary}</p>
                ) : item.content_preview ? (
                  <p className="text-xs text-text-muted mt-1 leading-relaxed truncate">{item.content_preview}</p>
                ) : null}

                <div className="flex items-center gap-3 mt-1.5">
                  <span className="text-[10px] text-text-muted">
                    {item.saved_at ? new Date(item.saved_at + 'Z').toLocaleDateString() : ''}
                  </span>
                  {item.url && (
                    <button
                      onClick={() => {
                        window.navigator.clipboard.writeText(item.url!);
                        setCopiedId(item.item_id);
                        setTimeout(() => setCopiedId(prev => prev === item.item_id ? null : prev), 2000);
                      }}
                      aria-label={copiedId === item.item_id ? t('saved.copied', 'Copied!') : `${t('saved.copyUrl')} for ${item.title}`}
                      className={`text-[10px] transition-colors ${copiedId === item.item_id ? 'text-green-400' : 'text-text-muted hover:text-text-secondary'}`}
                    >
                      {copiedId === item.item_id ? t('saved.copied', 'Copied!') : t('saved.copyUrl')}
                    </button>
                  )}
                </div>
              </div>

              {/* Remove button */}
              <button
                onClick={() => handleRemove(item.item_id)}
                aria-label={`${t('saved.remove')} ${item.title}`}
                className="flex-shrink-0 text-[10px] px-2 py-1 rounded text-text-muted hover:text-red-400 hover:bg-red-500/10 transition-colors"
              >
                {t('saved.remove')}
              </button>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
