import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { commands } from '../../lib/commands';
import type { OllamaStatus, PullProgress } from './types';

type ProviderType = 'anthropic' | 'openai' | 'ollama';

interface EnvDetection {
  has_anthropic_env: boolean;
  anthropic_env_preview: string;
  has_openai_env: boolean;
  openai_env_preview: string;
  ollama_running: boolean;
  ollama_url: string | null;
}

interface SetupAIProviderProps {
  ollamaStatus: OllamaStatus | null;
  provider: ProviderType;
  apiKey: string;
  pullingModels: boolean;
  pullProgress: Record<string, PullProgress>;
  onProviderChange: (provider: ProviderType) => void;
  onApiKeyChange: (key: string) => void;
}

export function SetupAIProvider({
  ollamaStatus,
  provider,
  apiKey,
  pullingModels,
  pullProgress,
  onProviderChange,
  onApiKeyChange,
}: SetupAIProviderProps) {
  const { t } = useTranslation();
  const [envDetection, setEnvDetection] = useState<EnvDetection | null>(null);
  const [importing, setImporting] = useState(false);

  useEffect(() => {
    commands.detect_environment({}).then(setEnvDetection).catch(() => {});
  }, []);

  const handleImportEnvKey = async (envProvider: 'anthropic' | 'openai') => {
    setImporting(true);
    try {
      await commands.import_env_key({ provider: envProvider });
      onProviderChange(envProvider);
      onApiKeyChange('(imported from environment)');
    } catch {
      // Silently fail — user can still enter manually
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border space-y-3">
      {/* Environment key detection banner */}
      {envDetection && (envDetection.has_anthropic_env || envDetection.has_openai_env) && (
        <div className="p-3 bg-blue-900/20 border border-blue-500/30 rounded-lg space-y-2">
          <p className="text-sm text-blue-300 font-medium">API keys detected in your environment</p>
          {envDetection.has_anthropic_env && (
            <div className="flex items-center justify-between">
              <span className="text-xs text-text-secondary font-mono">
                Anthropic: {envDetection.anthropic_env_preview}
              </span>
              <button
                onClick={() => handleImportEnvKey('anthropic')}
                disabled={importing}
                className="text-xs px-3 py-1 bg-blue-500/20 text-blue-300 rounded hover:bg-blue-500/30 transition-colors disabled:opacity-50"
              >
                {importing ? 'Importing...' : 'Use This Key'}
              </button>
            </div>
          )}
          {envDetection.has_openai_env && (
            <div className="flex items-center justify-between">
              <span className="text-xs text-text-secondary font-mono">
                OpenAI: {envDetection.openai_env_preview}
              </span>
              <button
                onClick={() => handleImportEnvKey('openai')}
                disabled={importing}
                className="text-xs px-3 py-1 bg-blue-500/20 text-blue-300 rounded hover:bg-blue-500/30 transition-colors disabled:opacity-50"
              >
                {importing ? 'Importing...' : 'Use This Key'}
              </button>
            </div>
          )}
        </div>
      )}

      {/* Ollama detected and ready */}
      {ollamaStatus?.running && ollamaStatus.has_embedding_model && ollamaStatus.has_llm_model && provider === 'ollama' && (
        <div className="p-3 bg-green-900/20 border border-green-500/30 rounded-lg text-sm text-green-300 flex items-center gap-2">
          <span className="text-green-500">&#x2713;</span>
          {t('onboarding.setupAi.localAiReady')}
        </div>
      )}

      {/* Pulling models */}
      {pullingModels && (
        <div className="p-4 border border-orange-500/30 rounded-lg space-y-3">
          <div className="flex items-center gap-2 text-sm text-orange-300">
            <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
            {t('onboarding.apiKeys.installingModels')}
          </div>
          {Object.entries(pullProgress).map(([model, p]) => (
            <div key={model} className="space-y-1">
              <div className="flex items-center justify-between text-xs">
                <span className="text-text-secondary font-mono">{model}</span>
                <span className="text-text-muted">
                  {p.done ? t('onboarding.apiKeys.pullComplete') : p.status || `${p.percent}%`}
                </span>
              </div>
              <div className="w-full h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all duration-300 ${
                    p.done ? 'bg-green-500' : 'bg-orange-500'
                  }`}
                  style={{ width: `${p.done ? 100 : p.percent}%` }}
                />
              </div>
            </div>
          ))}
          <p className="text-xs text-text-muted">
            {t('onboarding.apiKeys.pullWaitMessage')}
          </p>
        </div>
      )}

      {/* Provider selector (shown when Ollama not ready) */}
      {!(ollamaStatus?.running && ollamaStatus.has_embedding_model && ollamaStatus.has_llm_model) && !pullingModels && (
        <>
          <div className="grid grid-cols-3 gap-2">
            {(['ollama', 'anthropic', 'openai'] as const).map((p) => (
              <button
                key={p}
                onClick={() => onProviderChange(p)}
                className={`p-3 rounded-lg text-center transition-all ${
                  provider === p
                    ? 'bg-orange-500/20 border-2 border-orange-500'
                    : 'bg-bg-tertiary border-2 border-transparent hover:border-border'
                }`}
              >
                <div className="text-sm font-medium text-white">
                  {p === 'ollama' ? 'Ollama' : p === 'anthropic' ? 'Anthropic' : 'OpenAI'}
                </div>
                <div className="text-xs text-text-muted mt-1">
                  {p === 'ollama' ? t('onboarding.setupAi.local') : p === 'anthropic' ? 'Claude' : 'GPT-4o'}
                </div>
              </button>
            ))}
          </div>

          {/* API key input for cloud providers */}
          {(provider === 'anthropic' || provider === 'openai') && (
            <div>
              <div className="flex items-center justify-between mb-2">
                <label className="text-xs text-text-muted">
                  {provider === 'anthropic' ? 'Anthropic' : 'OpenAI'} {t('settings.llm.apiKey')}
                </label>
                <a
                  href={provider === 'anthropic'
                    ? 'https://console.anthropic.com/settings/keys'
                    : 'https://platform.openai.com/api-keys'}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-xs text-orange-500 hover:underline"
                >
                  {t('onboarding.apiKeys.getKey')} &rarr;
                </a>
              </div>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => onApiKeyChange(e.target.value)}
                placeholder={provider === 'anthropic' ? 'sk-ant-api03-...' : 'sk-proj-...'}
                className="w-full px-4 py-3 bg-bg-tertiary border border-border rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
              />
            </div>
          )}

          {/* Ollama not running hint */}
          {provider === 'ollama' && !ollamaStatus?.running && (
            <div className="text-text-secondary text-sm p-3 bg-bg-tertiary rounded-lg border border-border">
              <p className="mb-1.5">
                {t('onboarding.setupAi.ollamaNotDetected')}{' '}
                <a href="https://ollama.ai" target="_blank" rel="noopener noreferrer" className="text-orange-500 hover:underline">
                  {t('onboarding.apiKeys.installOllama')}
                </a>
                {' '}{t('onboarding.setupAi.orChooseCloud')}
              </p>
              <p className="text-xs text-text-muted">
                {t('onboarding.setupAi.basicModeHint')}
              </p>
            </div>
          )}

          {/* Cloud provider without key — basic mode hint */}
          {(provider === 'anthropic' || provider === 'openai') && !apiKey.trim() && (
            <div className="text-xs text-text-muted p-3 bg-bg-tertiary rounded-lg border border-border">
              {t('onboarding.setupAi.noKeyHint')}
            </div>
          )}
        </>
      )}

      <p className="text-xs text-text-muted">
        {t('onboarding.setupAi.keywordHint')}
      </p>
    </div>
  );
}
