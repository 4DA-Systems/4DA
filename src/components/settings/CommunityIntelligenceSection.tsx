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
    <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center">
            <svg
              className="w-4 h-4 text-blue-400"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
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

        <button
          onClick={handleToggle}
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

      {/* Confirmation Dialog */}
      {showConfirm && (
        <div className="mb-4 p-3 bg-blue-500/10 border border-blue-500/20 rounded-lg">
          <p className="text-sm text-white mb-2 font-medium">
            {t('settings.community.confirmTitle', 'Enable Community Intelligence?')}
          </p>
          <div className="space-y-1.5 mb-3">
            <p className="text-xs text-text-secondary">
              {t('settings.community.shared', "What's shared:")}
            </p>
            <ul className="text-xs text-text-muted space-y-0.5 ms-3">
              <li>{t('settings.community.sharedWeights', 'Anonymized scoring weights')}</li>
              <li>{t('settings.community.sharedAccuracy', 'Stack profile accuracy metrics')}</li>
              <li>{t('settings.community.sharedTrends', 'Aggregated topic trend signals')}</li>
            </ul>
            <p className="text-xs text-text-secondary mt-2">
              {t('settings.community.neverShared', "What's NEVER shared:")}
            </p>
            <ul className="text-xs text-text-muted space-y-0.5 ms-3">
              <li>{t('settings.community.neverContent', 'Your content, URLs, or bookmarks')}</li>
              <li>{t('settings.community.neverIdentity', 'Your identity or API keys')}</li>
              <li>{t('settings.community.neverStack', 'Your tech stack or interests')}</li>
            </ul>
          </div>
          <div className="flex gap-2">
            <button
              onClick={confirmEnable}
              className="px-3 py-1.5 bg-blue-500/20 text-blue-400 text-xs rounded hover:bg-blue-500/30 transition-colors"
            >
              {t('settings.community.enable', 'Enable')}
            </button>
            <button
              onClick={() => setShowConfirm(false)}
              className="px-3 py-1.5 bg-bg-primary text-text-muted text-xs rounded hover:text-white transition-colors"
            >
              {t('settings.community.cancel', 'Cancel')}
            </button>
          </div>
        </div>
      )}

      {status.enabled && (
        <div className="space-y-3">
          {/* Frequency */}
          <div className="flex items-center justify-between">
            <span className="text-xs text-text-secondary">
              {t('settings.community.frequency', 'Contribution frequency')}
            </span>
            <select
              value={status.frequency}
              onChange={(e) => handleFrequencyChange(e.target.value)}
              className="bg-bg-primary text-white text-xs rounded px-2 py-1 border border-border"
            >
              <option value="weekly">{t('settings.community.weekly', 'Weekly')}</option>
              <option value="monthly">{t('settings.community.monthly', 'Monthly')}</option>
            </select>
          </div>

          {/* Last contribution */}
          <div className="flex items-center justify-between text-xs">
            <span className="text-text-muted">
              {t('settings.community.lastContribution', 'Last contribution')}
            </span>
            <span className="text-text-secondary">
              {status.last_contributed || t('settings.community.never', 'Never')}
            </span>
          </div>

          {/* Anonymous ID preview */}
          {status.anonymous_id_preview && (
            <div className="flex items-center justify-between text-xs">
              <span className="text-text-muted">
                {t('settings.community.anonymousId', 'Anonymous ID')}
              </span>
              <span className="text-text-secondary font-mono">
                {status.anonymous_id_preview}...
              </span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
