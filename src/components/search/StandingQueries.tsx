import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

interface StandingQuery {
  id: number;
  query_text: string;
  keywords: string;
  created_at: string;
  last_run: string | null;
  total_matches: number;
  new_matches: number;
  active: boolean;
}

interface StandingQueriesProps {
  isPro: boolean;
}

export function StandingQueries({ isPro }: StandingQueriesProps) {
  const { t } = useTranslation();
  const [watches, setWatches] = useState<StandingQuery[]>([]);
  const [loading, setLoading] = useState(false);

  const loadWatches = useCallback(async () => {
    if (!isPro) return;
    setLoading(true);
    try {
      const result = await invoke<StandingQuery[]>('list_standing_queries');
      setWatches(result);
    } catch (err) {
      console.error('Failed to load standing queries:', err);
    } finally {
      setLoading(false);
    }
  }, [isPro]);

  useEffect(() => {
    loadWatches();
  }, [loadWatches]);

  const handleDelete = async (id: number) => {
    try {
      await invoke('delete_standing_query', { id });
      setWatches((prev) => prev.filter((w) => w.id !== id));
    } catch (err) {
      console.error('Failed to delete standing query:', err);
    }
  };

  if (!isPro) return null;

  return (
    <div className="space-y-2">
      <h4 className="text-xs text-gray-400 uppercase tracking-wider font-medium">
        {t('search.myWatches')}
      </h4>

      {loading && (
        <div className="text-xs text-gray-500">{t('action.loading')}</div>
      )}

      {!loading && watches.length === 0 && (
        <p className="text-xs text-gray-500">{t('search.watchHint')}</p>
      )}

      {watches.map((watch) => (
        <div
          key={watch.id}
          className="flex items-center gap-2 px-3 py-2 bg-bg-secondary rounded-lg border border-border group"
        >
          <span className="text-sm text-gray-300 flex-1 truncate">{watch.query_text}</span>
          <span className="text-xs text-gray-500">{watch.total_matches}</span>
          {watch.new_matches > 0 && (
            <span className="px-1.5 py-0.5 text-[10px] rounded-full bg-cyan-500/20 text-cyan-400 font-medium">
              +{watch.new_matches}
            </span>
          )}
          <button
            onClick={() => handleDelete(watch.id)}
            className="text-gray-600 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-all text-xs"
            aria-label={t('action.delete')}
          >
            {'\u2715'}
          </button>
        </div>
      ))}
    </div>
  );
}
