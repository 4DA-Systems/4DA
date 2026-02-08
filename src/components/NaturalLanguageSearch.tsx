import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface QueryResultItem {
  id: number;
  file_path: string | null;
  file_name: string | null;
  preview: string;
  relevance: number;
  source_type: string;
  timestamp: string | null;
  match_reason: string;
}

interface QueryResult {
  query: string;
  intent: string;
  items: QueryResultItem[];
  total_count: number;
  execution_ms: number;
  summary: string | null;
  parsed: {
    keywords: string[];
    entities: string[];
    time_range: {
      start: string;
      end: string;
      relative: string | null;
    } | null;
    file_types: string[];
    sentiment: string | null;
    confidence: number;
  };
}

const intentIcons: Record<string, string> = {
  Find: '🔍',
  Summarize: '📝',
  Compare: '⚖️',
  Timeline: '📅',
  Count: '#️⃣',
};

const sourceIcons: Record<string, string> = {
  pdf: '📄',
  docx: '📝',
  xlsx: '📊',
  image: '🖼️',
  context: '📁',
};

interface NaturalLanguageSearchProps {
  onStatusChange?: (status: string) => void;
  defaultExpanded?: boolean;
}

export function NaturalLanguageSearch({ onStatusChange, defaultExpanded = true }: NaturalLanguageSearchProps) {
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<QueryResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState(defaultExpanded);

  const handleSearch = async () => {
    if (!query.trim()) return;

    setLoading(true);
    try {
      const searchResult = await invoke<QueryResult>('natural_language_query', {
        queryText: query,
      });
      setResult(searchResult);
      onStatusChange?.(`Found ${searchResult.total_count} results in ${searchResult.execution_ms}ms`);
    } catch (error) {
      console.error('Search failed:', error);
      onStatusChange?.(`Search error: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSearch();
    }
  };

  const clearResults = () => {
    setResult(null);
    setQuery('');
  };

  return (
    <div className="bg-[#1F1F1F] rounded-lg p-5 border border-[#2A2A2A]">
      <div
        className="flex items-center justify-between cursor-pointer"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-cyan-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
            <span className="text-cyan-400">💬</span>
          </div>
          <div>
            <h3 className="text-white font-medium">Natural Language Search</h3>
            <p className="text-gray-500 text-sm">Ask questions about your indexed files</p>
          </div>
        </div>
        <span className="text-gray-500 text-sm">{expanded ? '▼' : '▶'}</span>
      </div>

      {expanded && (
        <div className="mt-4 space-y-4">
          {/* Search input */}
          <div className="flex gap-2">
            <input
              type="text"
              placeholder="Ask anything... e.g., 'files about rust from last week'"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={handleKeyDown}
              className="flex-1 px-4 py-3 text-sm bg-[#141414] border border-[#2A2A2A] rounded-lg text-white placeholder:text-gray-500 focus:outline-none focus:border-cyan-500/50 transition-colors"
            />
            <button
              onClick={handleSearch}
              disabled={loading || !query.trim()}
              className="px-5 py-3 text-sm bg-cyan-500/20 border border-cyan-500/30 text-cyan-400 rounded-lg hover:bg-cyan-500/30 disabled:opacity-50 disabled:cursor-not-allowed transition-all font-medium"
            >
              {loading ? '...' : 'Search'}
            </button>
          </div>

          {/* Example queries */}
          {!result && (
            <div className="space-y-2">
              <span className="text-xs text-gray-400 font-medium">Try these:</span>
              <div className="flex flex-wrap gap-2">
                {[
                  'show me files about authentication',
                  'pdfs from last month',
                  'summarize my notes on rust',
                  'what did I work on last week',
                ].map((example) => (
                  <button
                    key={example}
                    onClick={() => setQuery(example)}
                    className="px-3 py-1.5 text-xs bg-[#141414] rounded-lg border border-[#2A2A2A] text-gray-400 hover:text-cyan-400 hover:border-cyan-500/30 transition-all"
                  >
                    {example}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Results */}
          {result && (
            <div className="space-y-4">
              {/* Query parsing info */}
              <div className="flex items-center gap-2 p-3 bg-[#141414] rounded-lg border border-[#2A2A2A]">
                <span className="text-lg">{intentIcons[result.intent] || '🔍'}</span>
                <span className="text-sm text-gray-400">
                  {result.intent} query
                </span>
                <span className="text-sm text-white">•</span>
                <span className="text-sm text-cyan-400">
                  {result.parsed.keywords.join(', ')}
                </span>
                {result.parsed.time_range && (
                  <span className="px-2 py-1 text-xs bg-[#1F1F1F] rounded-md text-gray-300 border border-[#2A2A2A]">
                    {result.parsed.time_range.relative || 'custom range'}
                  </span>
                )}
                {result.parsed.file_types.length > 0 && (
                  <span className="px-2 py-1 text-xs bg-[#1F1F1F] rounded-md text-gray-300 border border-[#2A2A2A]">
                    {result.parsed.file_types.join(', ')}
                  </span>
                )}
                <button
                  onClick={clearResults}
                  className="ml-auto text-gray-500 hover:text-white transition-colors"
                >
                  ✕
                </button>
              </div>

              {/* Summary */}
              {result.summary && (
                <div className="text-sm text-gray-300 bg-[#141414] rounded-lg p-4 border border-[#2A2A2A]">
                  {result.summary}
                </div>
              )}

              {/* Result items */}
              <div className="space-y-2 max-h-64 overflow-y-auto">
                {result.items.map((item, index) => (
                  <div
                    key={`${item.id}-${index}`}
                    className="p-3 bg-[#141414] rounded-lg border border-[#2A2A2A] hover:border-cyan-500/30 transition-colors"
                  >
                    <div className="flex items-start gap-3">
                      <span className="text-lg">
                        {sourceIcons[item.source_type] || '📁'}
                      </span>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="text-sm text-white font-medium truncate">
                            {item.file_name || 'Unknown file'}
                          </span>
                          <span className={`text-xs px-2 py-0.5 rounded-md ${
                            item.relevance > 0.7
                              ? 'bg-green-500/20 text-green-400'
                              : item.relevance > 0.4
                              ? 'bg-yellow-500/20 text-yellow-400'
                              : 'bg-gray-500/20 text-gray-400'
                          }`}>
                            {(item.relevance * 100).toFixed(0)}%
                          </span>
                        </div>
                        <p className="text-xs text-gray-500 mt-1 line-clamp-2">
                          {item.preview}
                        </p>
                        <div className="flex items-center gap-2 mt-2 text-[10px] text-gray-500">
                          <span className="text-cyan-400/70">{item.match_reason}</span>
                          {item.timestamp && (
                            <>
                              <span>•</span>
                              <span>{new Date(item.timestamp).toLocaleDateString()}</span>
                            </>
                          )}
                        </div>
                      </div>
                    </div>
                  </div>
                ))}

                {result.items.length === 0 && (
                  <div className="text-center py-6 bg-[#141414] rounded-lg border border-[#2A2A2A]">
                    <div className="text-2xl mb-2">🔍</div>
                    <div className="text-sm text-gray-400">No results found</div>
                    <div className="text-xs text-gray-500 mt-1">Try different keywords or a broader query</div>
                  </div>
                )}
              </div>

              {/* Stats footer */}
              <div className="text-xs text-gray-500 text-center pt-2 border-t border-[#2A2A2A]">
                {result.total_count} results • {result.execution_ms}ms • confidence: {(result.parsed.confidence * 100).toFixed(0)}%
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
