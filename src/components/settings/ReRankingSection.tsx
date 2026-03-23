import { useTranslation } from 'react-i18next';

import type { SettingsForm } from './ai-provider-types';

interface ReRankingSectionProps {
  settingsForm: SettingsForm;
  setSettingsForm: React.Dispatch<React.SetStateAction<SettingsForm>>;
}

export function ReRankingSection({ settingsForm, setSettingsForm }: ReRankingSectionProps) {
  const { t } = useTranslation();

  return (
    <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
      <div className="flex items-center gap-3 mb-4">
        <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
          <span>&#x26a1;</span>
        </div>
        <div>
          <h3 className="text-sm font-medium text-white">{t('settings.ai.rerankTitle')}</h3>
          <p className="text-xs text-text-muted">{t('settings.ai.rerankDescription')}</p>
        </div>
      </div>

      <div className="space-y-4">
        <label className="flex items-center gap-3 cursor-pointer p-3 bg-bg-secondary rounded-lg border border-border hover:border-orange-500/30 transition-all">
          <input
            type="checkbox"
            checked={settingsForm.rerankEnabled}
            onChange={(e) => setSettingsForm((f) => ({ ...f, rerankEnabled: e.target.checked }))}
            className="w-5 h-5 accent-orange-500 rounded"
          />
          <div>
            <span className="text-sm text-white">{t('settings.ai.enableRerank')}</span>
            <p className="text-xs text-text-muted mt-0.5">{t('settings.ai.rerankNote')}</p>
          </div>
        </label>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="text-xs text-text-muted block mb-2">{t('settings.ai.maxItemsBatch')}</label>
            <input
              type="number"
              value={settingsForm.maxItems}
              onChange={(e) => setSettingsForm((f) => ({ ...f, maxItems: parseInt(e.target.value) || 15 }))}
              className="w-full px-4 py-3 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
            />
          </div>
          <div>
            <label className="text-xs text-text-muted block mb-2">{t('settings.ai.minScore')}</label>
            <input
              type="number"
              step="0.05"
              value={settingsForm.minScore}
              onChange={(e) => setSettingsForm((f) => ({ ...f, minScore: parseFloat(e.target.value) || 0.25 }))}
              className="w-full px-4 py-3 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
            />
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="text-xs text-text-muted block mb-2">{t('settings.ai.dailyTokenLimit')}</label>
            <input
              type="number"
              value={settingsForm.dailyTokenLimit}
              onChange={(e) => setSettingsForm((f) => ({ ...f, dailyTokenLimit: parseInt(e.target.value) || 100000 }))}
              className="w-full px-4 py-3 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
            />
          </div>
          <div>
            <label className="text-xs text-text-muted block mb-2">{t('settings.ai.costLimit')}</label>
            <input
              type="number"
              value={settingsForm.dailyCostLimit}
              onChange={(e) => setSettingsForm((f) => ({ ...f, dailyCostLimit: parseInt(e.target.value) || 50 }))}
              className="w-full px-4 py-3 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
            />
          </div>
        </div>
      </div>
    </div>
  );
}
