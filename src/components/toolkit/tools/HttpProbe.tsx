import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

import {
  METHODS,
  BODY_METHODS,
  formatBytes,
  statusColor,
  statusBgColor,
  methodColor,
} from './http-probe-utils';
import type { HttpProbeRequest, HttpProbeResponse, HttpHistoryEntry } from './http-probe-utils';

// --- Component ---

export default function HttpProbe() {
  const { t } = useTranslation();
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
            className="bg-bg-tertiary border border-border text-white text-sm font-mono rounded px-2 py-2 outline-none focus:border-white/30 appearance-none cursor-pointer"
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
            className="flex-1 bg-bg-tertiary border border-border text-white text-sm font-mono rounded px-3 py-2 outline-none focus:border-white/30 placeholder:text-[#666]"
          />

          <button
            onClick={sendRequest}
            disabled={loading || !url.trim()}
            className="px-5 py-2 bg-white text-black text-sm font-medium rounded hover:bg-gray-200 transition-colors disabled:opacity-40 disabled:cursor-not-allowed flex items-center gap-2 shrink-0"
          >
            {loading ? (
              <>
                <span className="w-3 h-3 border-2 border-black/30 border-t-black rounded-full animate-spin" />
                {t('toolkit.httpProbe.sending')}
              </>
            ) : (
              t('toolkit.httpProbe.send')
            )}
          </button>
        </div>

        {/* Headers editor */}
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between">
            <label className="text-text-secondary text-xs uppercase tracking-wide font-medium">
              {t('toolkit.httpProbe.headers')}
            </label>
            <button
              onClick={addHeader}
              className="text-xs text-text-secondary hover:text-white transition-colors"
            >
              {t('toolkit.httpProbe.addHeader')}
            </button>
          </div>
          {headers.map(([key, val], i) => (
            <div key={i} className="flex items-center gap-2">
              <input
                type="text"
                value={key}
                onChange={(e) => updateHeader(i, 0, e.target.value)}
                placeholder={t('toolkit.httpProbe.headerName')}
                spellCheck={false}
                className="flex-1 bg-bg-tertiary border border-border text-white text-xs font-mono rounded px-2 py-1.5 outline-none focus:border-white/30 placeholder:text-[#666]"
              />
              <input
                type="text"
                value={val}
                onChange={(e) => updateHeader(i, 1, e.target.value)}
                placeholder={t('toolkit.httpProbe.value')}
                spellCheck={false}
                className="flex-1 bg-bg-tertiary border border-border text-white text-xs font-mono rounded px-2 py-1.5 outline-none focus:border-white/30 placeholder:text-[#666]"
              />
              <button
                onClick={() => removeHeader(i)}
                className="text-[#666] hover:text-[#EF4444] transition-colors text-xs px-1.5 py-1"
                title={t('toolkit.httpProbe.removeHeader')}
              >
                x
              </button>
            </div>
          ))}
        </div>

        {/* Body editor (only for methods that support a body) */}
        {BODY_METHODS.has(method) && (
          <div className="flex flex-col gap-1.5">
            <label className="text-text-secondary text-xs uppercase tracking-wide font-medium">
              {t('toolkit.httpProbe.body')}
            </label>
            <textarea
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder='{"key": "value"}'
              spellCheck={false}
              className="w-full h-32 bg-bg-tertiary border border-border text-white text-xs font-mono rounded px-3 py-2 outline-none focus:border-white/30 placeholder:text-[#666] resize-y"
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
              <span className="text-xs text-text-secondary font-mono">
                {response.duration_ms} ms
              </span>
              <span className="text-xs text-text-secondary font-mono">
                {formatBytes(response.size_bytes)}
              </span>
            </div>

            {/* Response headers (collapsible) */}
            <div>
              <button
                onClick={() => setShowResponseHeaders((p) => !p)}
                className="text-xs text-text-secondary hover:text-white transition-colors flex items-center gap-1"
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
                {t('toolkit.httpProbe.responseHeaders', { count: response.headers.length })}
              </button>
              {showResponseHeaders && (
                <div className="mt-1.5 bg-bg-secondary border border-border rounded p-2 max-h-40 overflow-auto">
                  {response.headers.map(([k, v], i) => (
                    <div key={i} className="text-xs font-mono py-0.5">
                      <span className="text-[#D4AF37]">{k}</span>
                      <span className="text-[#666]">: </span>
                      <span className="text-text-secondary">{v}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Response body */}
            <div className="flex flex-col gap-1.5 min-h-0 flex-1">
              <label className="text-text-secondary text-xs uppercase tracking-wide font-medium">
                {t('toolkit.httpProbe.responseBody')}
              </label>
              <pre className="bg-bg-secondary border border-border rounded p-3 text-xs font-mono text-white overflow-auto max-h-72 whitespace-pre-wrap break-words">
                {response.body}
              </pre>
            </div>
          </div>
        )}
      </div>

      {/* History sidebar */}
      <div className="w-64 shrink-0 flex flex-col min-h-0 border-l border-border pl-4">
        <button
          onClick={() => setShowHistory((p) => !p)}
          className="flex items-center justify-between w-full mb-2"
        >
          <span className="text-text-secondary text-xs uppercase tracking-wide font-medium">
            {t('toolkit.httpProbe.history')}
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
                {t('toolkit.httpProbe.noRequests')}
              </p>
            )}
            {history.map((entry) => (
              <button
                key={entry.id}
                onClick={() => replayEntry(entry)}
                className="w-full text-left bg-bg-secondary hover:bg-bg-tertiary border border-border rounded px-2 py-1.5 transition-colors group"
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
                <div className="text-[11px] text-text-secondary font-mono truncate group-hover:text-white transition-colors">
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
