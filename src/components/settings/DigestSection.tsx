import { useState, useEffect } from 'react';
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
      setSettingsStatus(digestConfig.enabled ? 'Digest disabled' : 'Digest enabled');
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  };

  return (
    <div className="bg-[#1F1F1F] rounded-lg p-5 border border-[#2A2A2A]">
      <div className="flex items-start gap-3 mb-4">
        <div className="w-8 h-8 bg-purple-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-purple-400">&#x1f4cb;</span>
        </div>
        <div>
          <h3 className="text-white font-medium">Daily Digest</h3>
          <p className="text-gray-500 text-sm mt-1">
            Save filtered results as a digest file after each analysis
          </p>
        </div>
      </div>

      {digestConfig ? (
        <div className="space-y-4">
          <div className="flex items-center justify-between p-3 bg-[#141414] rounded-lg border border-[#2A2A2A]">
            <div className="flex items-center gap-3">
              <div className={`w-2 h-2 rounded-full ${digestConfig.enabled ? 'bg-green-500' : 'bg-gray-500'}`} />
              <span className="text-sm text-gray-300">
                {digestConfig.enabled ? 'Active' : 'Inactive'}
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
              {digestConfig.enabled ? 'Disable' : 'Enable'}
            </button>
          </div>

          {digestConfig.enabled && (
            <div className="grid grid-cols-3 gap-3">
              <div className="p-3 bg-[#141414] rounded-lg border border-[#2A2A2A] text-center">
                <div className="text-xs text-gray-500 mb-1">Frequency</div>
                <div className="text-sm text-white font-medium">{digestConfig.frequency}</div>
              </div>
              <div className="p-3 bg-[#141414] rounded-lg border border-[#2A2A2A] text-center">
                <div className="text-xs text-gray-500 mb-1">Min Score</div>
                <div className="text-sm text-white font-medium">{Math.round(digestConfig.min_score * 100)}%</div>
              </div>
              <div className="p-3 bg-[#141414] rounded-lg border border-[#2A2A2A] text-center">
                <div className="text-xs text-gray-500 mb-1">Max Items</div>
                <div className="text-sm text-white font-medium">{digestConfig.max_items}</div>
              </div>
            </div>
          )}
        </div>
      ) : (
        <div className="text-sm text-gray-500">Loading digest settings...</div>
      )}
    </div>
  );
}
