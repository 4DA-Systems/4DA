import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';

interface CommunityStatus {
  enabled: boolean;
  frequency: string;
  last_contributed: string | null;
  anonymous_id_preview: string | null;
}

export function CommunityInsights() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<CommunityStatus | null>(null);

  useEffect(() => {
    invoke<CommunityStatus>('get_community_status')
      .then(setStatus)
      .catch((e) => console.warn('CommunityInsights: failed to load status', e));
  }, []);

  // Only show if community intelligence is enabled
  if (!status?.enabled) return null;

  return (
    <div className="bg-bg-secondary border border-blue-500/20 rounded-lg p-4 mb-4">
      <div className="flex items-center gap-2 mb-2">
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
        <h3 className="text-sm font-medium text-white">
          {t('community.title', 'Community Intelligence')}
        </h3>
        <span className="ml-auto px-1.5 py-0.5 bg-blue-500/20 text-blue-400 text-[10px] rounded">
          {t('community.active', 'Active')}
        </span>
      </div>

      <p className="text-xs text-text-secondary mb-3">
        {t(
          'community.description',
          'Contributing anonymized scoring patterns to improve accuracy for all 4DA users.',
        )}
      </p>

      <div className="grid grid-cols-2 gap-2 text-xs">
        <div className="bg-bg-tertiary rounded p-2">
          <div className="text-text-muted">{t('community.frequency', 'Frequency')}</div>
          <div className="text-white capitalize">{status.frequency}</div>
        </div>
        <div className="bg-bg-tertiary rounded p-2">
          <div className="text-text-muted">{t('community.lastSync', 'Last sync')}</div>
          <div className="text-white">
            {status.last_contributed
              ? new Date(status.last_contributed).toLocaleDateString()
              : t('community.pending', 'Pending')}
          </div>
        </div>
      </div>
    </div>
  );
}
