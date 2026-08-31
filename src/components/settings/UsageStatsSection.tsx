// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';

import type { Settings } from '../../types';

interface UsageStatsSectionProps {
  settings: Settings;
  provider: string;
}

interface LlmUsage {
  used: number;
  limit: number;
  cost_used_cents: number;
  cost_limit_cents: number;
  cost_limit_reached: boolean;
}

interface TaskRow {
  task_type: string;
  cost_usd: number;
  request_count: number;
}

/** How many by-feature rows earn a place. Everything below folds into the total. */
const MAX_FEATURE_ROWS = 6;

/**
 * Truthful AI usage panel.
 *
 * This section used to read `settings.usage` — the RERANK ledger, which only
 * counts rerank passes — and presented it as "cost today". After the
 * 2026-08-31 cost audit split usage recording per feature, that number
 * under-reported real spend by an order of magnitude while the true totals
 * sat dark in `ai_usage`. This panel now reads:
 *  - `get_llm_usage` — the global daily ledger (all features, seeded across
 *    restarts) plus the configured daily cap, and
 *  - `get_ai_usage_summary` — the current month, broken down by feature, so
 *    "what is burning tokens" is answerable from the UI instead of SQL.
 */
export function UsageStatsSection({ provider }: UsageStatsSectionProps) {
  const { t } = useTranslation();

  const [daily, setDaily] = useState<LlmUsage | null>(null);
  const [monthTotal, setMonthTotal] = useState<number | null>(null);
  const [byTask, setByTask] = useState<TaskRow[]>([]);

  useEffect(() => {
    let cancelled = false;
    cmd('get_llm_usage')
      .then((u) => { if (!cancelled) setDaily(u); })
      .catch(() => {});
    cmd('get_ai_usage_summary', {})
      .then((s) => {
        if (cancelled) return;
        setMonthTotal(s.total_cost_usd);
        setByTask(
          [...s.by_task]
            .sort((a, b) => b.cost_usd - a.cost_usd)
            .slice(0, MAX_FEATURE_ROWS),
        );
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, []);

  const costTracked = provider !== 'openai-compatible';
  const capCents = daily?.cost_limit_cents ?? 0;
  const usedCents = daily?.cost_used_cents ?? 0;
  const capPct = capCents > 0 ? (usedCents / capCents) * 100 : 0;
  const costTone = capCents > 0 && usedCents >= capCents
    ? 'text-red-400'
    : capPct > 80
      ? 'text-orange-400'
      : 'text-green-400';

  // Feature labels come from the usage namespace; an unknown task_type falls
  // back to its raw tag rather than hiding the row — never drop real spend.
  const featureLabel = (taskType: string) =>
    t(`usage:feature.${taskType}`, { defaultValue: taskType });

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div className="flex items-center gap-3 mb-3">
        <div className="w-8 h-8 bg-green-500/20 rounded-lg flex items-center justify-center">
          <span>&#x1f4c8;</span>
        </div>
        <div>
          <h3 className="text-sm font-medium text-text-primary">{t('settings.ai.usageTitle')}</h3>
          <p className="text-xs text-text-muted">{t('settings.ai.usageDescription')}</p>
        </div>
      </div>
      <div className="grid grid-cols-3 gap-3">
        <div className="bg-bg-secondary rounded-lg p-3 text-center">
          {costTracked ? (
            <>
              <p className={`text-xl font-semibold ${costTone}`}>
                {daily ? `$${(usedCents / 100).toFixed(2)}` : '—'}
              </p>
              <p className="text-xs text-text-muted">
                {capCents > 0
                  ? t('usage:todayOfCap', { cap: (capCents / 100).toFixed(2) })
                  : t('usage:today')}
              </p>
            </>
          ) : (
            <>
              <p className="text-sm font-semibold text-text-muted">{t('settings.ai.costUnavailableMessage', 'Not tracked for this provider')}</p>
              <p className="text-xs text-text-muted">{t('settings.ai.cost')}</p>
            </>
          )}
        </div>
        <div className="bg-bg-secondary rounded-lg p-3 text-center">
          <p className="text-xl font-semibold text-text-primary">
            {daily ? daily.used.toLocaleString() : '—'}
          </p>
          <p className="text-xs text-text-muted">{t('usage:tokensToday')}</p>
        </div>
        <div className="bg-bg-secondary rounded-lg p-3 text-center">
          {costTracked ? (
            <>
              <p className="text-xl font-semibold text-text-primary">
                {monthTotal !== null ? `$${monthTotal.toFixed(2)}` : '—'}
              </p>
              <p className="text-xs text-text-muted">{t('usage:thisMonth')}</p>
            </>
          ) : (
            <>
              <p className="text-xl font-semibold text-text-primary">—</p>
              <p className="text-xs text-text-muted">{t('usage:thisMonth')}</p>
            </>
          )}
        </div>
      </div>
      {costTracked && byTask.length > 0 && (
        <div className="mt-3">
          <p className="text-xs text-text-muted mb-2">{t('usage:byFeature')}</p>
          <div className="space-y-1">
            {byTask.map((row) => (
              <div
                key={row.task_type}
                className="flex items-center justify-between bg-bg-secondary rounded px-3 py-1.5 text-xs"
              >
                <span className="text-text-secondary">{featureLabel(row.task_type)}</span>
                <span className="text-text-muted">
                  {t('usage:calls', { count: row.request_count })}
                  <span className="ml-3 text-text-primary">${row.cost_usd.toFixed(2)}</span>
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
