// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { AiUsageSummary, AiCostRecommendation } from '../../lib/commands';

// ============================================================================
// Sub-components
// ============================================================================

function LoadingSkeleton() {
  return (
    <div className="p-5 space-y-4">
      <div className="grid grid-cols-3 gap-3">
        {[1, 2, 3].map(i => (
          <div key={i} className="h-16 bg-bg-tertiary rounded-lg animate-pulse" />
        ))}
      </div>
      <div className="h-32 bg-bg-tertiary rounded-lg animate-pulse" />
    </div>
  );
}

function StatCard({ label, value, suffix, color }: { label: string; value: string | number; suffix?: string; color?: string }) {
  return (
    <div className="bg-bg-tertiary rounded-lg border border-border px-4 py-3">
      <div className={`text-xl font-semibold ${color ?? 'text-white'}`}>
        {suffix === '$' && <span className="text-sm mr-0.5">$</span>}
        {value}
        {suffix && suffix !== '$' && <span className="text-sm ml-0.5">{suffix}</span>}
      </div>
      <div className="text-xs text-text-muted mt-0.5">{label}</div>
    </div>
  );
}

function formatTokens(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(1)}K`;
  return count.toString();
}

// ============================================================================
// CostTab
// ============================================================================

export const CostTab = memo(function CostTab() {
  const { t } = useTranslation();
  const [usage, setUsage] = useState<AiUsageSummary | null>(null);
  const [recommendation, setRecommendation] = useState<AiCostRecommendation | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    setError(null);

    Promise.allSettled([
      cmd('get_ai_usage_summary', { period: 'month' }),
      cmd('get_ai_cost_recommendation'),
    ])
      .then(([usageResult, recResult]) => {
        if (usageResult.status === 'fulfilled') {
          setUsage(usageResult.value);
          // Usage summary may include an inline recommendation
          if (usageResult.value.recommendation) {
            setRecommendation(usageResult.value.recommendation);
          }
        }
        if (recResult.status === 'fulfilled') {
          setRecommendation(recResult.value);
        }
      })
      .catch(() => setError('Failed to load AI cost data.'))
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <LoadingSkeleton />;

  if (error && !usage) {
    return (
      <div className="p-5">
        <div className="bg-error/10 rounded-lg border border-error/20 p-4">
          <p className="text-xs text-error">{error}</p>
        </div>
      </div>
    );
  }

  if (!usage || (usage.total_cost_usd === 0 && usage.by_provider.length === 0)) {
    return (
      <div className="p-5">
        <div className="bg-bg-primary rounded-lg border border-border/50 p-6 text-center">
          <p className="text-sm text-text-muted mb-2">{t('costs.noData')}</p>
          <p className="text-xs text-text-muted/60 leading-relaxed">
            {t('costs.noDataDesc')}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-5 space-y-5 overflow-y-auto">
      {/* Summary Stats */}
      <div className="grid grid-cols-3 gap-3">
        <StatCard
          label={t('costs.totalCost')}
          value={usage.total_cost_usd.toFixed(2)}
          suffix="$"
          color="text-white"
        />
        <StatCard
          label={t('costs.tokensIn')}
          value={formatTokens(usage.total_tokens_in)}
          color="text-success"
        />
        <StatCard
          label={t('costs.tokensOut')}
          value={formatTokens(usage.total_tokens_out)}
          color="text-accent-gold"
        />
      </div>

      {/* Cost by Provider */}
      {usage.by_provider.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            {t('costs.byProvider')}
          </h4>
          <div className="overflow-hidden rounded-lg border border-border">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-bg-tertiary text-text-muted text-xs uppercase tracking-wider">
                  <th className="text-left px-4 py-2.5 font-medium">{t('costs.thProvider')}</th>
                  <th className="text-left px-4 py-2.5 font-medium">{t('costs.thModel')}</th>
                  <th className="text-right px-4 py-2.5 font-medium">{t('costs.thRequests')}</th>
                  <th className="text-right px-4 py-2.5 font-medium">{t('costs.thCost')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {usage.by_provider.map((entry, i) => (
                  <tr key={`${entry.provider}-${entry.model}-${i}`} className="hover:bg-[#1A1A1A] transition-colors">
                    <td className="px-4 py-2.5 text-text-primary">{entry.provider}</td>
                    <td className="px-4 py-2.5 font-mono text-text-secondary text-xs">{entry.model}</td>
                    <td className="px-4 py-2.5 text-text-secondary text-right">{entry.request_count}</td>
                    <td className="px-4 py-2.5 text-right">
                      <span className="text-text-primary font-mono">${entry.cost_usd.toFixed(4)}</span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Cost by Task */}
      {usage.by_task.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            {t('costs.byTask')}
          </h4>
          <div className="space-y-2">
            {usage.by_task.map((task, i) => {
              const pct = usage.total_cost_usd > 0 ? (task.cost_usd / usage.total_cost_usd) * 100 : 0;
              return (
                <div key={`${task.task_type}-${i}`} className="bg-bg-primary rounded-lg border border-border/50 px-4 py-3">
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="text-sm text-text-primary">{task.task_type}</span>
                    <div className="flex items-center gap-3">
                      <span className="text-xs text-text-muted">{task.request_count} req</span>
                      <span className="text-xs font-mono text-text-secondary">${task.cost_usd.toFixed(4)}</span>
                    </div>
                  </div>
                  <div className="w-full h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                    <div
                      className="h-full bg-accent-gold rounded-full transition-all"
                      style={{ width: `${Math.min(pct, 100)}%` }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Cost Recommendation */}
      {recommendation && (
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            {t('costs.optimization')}
          </h4>
          <div className="bg-bg-primary rounded-lg border border-success/20 p-4 space-y-2">
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-success" />
              <span className="text-sm text-text-primary">
                Switch {recommendation.current_provider}/{recommendation.current_model} to{' '}
                {recommendation.recommended_provider}/{recommendation.recommended_model}
              </span>
            </div>
            <p className="text-xs text-text-muted leading-relaxed">{recommendation.reason}</p>
            <div className="flex items-center gap-4 text-xs text-text-muted">
              <span>
                {t('costs.estSavings')}: <span className="text-success font-medium">${recommendation.estimated_savings_usd.toFixed(2)}/mo</span>
              </span>
              <span>
                {t('costs.qualityMatch')}: <span className="text-text-secondary">{recommendation.quality_match_pct}%</span>
              </span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
});
