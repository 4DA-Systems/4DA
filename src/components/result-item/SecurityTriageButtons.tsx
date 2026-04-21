// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { SourceRelevance } from '../../types/analysis';

interface Props {
  item: SourceRelevance;
  onTriaged?: () => void;
}

export function SecurityTriageButtons({ item, onTriaged }: Props) {
  const { t } = useTranslation();
  const [triageAction, setTriageAction] = useState<string | null>(null);

  const handleTriage = useCallback((action: string) => {
    setTriageAction(action);
    cmd('triage_alert', {
      itemId: item.id,
      action,
      advisoryId: item.advisory_id ?? null,
      reason: null,
      expiresAt: action === 'snoozed'
        ? new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString()
        : null,
    }).catch(() => { /* best-effort */ });
    onTriaged?.();
  }, [item.id, item.advisory_id, onTriaged]);

  if (triageAction) {
    return (
      <div className="flex items-center gap-1.5 text-[11px] text-text-muted mb-3">
        <span className={
          triageAction === 'investigating' ? 'text-blue-400'
          : triageAction === 'fixed' ? 'text-emerald-400'
          : triageAction === 'accepted_risk' ? 'text-amber-400'
          : 'text-text-muted'
        }>
          {triageAction === 'investigating' ? t('triage.investigating', 'Investigating')
          : triageAction === 'fixed' ? t('triage.fixed', 'Fixed')
          : triageAction === 'not_applicable' ? t('triage.notApplicable', 'Not applicable')
          : triageAction === 'accepted_risk' ? t('triage.riskAccepted', 'Risk accepted')
          : t('triage.snoozed', 'Snoozed 7d')}
        </span>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-1 mb-3">
      <button
        onClick={() => handleTriage('investigating')}
        className="px-2 py-1 text-[11px] rounded bg-blue-500/10 text-blue-400 hover:bg-blue-500/20 transition-colors"
      >
        {t('triage.investigate', 'Investigate')}
      </button>
      <button
        onClick={() => handleTriage('fixed')}
        className="px-2 py-1 text-[11px] rounded bg-emerald-500/10 text-emerald-400 hover:bg-emerald-500/20 transition-colors"
      >
        {t('triage.markFixed', 'Fixed')}
      </button>
      <button
        onClick={() => handleTriage('not_applicable')}
        className="px-2 py-1 text-[11px] rounded bg-zinc-500/10 text-text-muted hover:bg-zinc-500/20 transition-colors"
      >
        {t('triage.markNA', 'N/A')}
      </button>
      <button
        onClick={() => handleTriage('accepted_risk')}
        className="px-2 py-1 text-[11px] rounded bg-amber-500/10 text-amber-400 hover:bg-amber-500/20 transition-colors"
        title={t('triage.acceptRiskTooltip', 'Accept risk for 30 days')}
      >
        {t('triage.acceptRisk', 'Accept')}
      </button>
    </div>
  );
}
