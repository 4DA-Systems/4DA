import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

// --- Types ---

interface HttpProbeRequest {
  method: string;
  url: string;
  headers: [string, string][];
  body: string | null;
}

interface HttpProbeResponse {
  status: number;
  status_text: string;
  headers: [string, string][];
  body: string;
  duration_ms: number;
  size_bytes: number;
}

interface HttpHistoryEntry {
  id: number;
  method: string;
  url: string;
  status: number;
  duration_ms: number;
  created_at: string;
}

// --- Helpers ---

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const;
const BODY_METHODS = new Set(['POST', 'PUT', 'PATCH']);

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1048576).toFixed(1)} MB`;
}

function statusColor(status: number): string {
  if (status >= 200 && status < 300) return 'text-[#22C55E]';
  if (status >= 300 && status < 400) return 'text-blue-400';
  if (status >= 400 && status < 500) return 'text-orange-400';
  if (status >= 500) return 'text-[#EF4444]';
  return 'text-[#A0A0A0]';
}

function statusBgColor(status: number): string {
  if (status >= 200 && status < 300) return 'bg-[#22C55E]/10';
  if (status >= 300 && status < 400) return 'bg-blue-400/10';
  if (status >= 400 && status < 500) return 'bg-orange-400/10';
  if (status >= 500) return 'bg-[#EF4444]/10';
  return 'bg-[#1F1F1F]';
}

function methodColor(method: string): string {
  switch (method) {
    case 'GET': return 'bg-[#22C55E]/15 text-[#22C55E]';
    case 'POST': return 'bg-blue-400/15 text-blue-400';
    case 'PUT': return 'bg-orange-400/15 text-orange-400';
    case 'PATCH': return 'bg-orange-400/15 text-orange-400';
    case 'DELETE': return 'bg-[#EF4444]/15 text-[#EF4444]';
    default: return 'bg-[#1F1F1F] text-[#A0A0A0]';
  }
}

// --- Component ---

export default function HttpProbe() {
  const [method, setMethod] = useState<string>('GET');
  const [url, setUrl] = useState('');
  const [headers, setHeaders] = useState<[string, string][]>([['', '']]);
  const [body, setBody] = useState('');

  const [loading, setLoading] = useState(false);
  const [response, setResponse] = useState<HttpProbeResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showResponseHeaders, setShowResponseHeaders] = useState(false);

  const [history, setHistory] = useState<HttpHistoryEntry[]>([]);
  const [showHistory, setShowHistory] = useState(true);

  // Fetch history on mount
  const fetchHistory = useCallback(async () => {
    try {
      const entries = await invoke<HttpHistoryEntry[]>('toolkit_get_http_history', {
        limit: 25,
      });
      setHistory(entries);
    } catch {
      // History is non-critical -- silently ignore
    }
  }, []);

  useEffect(() => {
    fetchHistory();
  }, [fetchHistory]);

  // Send request
  const sendRequest = useCallback(async () => {
    if (!url.trim()) return;
    setLoading(true);
    setError(null);
    setResponse(null);

    const cleanHeaders = headers.filter(
      ([k, v]) => k.trim() !== '' || v.trim() !== '',
    );

    const request: HttpProbeRequest = {
      method,
      url: url.trim(),
      headers: cleanHeaders,
      body: BODY_METHODS.has(method) ? body || null : null,
    };

    try {
      const res = await invoke<HttpProbeResponse>('toolkit_http_request', {
        request,
      });
      setResponse(res);
      fetchHistory();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [method, url, headers, body, fetchHistory]);

  // Header row management
  const updateHeader = (index: number, col: 0 | 1, value: string) => {
    setHeaders((prev) => {
      const next = [...prev];
      const row: [string, string] = [...next[index]];
      row[col] = value;
      next[index] = row;
      return next;
    });
  };

  const addHeader = () => setHeaders((prev) => [...prev, ['', '']]);

  const removeHeader = (index: number) => {
    setHeaders((prev) => (prev.length <= 1 ? [['', '']] : prev.filter((_, i) => i !== index)));
  };

  // Replay from history
  const replayEntry = (entry: HttpHistoryEntry) => {
    setMethod(entry.method);
    setUrl(entry.url);
    setHeaders([['', '']]);
    setBody('');
    setResponse(null);
    setError(null);
  };

  // Keyboard shortcut: Ctrl+Enter to send
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !loading) {
      e.preventDefault();
      sendRequest();
    }
  };

  return (
    <div className="flex gap-4 h-full min-h-0" onKeyDown={handleKeyDown}>
      {/* Main panel */}
      <div className="flex-1 flex flex-col gap-4 min-w-0">
        {/* Method + URL + Send */}
        <div className="flex items-center gap-2">
          <select
            value={method}
            onChange={(e) => setMethod(e.target.value)}
            className="bg-[#1F1F1F] border border-[#2A2A2A] text-white text-sm font-mono rounded px-2 py-2 outline-none focus:border-white/30 appearance-none cursor-pointer"
          >
            {METHODS.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>

          <input
            type="text"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://api.example.com/endpoint"
            spellCheck={false}
            className="flex-1 bg-[#1F1F1F] border border-[#2A2A2A] text-white text-sm font-mono rounded px-3 py-2 outline-none focus:border-white/30 placeholder:text-[#666]"
          />

          <button
            onClick={sendRequest}
            disabled={loading || !url.trim()}
            className="px-5 py-2 bg-white text-black text-sm font-medium rounded hover:bg-gray-200 transition-colors disabled:opacity-40 disabled:cursor-not-allowed flex items-center gap-2 shrink-0"
          >
            {loading ? (
              <>
                <span className="w-3 h-3 border-2 border-black/30 border-t-black rounded-full animate-spin" />
                Sending...
              </>
            ) : (
              'Send'
            )}
          </button>
        </div>

        {/* Headers editor */}
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between">
            <label className="text-[#A0A0A0] text-xs uppercase tracking-wide font-medium">
              Headers
            </label>
            <button
              onClick={addHeader}
              className="text-xs text-[#A0A0A0] hover:text-white transition-colors"
            >
              + Add Header
            </button>
          </div>
          {headers.map(([key, val], i) => (
            <div key={i} className="flex items-center gap-2">
              <input
                type="text"
                value={key}
                onChange={(e) => updateHeader(i, 0, e.target.value)}
                placeholder="Header name"
                spellCheck={false}
                className="flex-1 bg-[#1F1F1F] border border-[#2A2A2A] text-white text-xs font-mono rounded px-2 py-1.5 outline-none focus:border-white/30 placeholder:text-[#666]"
              />
              <input
                type="text"
                value={val}
                onChange={(e) => updateHeader(i, 1, e.target.value)}
                placeholder="Value"
                spellCheck={false}
                className="flex-1 bg-[#1F1F1F] border border-[#2A2A2A] text-white text-xs font-mono rounded px-2 py-1.5 outline-none focus:border-white/30 placeholder:text-[#666]"
              />
              <button
                onClick={() => removeHeader(i)}
                className="text-[#666] hover:text-[#EF4444] transition-colors text-xs px-1.5 py-1"
                title="Remove header"
              >
                x
              </button>
            </div>
          ))}
        </div>

        {/* Body editor (only for methods that support a body) */}
        {BODY_METHODS.has(method) && (
          <div className="flex flex-col gap-1.5">
            <label className="text-[#A0A0A0] text-xs uppercase tracking-wide font-medium">
              Body
            </label>
            <textarea
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder='{"key": "value"}'
              spellCheck={false}
              className="w-full h-32 bg-[#1F1F1F] border border-[#2A2A2A] text-white text-xs font-mono rounded px-3 py-2 outline-none focus:border-white/30 placeholder:text-[#666] resize-y"
            />
          </div>
        )}

        {/* Error banner */}
        {error && (
          <div className="bg-[#EF4444]/10 border border-[#EF4444]/30 rounded px-3 py-2 text-[#EF4444] text-xs font-mono">
            {error}
          </div>
        )}

        {/* Response display */}
        {response && (
          <div className="flex flex-col gap-3 min-h-0">
            {/* Status bar */}
            <div className="flex items-center gap-3 flex-wrap">
              <span
                className={`text-sm font-mono font-semibold px-2 py-0.5 rounded ${statusColor(response.status)} ${statusBgColor(response.status)}`}
              >
                {response.status} {response.status_text}
              </span>
              <span className="text-xs text-[#A0A0A0] font-mono">
                {response.duration_ms} ms
              </span>
              <span className="text-xs text-[#A0A0A0] font-mono">
                {formatBytes(response.size_bytes)}
              </span>
            </div>

            {/* Response headers (collapsible) */}
            <div>
              <button
                onClick={() => setShowResponseHeaders((p) => !p)}
                className="text-xs text-[#A0A0A0] hover:text-white transition-colors flex items-center gap-1"
              >
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  className={`transition-transform ${showResponseHeaders ? 'rotate-90' : ''}`}
                >
                  <path d="M9 18l6-6-6-6" />
                </svg>
                Response Headers ({response.headers.length})
              </button>
              {showResponseHeaders && (
                <div className="mt-1.5 bg-[#141414] border border-[#2A2A2A] rounded p-2 max-h-40 overflow-auto">
                  {response.headers.map(([k, v], i) => (
                    <div key={i} className="text-xs font-mono py-0.5">
                      <span className="text-[#D4AF37]">{k}</span>
                      <span className="text-[#666]">: </span>
                      <span className="text-[#A0A0A0]">{v}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Response body */}
            <div className="flex flex-col gap-1.5 min-h-0 flex-1">
              <label className="text-[#A0A0A0] text-xs uppercase tracking-wide font-medium">
                Response Body
              </label>
              <pre className="bg-[#141414] border border-[#2A2A2A] rounded p-3 text-xs font-mono text-white overflow-auto max-h-72 whitespace-pre-wrap break-words">
                {response.body}
              </pre>
            </div>
          </div>
        )}
      </div>

      {/* History sidebar */}
      <div className="w-64 shrink-0 flex flex-col min-h-0 border-l border-[#2A2A2A] pl-4">
        <button
          onClick={() => setShowHistory((p) => !p)}
          className="flex items-center justify-between w-full mb-2"
        >
          <span className="text-[#A0A0A0] text-xs uppercase tracking-wide font-medium">
            History
          </span>
          <svg
            width="10"
            height="10"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className={`text-[#666] transition-transform ${showHistory ? 'rotate-90' : ''}`}
          >
            <path d="M9 18l6-6-6-6" />
          </svg>
        </button>

        {showHistory && (
          <div className="flex-1 overflow-auto space-y-1">
            {history.length === 0 && (
              <p className="text-xs text-[#666] py-4 text-center">
                No requests yet
              </p>
            )}
            {history.map((entry) => (
              <button
                key={entry.id}
                onClick={() => replayEntry(entry)}
                className="w-full text-left bg-[#141414] hover:bg-[#1F1F1F] border border-[#2A2A2A] rounded px-2 py-1.5 transition-colors group"
              >
                <div className="flex items-center gap-1.5 mb-0.5">
                  <span
                    className={`text-[10px] font-mono font-semibold px-1 py-0.5 rounded ${methodColor(entry.method)}`}
                  >
                    {entry.method}
                  </span>
                  <span
                    className={`text-[10px] font-mono ${statusColor(entry.status)}`}
                  >
                    {entry.status}
                  </span>
                  <span className="text-[10px] text-[#666] font-mono ml-auto">
                    {entry.duration_ms}ms
                  </span>
                </div>
                <div className="text-[11px] text-[#A0A0A0] font-mono truncate group-hover:text-white transition-colors">
                  {entry.url}
                </div>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
