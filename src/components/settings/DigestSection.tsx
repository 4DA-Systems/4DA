import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

interface DigestSectionProps {
  setSettingsStatus: (status: string) => void;
}

interface DigestConfig {
  enabled: boolean;
  save_local: boolean;
  frequency: string;
  min_score: number;
  max_items: number;
}

export function DigestSection({ setSettingsStatus }: DigestSectionProps) {
  const { t } = useTranslation();
  const [digestConfig, setDigestConfig] = useState<DigestConfig | null>(null);

  useEffect(() => {
    loadDigestConfig();
  }, []);

  const loadDigestConfig = async () => {
    try {
      const config = await invoke<DigestConfig>('get_digest_config');
      setDigestConfig(config);
    } catch (error) {
      console.error('Failed to load digest config:', error);
    }
  };

  const handleToggleDigest = async () => {
    if (!digestConfig) return;
    try {
      await invoke('set_digest_config', {
        enabled: !digestConfig.enabled,
      });
      setDigestConfig({ ...digestConfig, enabled: !digestConfig.enabled });
      setSettingsStatus(digestConfig.enabled ? t('settings.digest.disabled') : t('settings.digest.enabled'));
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  };

  return (
    <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
      <div className="flex items-start gap-3 mb-4">
        <div className="w-8 h-8 bg-purple-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-purple-400">&#x1f4cb;</span>
        </div>
        <div>
          <h3 className="text-white font-medium">{t('settings.digest.title')}</h3>
          <p className="text-gray-500 text-sm mt-1">
            {t('settings.digest.description')}
          </p>
        </div>
      </div>

      {digestConfig ? (
        <div className="space-y-4">
          <div className="flex items-center justify-between p-3 bg-bg-secondary rounded-lg border border-border">
            <div className="flex items-center gap-3">
              <div className={`w-2 h-2 rounded-full ${digestConfig.enabled ? 'bg-green-500' : 'bg-gray-500'}`} />
              <span className="text-sm text-gray-300">
                {digestConfig.enabled ? t('status.active') : t('status.inactive')}
              </span>
            </div>
            <button
              onClick={handleToggleDigest}
              className={`px-4 py-2 text-sm rounded-lg transition-all ${
                digestConfig.enabled
                  ? 'bg-red-500/10 text-red-400 border border-red-500/30 hover:bg-red-500/20'
                  : 'bg-green-500/10 text-green-400 border border-green-500/30 hover:bg-green-500/20'
              }`}
            >
              {digestConfig.enabled ? t('action.disable') : t('action.enable')}
            </button>
          </div>

          {digestConfig.enabled && (
            <div className="grid grid-cols-3 gap-3">
              <div className="p-3 bg-bg-secondary rounded-lg border border-border text-center">
                <div className="text-xs text-gray-500 mb-1">{t('settings.digest.frequency')}</div>
                <div className="text-sm text-white font-medium">{digestConfig.frequency}</div>
              </div>
              <div className="p-3 bg-bg-secondary rounded-lg border border-border text-center">
                <div className="text-xs text-gray-500 mb-1">{t('settings.digest.minScore')}</div>
                <div className="text-sm text-white font-medium">{Math.round(digestConfig.min_score * 100)}%</div>
              </div>
              <div className="p-3 bg-bg-secondary rounded-lg border border-border text-center">
                <div className="text-xs text-gray-500 mb-1">{t('settings.digest.maxItems')}</div>
                <div className="text-sm text-white font-medium">{digestConfig.max_items}</div>
              </div>
            </div>
          )}
        </div>
      ) : (
        <div className="text-sm text-gray-500">{t('settings.digest.loading')}</div>
      )}
    </div>
  );
}
