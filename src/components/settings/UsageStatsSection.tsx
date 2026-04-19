// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';

import type { Settings } from '../../types';

interface UsageStatsSectionProps {
  settings: Settings;
  provider: string;
}

export function UsageStatsSection({ settings, provider }: UsageStatsSectionProps) {
  const { t } = useTranslation();

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div className="flex items-center gap-3 mb-3">
        <div className="w-8 h-8 bg-green-500/20 rounded-lg flex items-center justify-center">
          <span>&#x1f4c8;</span>
        </div>
        <div>
          <h3 className="text-sm font-medium text-white">{t('settings.ai.usageTitle')}</h3>
          <p className="text-xs text-text-muted">{t('settings.ai.usageDescription')}</p>
        </div>
      </div>
      <div className="grid grid-cols-3 gap-3">
        <div className="bg-bg-secondary rounded-lg p-3 text-center">
          <p className="text-xl font-semibold text-white">{settings.usage.tokens_today.toLocaleString()}</p>
          <p className="text-xs text-text-muted">{t('settings.ai.tokens')}</p>
        </div>
        <div className="bg-bg-secondary rounded-lg p-3 text-center">
          {provider === 'openai-compatible' ? (
            <>
              <p className="text-sm font-semibold text-text-muted">{t('settings.ai.costUnavailableMessage', 'Not tracked for this provider')}</p>
              <p className="text-xs text-text-muted">{t('settings.ai.cost')}</p>
            </>
          ) : (
            <>
              <p className="text-xl font-semibold text-green-400">${(settings.usage.cost_today_cents / 100).toFixed(2)}</p>
              <p className="text-xs text-text-muted">{t('settings.ai.cost')}</p>
            </>
          )}
        </div>
        <div className="bg-bg-secondary rounded-lg p-3 text-center">
          <p className="text-xl font-semibold text-orange-400">{settings.usage.items_reranked}</p>
          <p className="text-xs text-text-muted">{t('settings.ai.reranked')}</p>
        </div>
      </div>
    </div>
  );
}
