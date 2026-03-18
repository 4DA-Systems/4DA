import { useState, useEffect, useCallback, memo } from 'react';

interface StandingQuery {
  id: number;
  query: string;
  query_type: string;
  created_at: string;
  last_match_at: string | null;
  match_count: number;
  active: boolean;
}

interface QueryMatch {
  id: number;
  query_id: number;
  source_item_id: number;
  matched_at: string;
  title?: string;
  url?: string;
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
            {query.query_type}
          </span>
          <span className="text-sm font-medium text-text-primary truncate">
            {query.query}
          </span>
        </div>
        <div className="flex items-center gap-3 mt-1 text-xs text-text-muted">
          <span>{query.match_count} match{query.match_count !== 1 ? 'es' : ''}</span>
          {query.last_match_at && (
            <span>Last: {new Date(query.last_match_at).toLocaleDateString()}</span>
          )}
        </div>
      </div>
      <div className="flex items-center gap-1 ml-2">
        {query.match_count > 0 && (
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
  const [selectedMatches, setSelectedMatches] = useState<QueryMatch[]>([]);
  const [selectedQueryId, setSelectedQueryId] = useState<number | null>(null);

  // Mock data for initial implementation — will be replaced with Tauri commands
  useEffect(() => {
    setLoading(true);
    // Simulated load
    setTimeout(() => {
      setQueries([]);
      setLoading(false);
    }, 300);
  }, []);

  const handleAdd = useCallback(async () => {
    if (!newQuery.trim()) return;
    setAdding(true);
    try {
      const query: StandingQuery = {
        id: Date.now(),
        query: newQuery.trim(),
        query_type: queryType,
        created_at: new Date().toISOString(),
        last_match_at: null,
        match_count: 0,
        active: true,
      };
      setQueries(prev => [query, ...prev]);
      setNewQuery('');
    } finally {
      setAdding(false);
    }
  }, [newQuery, queryType]);

  const handleDelete = useCallback((id: number) => {
    setQueries(prev => prev.filter(q => q.id !== id));
  }, []);

  const handleToggle = useCallback((id: number) => {
    setQueries(prev =>
      prev.map(q => (q.id === id ? { ...q, active: !q.active } : q)),
    );
  }, []);

  const handleViewMatches = useCallback((id: number) => {
    setSelectedQueryId(id);
    setSelectedMatches([]); // Would fetch from backend
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
            Watch
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
              onClick={() => setSelectedQueryId(null)}
              className="text-xs text-text-muted hover:text-text-secondary"
            >
              Close
            </button>
          </div>
          {selectedMatches.length === 0 ? (
            <p className="text-xs text-text-muted">No matches recorded yet.</p>
          ) : (
            selectedMatches.map(m => (
              <div key={m.id} className="py-1.5 text-xs text-text-secondary">
                {m.title || `Item #${m.source_item_id}`}
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}
