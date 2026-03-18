import { useState, useEffect, useCallback, memo } from 'react';
import { cmd } from '../lib/commands';

interface StandingQuery {
  id: number;
  query_text: string;
  keywords: string[];
  created_at: string;
  last_run: string | null;
  total_matches: number;
  new_matches: number;
  active: boolean;
}

interface StandingQueryMatch {
  item_id: number;
  title: string;
  source_type: string;
  url: string | null;
  discovered_at: string | null;
}

const QUERY_TYPES = [
  { value: 'keyword', label: 'Keyword', description: 'Match exact words in content' },
  { value: 'dependency', label: 'Dependency', description: 'Track a specific package' },
  { value: 'topic', label: 'Topic', description: 'Match topic area' },
  { value: 'author', label: 'Author', description: 'Track a specific author' },
] as const;

const QueryItem = memo(function QueryItem({
  query,
  onDelete,
  onToggle,
  onViewMatches,
}: {
  query: StandingQuery;
  onDelete: (id: number) => void;
  onToggle: (id: number) => void;
  onViewMatches: (id: number) => void;
}) {
  return (
    <div className={`flex items-center justify-between p-3 border-b border-border/30 ${
      query.active ? '' : 'opacity-50'
    }`}>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-xs px-1.5 py-0.5 rounded bg-bg-tertiary text-text-muted uppercase tracking-wider font-mono">
            {query.keywords.length > 0 ? query.keywords[0] : 'keyword'}
          </span>
          <span className="text-sm font-medium text-text-primary truncate">
            {query.query_text}
          </span>
        </div>
        <div className="flex items-center gap-3 mt-1 text-xs text-text-muted">
          <span>{query.total_matches} match{query.total_matches !== 1 ? 'es' : ''}</span>
          {query.new_matches > 0 && (
            <span className="text-[#22C55E]">{query.new_matches} new</span>
          )}
          {query.last_run && (
            <span>Last run: {new Date(query.last_run).toLocaleDateString()}</span>
          )}
        </div>
      </div>
      <div className="flex items-center gap-1 ml-2">
        {query.total_matches > 0 && (
          <button
            onClick={() => onViewMatches(query.id)}
            className="text-xs px-2 py-1 rounded text-text-muted hover:text-text-secondary transition-colors"
          >
            View
          </button>
        )}
        <button
          onClick={() => onToggle(query.id)}
          className="text-xs px-2 py-1 rounded text-text-muted hover:text-text-secondary transition-colors"
        >
          {query.active ? 'Pause' : 'Resume'}
        </button>
        <button
          onClick={() => onDelete(query.id)}
          className="text-xs px-2 py-1 rounded text-text-muted hover:text-error transition-colors"
        >
          Remove
        </button>
      </div>
    </div>
  );
});

export default function StandingQueries() {
  const [queries, setQueries] = useState<StandingQuery[]>([]);
  const [newQuery, setNewQuery] = useState('');
  const [queryType, setQueryType] = useState<string>('keyword');
  const [loading, setLoading] = useState(true);
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedMatches, setSelectedMatches] = useState<StandingQueryMatch[]>([]);
  const [selectedQueryId, setSelectedQueryId] = useState<number | null>(null);
  const [matchesLoading, setMatchesLoading] = useState(false);

  // Load standing queries from backend
  const loadQueries = useCallback(() => {
    setLoading(true);
    setError(null);
    cmd('list_standing_queries')
      .then(result => {
        setQueries(result);
      })
      .catch(err => {
        console.error('Failed to load standing queries:', err);
        setQueries([]);
        setError('Failed to load watches');
      })
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => { loadQueries(); }, [loadQueries]);

  const handleAdd = useCallback(async () => {
    if (!newQuery.trim()) return;
    setAdding(true);
    setError(null);
    try {
      // Prefix with query type for the backend to parse
      const queryText = queryType !== 'keyword'
        ? `${queryType}:${newQuery.trim()}`
        : newQuery.trim();

      await cmd('create_standing_query', { queryText });
      setNewQuery('');
      // Reload the full list to get the backend-generated entry
      loadQueries();
    } catch (err) {
      console.error('Failed to create standing query:', err);
      setError('Failed to create watch');
    } finally {
      setAdding(false);
    }
  }, [newQuery, queryType, loadQueries]);

  const handleDelete = useCallback(async (id: number) => {
    try {
      await cmd('delete_standing_query', { id });
      setQueries(prev => prev.filter(q => q.id !== id));
      // Clear matches panel if viewing this query
      if (selectedQueryId === id) {
        setSelectedQueryId(null);
        setSelectedMatches([]);
      }
    } catch (err) {
      console.error('Failed to delete standing query:', err);
      setError('Failed to remove watch');
    }
  }, [selectedQueryId]);

  const handleToggle = useCallback((id: number) => {
    // Toggle is optimistic in the UI — backend doesn't have a toggle command yet
    // so we just update local state. This will be reflected on next full reload.
    setQueries(prev =>
      prev.map(q => (q.id === id ? { ...q, active: !q.active } : q)),
    );
  }, []);

  const handleViewMatches = useCallback(async (id: number) => {
    setSelectedQueryId(id);
    setMatchesLoading(true);
    try {
      const matches = await cmd('get_standing_query_matches', { id, limit: 20 });
      setSelectedMatches(matches);
    } catch (err) {
      console.error('Failed to load matches:', err);
      setSelectedMatches([]);
    } finally {
      setMatchesLoading(false);
    }
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-48 text-text-muted text-sm">
        Loading watches...
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Error banner */}
      {error && (
        <div className="px-4 py-2 bg-[#EF4444]/10 border-b border-[#EF4444]/20">
          <p className="text-xs text-[#EF4444]">{error}</p>
        </div>
      )}

      {/* Add new query */}
      <div className="p-4 border-b border-border/50">
        <div className="flex gap-2">
          <select
            value={queryType}
            onChange={e => setQueryType(e.target.value)}
            className="text-xs px-2 py-1.5 rounded bg-bg-tertiary border border-border/50 text-text-secondary focus:outline-none focus:border-text-muted"
          >
            {QUERY_TYPES.map(t => (
              <option key={t.value} value={t.value}>
                {t.label}
              </option>
            ))}
          </select>
          <input
            type="text"
            value={newQuery}
            onChange={e => setNewQuery(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleAdd()}
            placeholder="Watch for..."
            className="flex-1 text-sm px-3 py-1.5 rounded bg-bg-tertiary border border-border/50 text-text-primary placeholder:text-text-muted/50 focus:outline-none focus:border-text-muted"
          />
          <button
            onClick={handleAdd}
            disabled={adding || !newQuery.trim()}
            className="text-xs px-3 py-1.5 rounded bg-white/5 border border-border/50 text-text-secondary hover:bg-white/10 hover:text-text-primary transition-colors disabled:opacity-30"
          >
            {adding ? 'Adding...' : 'Watch'}
          </button>
        </div>
        <p className="text-xs text-text-muted mt-2">
          {QUERY_TYPES.find(t => t.value === queryType)?.description}
        </p>
      </div>

      {/* Query list */}
      <div className="flex-1 overflow-y-auto">
        {queries.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-48 text-center px-6">
            <p className="text-text-muted text-sm mb-2">No watches yet</p>
            <p className="text-text-muted/60 text-xs leading-relaxed">
              Add a watch to track specific keywords, dependencies, topics, or authors.
              4DA will boost matching content and notify you of matches.
            </p>
          </div>
        ) : (
          queries.map(q => (
            <QueryItem
              key={q.id}
              query={q}
              onDelete={handleDelete}
              onToggle={handleToggle}
              onViewMatches={handleViewMatches}
            />
          ))
        )}
      </div>

      {/* Match detail panel */}
      {selectedQueryId !== null && (
        <div className="border-t border-border/50 p-4">
          <div className="flex items-center justify-between mb-2">
            <h4 className="text-xs font-medium text-text-secondary uppercase tracking-wider">
              Matches
            </h4>
            <button
              onClick={() => { setSelectedQueryId(null); setSelectedMatches([]); }}
              className="text-xs text-text-muted hover:text-text-secondary"
            >
              Close
            </button>
          </div>
          {matchesLoading ? (
            <p className="text-xs text-text-muted">Loading matches...</p>
          ) : selectedMatches.length === 0 ? (
            <p className="text-xs text-text-muted">No matches recorded yet.</p>
          ) : (
            <div className="space-y-1">
              {selectedMatches.map(m => (
                <div key={m.item_id} className="py-1.5 flex items-center justify-between">
                  <div className="min-w-0 flex-1">
                    {m.url ? (
                      <a
                        href={m.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-xs text-text-secondary hover:text-white transition-colors truncate block"
                      >
                        {m.title || `Item #${m.item_id}`}
                      </a>
                    ) : (
                      <span className="text-xs text-text-secondary truncate block">
                        {m.title || `Item #${m.item_id}`}
                      </span>
                    )}
                  </div>
                  <div className="flex items-center gap-2 ml-2 shrink-0">
                    <span className="text-xs text-text-muted/60">{m.source_type}</span>
                    {m.discovered_at && (
                      <span className="text-xs text-text-muted/40">
                        {new Date(m.discovered_at).toLocaleDateString()}
                      </span>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
