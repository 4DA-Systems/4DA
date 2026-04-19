// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { cmd } from '../../lib/commands';
import type { DataHealth, MaintenanceResult } from '../../types/autophagy';

type CleanState = 'idle' | 'cleaning' | 'done';

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function HealthBadge({ status }: { status: string }) {
  const { t } = useTranslation();
  const colors: Record<string, string> = {
    healthy: 'bg-green-500/20 text-green-400 border-green-500/30',
    growing: 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30',
    needs_attention: 'bg-red-500/20 text-red-400 border-red-500/30',
  };
  const labels: Record<string, string> = {
    healthy: t('settings.dataHealth.statusHealthy'),
    growing: t('settings.dataHealth.statusGrowing'),
    needs_attention: t('settings.dataHealth.statusNeedsAttention'),
  };
  return (
    <span className={`px-2 py-0.5 text-xs rounded border ${colors[status] ?? colors.healthy}`}>
      {labels[status] ?? status}
    </span>
  );
}

function StatRow({ label, value, muted }: { label: string; value: string | number; muted?: boolean }) {
  return (
    <div className="flex items-center justify-between py-1">
      <span className="text-xs text-text-muted">{label}</span>
      <span className={`text-xs font-mono ${muted === true ? 'text-text-muted' : 'text-text-secondary'}`}>{value}</span>
    </div>
  );
}

function CleanResultSummary({ result }: { result: MaintenanceResult }) {
  const { t } = useTranslation();
  const total = result.deleted_items + result.deleted_feedback + result.deleted_intelligence
    + result.deleted_windows + result.deleted_cycles + result.deleted_necessity + result.deleted_void;

  if (total === 0) {
    return <p className="text-xs text-text-muted mt-2">{t('settings.dataHealth.alreadyClean')}</p>;
  }

  const lines: string[] = [];
  if (result.deleted_items > 0) lines.push(t('settings.dataHealth.cleanItems', { count: result.deleted_items }));
  if (result.deleted_feedback > 0) lines.push(t('settings.dataHealth.cleanFeedback', { count: result.deleted_feedback }));
  if (result.deleted_intelligence > 0) lines.push(t('settings.dataHealth.cleanCalibrations', { count: result.deleted_intelligence }));
  if (result.deleted_windows > 0) lines.push(t('settings.dataHealth.cleanWindows', { count: result.deleted_windows }));
  if (result.deleted_cycles > 0) lines.push(t('settings.dataHealth.cleanCycles', { count: result.deleted_cycles }));
  if (result.deleted_necessity > 0) lines.push(t('settings.dataHealth.cleanScores', { count: result.deleted_necessity }));
  if (result.deleted_void > 0) lines.push(t('settings.dataHealth.cleanVoid', { count: result.deleted_void }));

  return (
    <div className="mt-2 p-2 bg-green-500/10 border border-green-500/20 rounded text-xs text-green-400">
      <p className="font-medium">{t('settings.dataHealth.cleanedRecords', { count: total })}{result.vacuumed ? ` + ${t('settings.dataHealth.compacted')}` : ''}</p>
      <p className="text-green-400/70 mt-0.5">{lines.join(', ')}</p>
    </div>
  );
}

export function DataHealthSection() {
  const { t } = useTranslation();
  const [health, setHealth] = useState<DataHealth | null>(null);
  const [loading, setLoading] = useState(true);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [cleanState, setCleanState] = useState<CleanState>('idle');
  const [cleanResult, setCleanResult] = useState<MaintenanceResult | null>(null);
  const [retentionDays, setRetentionDays] = useState(30);
  const [retentionSaving, setRetentionSaving] = useState(false);
  const [warning, setWarning] = useState<string | null>(null);

  const loadHealth = useCallback(async () => {
    try {
      const data = await cmd('get_data_health');
      setHealth(data);
      setRetentionDays(data.retention_days);
    } catch {
      setHealthError('Could not load data health');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadHealth();
  }, [loadHealth]);

  // Listen for data health warnings from monitoring
  useEffect(() => {
    const unlisten = listen<{ message: string }>('data-health-warning', (event) => {
      setWarning(event.payload.message);
    });
    return () => { void unlisten.then(fn => fn()); };
  }, []);

  const handleDeepClean = async () => {
    setCleanState('cleaning');
    setCleanResult(null);
    try {
      const result = await cmd('run_deep_clean');
      setCleanResult(result);
      setCleanState('done');
      // Refresh stats after clean
      void loadHealth();
    } catch {
      setCleanState('idle');
    }
  };

  const handleRetentionChange = async (days: number) => {
    setRetentionDays(days);
    setRetentionSaving(true);
    try {
      await cmd('set_cleanup_retention', { days });
    } catch {
      // Revert on error
      if (health) setRetentionDays(health.retention_days);
    } finally {
      setRetentionSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <div className="text-xs text-text-muted">{t('settings.dataHealth.loading')}</div>
      </div>
    );
  }

  if (!health) {
    return healthError ? (
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <div className="text-xs text-red-400">{healthError}</div>
      </div>
    ) : null;
  }

  const { stats } = health;

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      {/* Header */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-orange-400">
              <path d="M2 3h12v2H2V3zm0 4h12v2H2V7zm0 4h8v2H2v-2z" fill="currentColor" opacity="0.7" />
            </svg>
          </div>
          <div>
            <h3 className="text-sm font-medium text-white">
              {t('settings.dataHealth.title')}
            </h3>
            <p className="text-xs text-text-muted">
              {t('settings.dataHealth.description')}
            </p>
          </div>
        </div>
        <HealthBadge status={health.health_status} />
      </div>

      {/* Warning banner */}
      {warning != null && warning !== '' && (
        <div className="mb-3 p-2.5 bg-red-500/10 border border-red-500/20 rounded-lg text-xs text-red-400">
          {warning}
        </div>
      )}

      {/* DB Size overview */}
      <div className="p-3 bg-bg-secondary rounded-lg border border-border mb-3">
        <div className="flex items-center justify-between mb-2">
          <span className="text-xs font-medium text-white">
            {t('settings.dataHealth.databaseSize')}
          </span>
          <span className="text-sm font-mono text-white">{formatBytes(stats.db_size_bytes)}</span>
        </div>
        <div className="space-y-0.5">
          <StatRow label={t('settings.dataHealth.contentItems')} value={formatCount(stats.source_items)} />
          <StatRow label={t('settings.dataHealth.embeddings')} value={formatCount(stats.embeddings_count)} />
          <StatRow label={t('settings.dataHealth.feedback')} value={formatCount(stats.feedback_count)} />
          <StatRow label={t('settings.dataHealth.intelligence')} value={formatCount(stats.digested_intelligence)} />
          <StatRow label={t('settings.dataHealth.decisionWindows')} value={formatCount(stats.decision_windows)} />
          <StatRow label={t('settings.dataHealth.autophagyCycles')} value={formatCount(stats.autophagy_cycles)} />
          <StatRow label={t('settings.dataHealth.necessityScores')} value={formatCount(stats.necessity_scores)} />
          {stats.oldest_item_date != null && stats.oldest_item_date !== '' && (
            <StatRow
              label={t('settings.dataHealth.oldestItem')}
              value={stats.oldest_item_date.split('T')[0] ?? stats.oldest_item_date}
              muted
            />
          )}
        </div>
      </div>

      {/* Retention slider */}
      <div className="p-3 bg-bg-secondary rounded-lg border border-border mb-3">
        <div className="flex items-center justify-between mb-2">
          <span className="text-xs font-medium text-white">
            {t('settings.dataHealth.retention')}
          </span>
          <span className="text-xs text-text-secondary font-mono">
            {retentionDays} {t('settings.dataHealth.days')}
            {retentionSaving ? <span className="text-orange-400 ms-1">{t('settings.dataHealth.saving')}</span> : null}
          </span>
        </div>
        <input
          type="range"
          min={7}
          max={365}
          step={1}
          value={retentionDays}
          onChange={(e) => { void handleRetentionChange(parseInt(e.target.value)); }}
          className="w-full h-1 bg-border rounded-full appearance-none cursor-pointer accent-orange-500"
        />
        <div className="flex justify-between text-[10px] text-text-muted mt-1">
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span>7d</span><span>30d</span><span>90d</span><span>180d</span><span>365d</span>
        </div>
      </div>

      {/* Routine Maintenance */}
      <div className="p-3 bg-bg-secondary rounded-lg border border-border">
        <div className="flex items-center gap-2 mb-2.5">
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" className="text-orange-400 flex-shrink-0">
            <path d="M8 1a7 7 0 100 14A7 7 0 008 1zm0 12.5a5.5 5.5 0 110-11 5.5 5.5 0 010 11zM8 4v4.5l3 1.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <span className="text-xs font-medium text-white">
            {t('settings.dataHealth.maintenanceTitle')}
          </span>
        </div>

        <p className="text-[11px] text-text-muted mb-3 leading-relaxed">
          {t('settings.dataHealth.maintenanceDesc')}
        </p>

        {/* What happens — always visible */}
        <div className="grid grid-cols-2 gap-2 mb-3">
          <div className="p-2 rounded bg-bg-tertiary">
            <p className="text-[10px] text-green-400 font-medium mb-1">
              {t('settings.dataHealth.preserved')}
            </p>
            <ul className="text-[10px] text-text-muted space-y-0.5">
              <li>{t('settings.dataHealth.preserveCalibrations')}</li>
              <li>{t('settings.dataHealth.preserveRecent')}</li>
              <li>{t('settings.dataHealth.preserveDecisions')}</li>
              <li>{t('settings.dataHealth.preserveProfile')}</li>
            </ul>
          </div>
          <div className="p-2 rounded bg-bg-tertiary">
            <p className="text-[10px] text-text-secondary font-medium mb-1">
              {t('settings.dataHealth.removed')}
            </p>
            <ul className="text-[10px] text-text-muted space-y-0.5">
              <li>{t('settings.dataHealth.removeOld')}</li>
              <li>{t('settings.dataHealth.removeOrphaned')}</li>
              <li>{t('settings.dataHealth.removeSuperseded')}</li>
              <li>{t('settings.dataHealth.removeFragments')}</li>
            </ul>
          </div>
        </div>

        {/* When to use hint */}
        <p className="text-[10px] text-text-muted mb-3 italic">
          {t('settings.dataHealth.whenToUse')}
        </p>

        {/* Action area */}
        <div className="space-y-2">
          {cleanState === 'idle' && (
            <button
              onClick={() => { void handleDeepClean(); }}
              className="w-full px-4 py-2 text-xs bg-bg-tertiary border border-border text-text-secondary rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
            >
              {t('settings.dataHealth.runMaintenance')}
            </button>
          )}

          {cleanState === 'cleaning' && (
            <div className="flex items-center justify-center gap-2 py-2">
              <div className="w-3 h-3 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
              <span className="text-xs text-orange-400">
                {t('settings.dataHealth.cleaning')}
              </span>
            </div>
          )}

          {cleanState === 'done' && cleanResult && (
            <div>
              <CleanResultSummary result={cleanResult} />
              <button
                onClick={() => { setCleanState('idle'); setCleanResult(null); }}
                className="mt-2 text-xs text-text-muted hover:text-white transition-colors"
              >
                {t('settings.dataHealth.dismiss')}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
