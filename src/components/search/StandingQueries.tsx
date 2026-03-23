import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

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

interface StandingQuerySuggestion {
  topic: string;
  reason: string;
  engagement_count: number;
  query_type: string;
}

interface StandingQueriesProps {
  isPro: boolean;
}

export function StandingQueries({ isPro }: StandingQueriesProps) {
  const { t } = useTranslation();
  const [watches, setWatches] = useState<StandingQuery[]>([]);
  const [suggestions, setSuggestions] = useState<StandingQuerySuggestion[]>([]);
  const [loading, setLoading] = useState(false);
  const [creatingSuggestion, setCreatingSuggestion] = useState<string | null>(null);

  const loadWatches = useCallback(async () => {
    if (!isPro) return;
    setLoading(true);
    try {
      const result = await cmd('list_standing_queries') as unknown as StandingQuery[];
      setWatches(result);
    } catch (err) {
      console.error('Failed to load standing queries:', err);
    } finally {
      setLoading(false);
    }
  }, [isPro]);

  const loadSuggestions = useCallback(async () => {
    if (!isPro) return;
    try {
      const result = await cmd('get_standing_query_suggestions') as unknown as StandingQuerySuggestion[];
      setSuggestions(result);
    } catch {
      /* suggestions are non-critical */
    }
  }, [isPro]);

  useEffect(() => {
    loadWatches();
    loadSuggestions();
  }, [loadWatches, loadSuggestions]);

  const handleDelete = async (id: number) => {
    try {
      await cmd('delete_standing_query', { id });
      setWatches((prev) => prev.filter((w) => w.id !== id));
    } catch (err) {
      console.error('Failed to delete standing query:', err);
    }
  };

  const handleWatchSuggestion = async (suggestion: StandingQuerySuggestion) => {
    setCreatingSuggestion(suggestion.topic);
    try {
      await cmd('create_standing_query', { queryText: suggestion.topic });
      // Remove the accepted suggestion and reload watches
      setSuggestions((prev) => prev.filter((s) => s.topic !== suggestion.topic));
      await loadWatches();
    } catch (err) {
      console.error('Failed to create standing query from suggestion:', err);
    } finally {
      setCreatingSuggestion(null);
    }
  };

  const handleDismissSuggestion = (topic: string) => {
    setSuggestions((prev) => prev.filter((s) => s.topic !== topic));
  };

  if (!isPro) return null;

  return (
    <div className="space-y-2">
      {suggestions.length > 0 && (
        <div className="space-y-1.5 mb-3">
          <h4 className="text-xs text-text-muted uppercase tracking-wider font-medium">
            {t('search.suggestedWatches', 'Suggested Watches')}
          </h4>
          {suggestions.map((suggestion) => (
            <div
              key={`${suggestion.query_type}-${suggestion.topic}`}
              className="flex items-center gap-2 px-3 py-2 bg-bg-tertiary/50 rounded-lg border border-border/50 group"
            >
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-accent-gold/10 text-accent-gold font-medium uppercase">
                {suggestion.query_type}
              </span>
              <span className="text-sm text-text-secondary flex-1 truncate">
                {suggestion.topic}
              </span>
              <span className="text-[10px] text-text-muted hidden group-hover:inline truncate max-w-[140px]">
                {suggestion.reason}
              </span>
              <button
                onClick={() => handleWatchSuggestion(suggestion)}
                disabled={creatingSuggestion === suggestion.topic}
                className="px-2 py-0.5 text-[10px] rounded bg-white/5 text-text-secondary hover:bg-white/10 hover:text-text-primary transition-all font-medium disabled:opacity-50"
              >
                {creatingSuggestion === suggestion.topic ? '...' : t('search.watch', 'Watch')}
              </button>
              <button
                onClick={() => handleDismissSuggestion(suggestion.topic)}
                className="text-text-muted hover:text-text-secondary opacity-0 group-hover:opacity-100 focus:opacity-100 transition-all text-xs"
                aria-label={t('action.dismiss', 'Dismiss')}
              >
                {'\u2715'}
              </button>
            </div>
          ))}
        </div>
      )}

      <h4 className="text-xs text-text-secondary uppercase tracking-wider font-medium">
        {t('search.myWatches')}
      </h4>

      {loading && (
        <div className="text-xs text-text-muted">{t('action.loading')}</div>
      )}

      {!loading && watches.length === 0 && (
        <p className="text-xs text-text-muted">{t('search.watchHint')}</p>
      )}

      {watches.map((watch) => (
        <div
          key={watch.id}
          className="flex items-center gap-2 px-3 py-2 bg-bg-secondary rounded-lg border border-border group"
        >
          <span className="text-sm text-text-secondary flex-1 truncate">{watch.query_text}</span>
          <span className="text-xs text-text-muted">{watch.total_matches}</span>
          {watch.new_matches > 0 && (
            <span className="px-1.5 py-0.5 text-[10px] rounded-full bg-cyan-500/20 text-cyan-400 font-medium">
              +{watch.new_matches}
            </span>
          )}
          <button
            onClick={() => handleDelete(watch.id)}
            className="text-text-muted hover:text-red-400 opacity-0 group-hover:opacity-100 focus:opacity-100 transition-all text-xs"
            aria-label={t('action.delete')}
          >
            {'\u2715'}
          </button>
        </div>
      ))}
    </div>
  );
}
