// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

interface CapabilityInfo {
  state: 'full' | 'degraded' | 'unavailable';
  reason?: string;
  since?: string;
  fallback?: string;
  remediation?: string;
}

interface CapSummary {
  full: number;
  degraded: number;
  unavailable: number;
  total: number;
}

const CAPABILITY_LABELS: Record<string, string> = {
  embedding_search: 'Semantic Search',
  llm_reranking: 'AI Re-ranking',
  briefing_generation: 'Intelligence Briefing',
  source_fetching: 'Content Sources',
  ace_context: 'Project Context',
  system_tray: 'System Tray',
  notifications: 'Notifications',
  credential_storage: 'Secure Storage',
  vector_search: 'Vector Database',
};

function StateIcon({ state }: { state: string }) {
  if (state === 'full') return <span className="w-2 h-2 rounded-full bg-green-500 flex-shrink-0" />;
  if (state === 'degraded') return <span className="w-2 h-2 rounded-full bg-yellow-500 flex-shrink-0" />;
  return <span className="w-2 h-2 rounded-full bg-red-500 flex-shrink-0" />;
}

function CapabilityRow({ name, info }: { name: string; info: CapabilityInfo }) {
  const label = CAPABILITY_LABELS[name] ?? name;
  return (
    <div className="flex items-start gap-2 py-1.5">
      <StateIcon state={info.state} />
      <div className="flex-1 min-w-0">
        <div className="flex items-center justify-between">
          <span className="text-xs text-white">{label}</span>
          <span className={`text-[10px] capitalize ${
            info.state === 'full' ? 'text-green-400' :
            info.state === 'degraded' ? 'text-yellow-400' : 'text-red-400'
          }`}>
            {info.state}
          </span>
        </div>
        {info.state === 'degraded' && info.fallback && (
          <p className="text-[10px] text-text-muted mt-0.5">{info.fallback}</p>
        )}
        {info.state === 'unavailable' && info.remediation && (
          <p className="text-[10px] text-text-muted mt-0.5">{info.remediation}</p>
        )}
      </div>
    </div>
  );
}

export function SystemHealthSection() {
  const { t } = useTranslation();
  const [capabilities, setCapabilities] = useState<Record<string, CapabilityInfo> | null>(null);
  const [summary, setSummary] = useState<CapSummary | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const [states, sum] = await Promise.all([
        cmd('get_capability_states'),
        cmd('get_capability_summary'),
      ]);
      setCapabilities(states as unknown as Record<string, CapabilityInfo>);
      setSummary(sum);
    } catch {
      // Non-fatal
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  // Auto-refresh every 30 seconds
  useEffect(() => {
    const interval = setInterval(() => { void refresh(); }, 30_000);
    return () => clearInterval(interval);
  }, [refresh]);

  if (loading) {
    return (
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <div className="text-xs text-text-muted">Loading system health...</div>
      </div>
    );
  }

  const overallState = summary
    ? summary.unavailable > 0 ? 'degraded' : summary.degraded > 0 ? 'partial' : 'healthy'
    : 'unknown';

  const overallColor = overallState === 'healthy' ? 'text-green-400' :
    overallState === 'partial' ? 'text-yellow-400' : 'text-red-400';

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      {/* Header */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-green-500/20 rounded-lg flex items-center justify-center">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-green-400">
              <path d="M8 1l2.5 5 5.5.8-4 3.9.9 5.3L8 13.5 3.1 16l.9-5.3-4-3.9L5.5 6z" fill="currentColor" opacity="0.7" />
            </svg>
          </div>
          <div>
            <h3 className="text-sm font-medium text-white">
              {t('settings.systemHealth.title', 'System Health')}
            </h3>
            <p className="text-xs text-text-muted">
              {t('settings.systemHealth.description', 'Capability status and fallback states')}
            </p>
          </div>
        </div>
        {summary && (
          <span className={`text-xs font-medium ${overallColor}`}>
            {summary.full}/{summary.total} {t('settings.systemHealth.operational', 'operational')}
          </span>
        )}
      </div>

      {/* Capability list */}
      {capabilities && (
        <div className="p-3 bg-bg-secondary rounded-lg border border-border divide-y divide-border/50">
          {Object.entries(capabilities).map(([name, info]) => (
            <CapabilityRow key={name} name={name} info={info as CapabilityInfo} />
          ))}
        </div>
      )}

      {/* Refresh button */}
      <button
        onClick={() => { void refresh(); }}
        className="mt-3 text-[10px] text-text-muted hover:text-text-secondary transition-colors"
        title="Check system capabilities: embeddings, storage, notifications"
      >
        {t('settings.systemHealth.refresh', 'Refresh status')}
      </button>
    </div>
  );
}
