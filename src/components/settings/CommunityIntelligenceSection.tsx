import { useState, useEffect, useCallback } from 'react';
import { cmd } from '../../lib/commands';
import { useTranslation } from 'react-i18next';

interface CommunityStatus {
  enabled: boolean;
  frequency: string;
  last_contributed: string | null;
  anonymous_id_preview: string | null;
}

export function CommunityIntelligenceSection() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<CommunityStatus | null>(null);
  const [showConfirm, setShowConfirm] = useState(false);

  const loadStatus = useCallback(() => {
    cmd('get_community_status')
      .then(setStatus)
      .catch((e) => console.warn('CommunityIntelligence: failed to load status', e));
  }, []);

  useEffect(() => {
    loadStatus();
  }, [loadStatus]);

  const handleToggle = useCallback(async () => {
    if (!status) return;

    if (!status.enabled) {
      // Show confirmation before enabling
      setShowConfirm(true);
      return;
    }

    // Disable directly
    await cmd('set_community_intelligence_enabled', { enabled: false });
    loadStatus();
  }, [status, loadStatus]);

  const confirmEnable = useCallback(async () => {
    await cmd('set_community_intelligence_enabled', { enabled: true });
    setShowConfirm(false);
    loadStatus();
  }, [loadStatus]);

  const handleFrequencyChange = useCallback(
    async (freq: string) => {
      await cmd('set_community_frequency', { frequency: freq });
      loadStatus();
    },
    [loadStatus],
  );

  if (!status) return null;

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      {/* Header */}
      <div className="flex items-center gap-3 mb-2">
        <div className="w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center">
          <svg className="w-4 h-4 text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
              d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z"
            />
          </svg>
        </div>
        <div>
          <h3 className="text-white font-medium text-sm">
            {t('settings.community.title', 'Community Intelligence')}
          </h3>
          <p className="text-text-muted text-xs">
            {t('settings.community.subtitle', 'Help improve scoring for all 4DA users')}
          </p>
        </div>
      </div>

      {/* Upfront education — always visible */}
      <div className="mb-3 p-2.5 bg-bg-secondary rounded-lg border border-border">
        <p className="text-[11px] text-text-muted leading-relaxed mb-2">
          {t('settings.community.explanation',
            'When enabled, 4DA periodically shares anonymous statistical patterns — like how scoring weights perform across different tech profiles. This helps improve accuracy for everyone. No content ever leaves your machine.')}
        </p>

        {/* Privacy guarantees — always visible, not hidden behind a confirmation */}
        <div className="grid grid-cols-2 gap-2">
          <div className="p-2 rounded bg-bg-tertiary">
            <p className="text-[10px] text-blue-400 font-medium mb-1">
              {t('settings.community.shared', "What's shared")}
            </p>
            <ul className="text-[10px] text-text-muted space-y-0.5">
              <li>{t('settings.community.sharedWeights', 'Anonymized scoring weights')}</li>
              <li>{t('settings.community.sharedAccuracy', 'Stack profile accuracy metrics')}</li>
              <li>{t('settings.community.sharedTrends', 'Aggregated topic trend signals')}</li>
            </ul>
          </div>
          <div className="p-2 rounded bg-bg-tertiary">
            <p className="text-[10px] text-green-400 font-medium mb-1">
              {t('settings.community.neverShared', 'Never shared')}
            </p>
            <ul className="text-[10px] text-text-muted space-y-0.5">
              <li>{t('settings.community.neverContent', 'Your content, URLs, or bookmarks')}</li>
              <li>{t('settings.community.neverIdentity', 'Your identity or API keys')}</li>
              <li>{t('settings.community.neverStack', 'Your tech stack or interests')}</li>
            </ul>
          </div>
        </div>
      </div>

      {/* Toggle — now the user knows exactly what they're opting into */}
      <div className="flex items-center justify-between p-3 bg-bg-secondary rounded-lg border border-border">
        <div className="flex items-center gap-2">
          <span className="text-xs text-text-secondary">
            {status.enabled
              ? t('settings.community.statusActive', 'Contributing anonymously')
              : t('settings.community.statusInactive', 'Not contributing')}
          </span>
        </div>

        <button
          onClick={() => {
            if (status.enabled) {
              void handleToggle();
            } else if (showConfirm) {
              setShowConfirm(false);
            } else {
              setShowConfirm(true);
            }
          }}
          className={`relative w-10 h-5 rounded-full transition-colors ${
            status.enabled ? 'bg-green-500/40' : 'bg-gray-600'
          }`}
        >
          <span
            className={`absolute top-0.5 start-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
              status.enabled ? 'translate-x-5' : 'translate-x-0'
            }`}
          />
        </button>
      </div>

      {/* Soft confirmation — since the user already read the education above */}
      {showConfirm && !status.enabled && (
        <div className="mt-2 flex items-center gap-2">
          <button
            onClick={() => { void confirmEnable(); }}
            className="px-3 py-1.5 bg-blue-500/20 text-blue-400 text-xs rounded hover:bg-blue-500/30 transition-colors"
          >
            {t('settings.community.confirmEnable', 'Yes, enable')}
          </button>
          <button
            onClick={() => setShowConfirm(false)}
            className="px-3 py-1.5 text-text-muted text-xs rounded hover:text-white transition-colors"
          >
            {t('settings.community.cancel', 'Cancel')}
          </button>
        </div>
      )}

      {/* Settings when enabled */}
      {status.enabled && (
        <div className="mt-3 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs text-text-muted">
              {t('settings.community.frequency', 'Contribution frequency')}
            </span>
            <select
              value={status.frequency}
              onChange={(e) => { void handleFrequencyChange(e.target.value); }}
              className="bg-bg-primary text-white text-xs rounded px-2 py-1 border border-border"
            >
              <option value="weekly">{t('settings.community.weekly', 'Weekly')}</option>
              <option value="monthly">{t('settings.community.monthly', 'Monthly')}</option>
            </select>
          </div>

          {status.last_contributed != null && (
            <div className="flex items-center justify-between text-xs">
              <span className="text-text-muted">
                {t('settings.community.lastContribution', 'Last contribution')}
              </span>
              <span className="text-text-secondary">
                {status.last_contributed}
              </span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
