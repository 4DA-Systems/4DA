import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { useVisibilityPolling } from '../../../hooks/use-visibility-polling';

interface ListeningPort {
  port: number;
  protocol: string;
  pid: number;
  process_name: string;
  address: string;
}

const DEV_PORTS = new Set([3000, 4444, 5173, 5432, 6379, 8080, 8443, 9090]);

const DEV_PORT_LABELS: Record<number, string> = {
  3000: 'React/Next',
  4444: '4DA',
  5173: 'Vite',
  5432: 'Postgres',
  6379: 'Redis',
  8080: 'HTTP Alt',
  8443: 'HTTPS Alt',
  9090: 'Prometheus',
};

export default function PortScanner() {
  const { t } = useTranslation();
  const [ports, setPorts] = useState<ListeningPort[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState('');
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [killConfirm, setKillConfirm] = useState<number | null>(null);
  const [killMessage, setKillMessage] = useState<string | null>(null);


  const scan = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<ListeningPort[]>('toolkit_list_ports');
      const sorted = [...result].sort((a, b) => a.port - b.port);
      setPorts(sorted);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  const killProcess = useCallback(async (pid: number) => {
    try {
      const msg = await invoke<string>('toolkit_kill_process', { pid });
      setKillMessage(msg);
      setKillConfirm(null);
      // Refresh the list after kill
      await scan();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setKillConfirm(null);
    }
  }, [scan]);

  // Visibility-aware auto-refresh polling — pauses when app is backgrounded
  useVisibilityPolling(scan, 5000, autoRefresh);

  // Clear kill message after 3 seconds
  useEffect(() => {
    if (!killMessage) return;
    const t = setTimeout(() => setKillMessage(null), 3000);
    return () => clearTimeout(t);
  }, [killMessage]);

  // Clear kill confirmation if user doesn't act within 3 seconds
  useEffect(() => {
    if (killConfirm === null) return;
    const t = setTimeout(() => setKillConfirm(null), 3000);
    return () => clearTimeout(t);
  }, [killConfirm]);

  const filterLower = filter.toLowerCase();
  const filtered = ports.filter(
    (p) =>
      String(p.port).includes(filterLower) ||
      p.process_name.toLowerCase().includes(filterLower) ||
      p.address.toLowerCase().includes(filterLower),
  );

  const devCount = filtered.filter((p) => DEV_PORTS.has(p.port)).length;

  return (
    <div className="space-y-4">
      {/* Controls */}
      <div className="flex items-center gap-3 flex-wrap">
        <button
          onClick={scan}
          disabled={loading}
          className="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-white text-[#0A0A0A] rounded-lg hover:bg-white/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {loading ? (
            <>
              <div className="w-3.5 h-3.5 border-2 border-[#0A0A0A]/30 border-t-[#0A0A0A] rounded-full animate-spin" />
              {t('toolkit.portScanner.scanning')}
            </>
          ) : (
            <>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="11" cy="11" r="8" />
                <path d="m21 21-4.3-4.3" />
              </svg>
              {t('toolkit.portScanner.scanPorts')}
            </>
          )}
        </button>

        <input
          type="text"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder={t('toolkit.portScanner.filterPlaceholder')}
          className="flex-1 min-w-[200px] px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-white placeholder-[#666] focus:outline-none focus:border-white/30 font-mono"
        />

        <label className="flex items-center gap-2 text-xs text-text-secondary cursor-pointer select-none">
          <button
            onClick={() => setAutoRefresh((v) => !v)}
            className={`w-8 h-[18px] rounded-full relative transition-colors ${
              autoRefresh ? 'bg-[#22C55E]' : 'bg-bg-tertiary border border-border'
            }`}
          >
            <span
              className={`absolute top-[2px] w-[14px] h-[14px] rounded-full bg-white transition-all ${
                autoRefresh ? 'left-[14px]' : 'left-[2px]'
              }`}
            />
          </button>
          {t('toolkit.portScanner.autoRefresh')}
        </label>
      </div>

      {/* Error banner */}
      {error && (
        <div className="flex items-center gap-3 px-4 py-3 bg-[#EF4444]/10 border border-[#EF4444]/30 rounded-lg">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#EF4444" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <span className="text-sm text-[#EF4444] flex-1">{error}</span>
          <button onClick={() => setError(null)} className="text-[#EF4444]/60 hover:text-[#EF4444] text-xs">
            {t('action.dismiss')}
          </button>
        </div>
      )}

      {/* Kill success message */}
      {killMessage && (
        <div className="flex items-center gap-3 px-4 py-3 bg-[#22C55E]/10 border border-[#22C55E]/30 rounded-lg">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#22C55E" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M20 6 9 17l-5-5" />
          </svg>
          <span className="text-sm text-[#22C55E]">{killMessage}</span>
        </div>
      )}

      {/* Stats bar */}
      {ports.length > 0 && (
        <div className="flex items-center gap-4 text-xs text-text-secondary">
          <span>{t('toolkit.portScanner.portsListening', { count: filtered.length })}</span>
          {devCount > 0 && (
            <span className="text-[#D4AF37]">
              {t('toolkit.portScanner.devPortsActive', { count: devCount })}
            </span>
          )}
          {filter && filtered.length !== ports.length && (
            <span className="text-[#666]">
              ({ports.length} total, {ports.length - filtered.length} hidden by filter)
            </span>
          )}
        </div>
      )}

      {/* Table */}
      {filtered.length > 0 ? (
        <div className="border border-border rounded-xl overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-bg-secondary text-text-secondary text-xs uppercase tracking-wider">
                <th className="text-left px-4 py-2.5 font-medium">Port</th>
                <th className="text-left px-4 py-2.5 font-medium">Protocol</th>
                <th className="text-left px-4 py-2.5 font-medium">Process</th>
                <th className="text-left px-4 py-2.5 font-medium">PID</th>
                <th className="text-left px-4 py-2.5 font-medium">Address</th>
                <th className="text-right px-4 py-2.5 font-medium w-20"></th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((p) => {
                const isDev = DEV_PORTS.has(p.port);
                const isConfirming = killConfirm === p.pid;
                return (
                  <tr
                    key={`${p.port}-${p.pid}-${p.protocol}`}
                    className={`border-t border-border transition-colors ${
                      isDev ? 'bg-[#D4AF37]/5 hover:bg-[#D4AF37]/10' : 'hover:bg-bg-tertiary'
                    }`}
                  >
                    <td className="px-4 py-2.5 font-mono text-white">
                      <span className="flex items-center gap-2">
                        {p.port}
                        {isDev && (
                          <span className="text-[10px] px-1.5 py-0.5 rounded bg-[#D4AF37]/15 text-[#D4AF37] font-sans">
                            {DEV_PORT_LABELS[p.port] || 'DEV'}
                          </span>
                        )}
                      </span>
                    </td>
                    <td className="px-4 py-2.5 font-mono text-text-secondary uppercase text-xs">
                      {p.protocol}
                    </td>
                    <td className="px-4 py-2.5 text-white truncate max-w-[200px]" title={p.process_name}>
                      {p.process_name}
                    </td>
                    <td className="px-4 py-2.5 font-mono text-text-secondary">
                      {p.pid}
                    </td>
                    <td className="px-4 py-2.5 font-mono text-[#666] text-xs">
                      {p.address}
                    </td>
                    <td className="px-4 py-2.5 text-right">
                      {isConfirming ? (
                        <button
                          onClick={() => killProcess(p.pid)}
                          className="px-2.5 py-1 text-xs bg-[#EF4444]/15 text-[#EF4444] border border-[#EF4444]/30 rounded-md hover:bg-[#EF4444]/25 transition-colors font-medium"
                        >
                          {t('toolkit.portScanner.killConfirm')}
                        </button>
                      ) : (
                        <button
                          onClick={() => setKillConfirm(p.pid)}
                          className="px-2.5 py-1 text-xs text-[#666] border border-border rounded-md hover:text-[#EF4444] hover:border-[#EF4444]/30 transition-colors"
                        >
                          {t('toolkit.portScanner.kill')}
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ) : ports.length === 0 && !loading ? (
        <div className="flex flex-col items-center justify-center py-16 text-[#666]">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="mb-3 opacity-50">
            <path d="M22 12h-4l-3 9L9 3l-3 9H2" />
          </svg>
          <p className="text-sm">{t('toolkit.portScanner.empty')}</p>
          <p className="text-xs mt-1">{t('toolkit.portScanner.emptyHint')}</p>
        </div>
      ) : filter && filtered.length === 0 ? (
        <div className="text-center py-10 text-[#666] text-sm">
          {t('toolkit.portScanner.noMatch', { filter })}
        </div>
      ) : null}
    </div>
  );
}
