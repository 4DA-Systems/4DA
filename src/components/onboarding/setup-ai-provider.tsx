import { useTranslation } from 'react-i18next';

import type { OllamaStatus, PullProgress } from './types';

type ProviderType = 'anthropic' | 'openai' | 'ollama';

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

  return (
    <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border space-y-3">
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
                <span className="text-gray-300 font-mono">{model}</span>
                <span className="text-gray-500">
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
          <p className="text-xs text-gray-500">
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
                <div className="text-xs text-gray-500 mt-1">
                  {p === 'ollama' ? t('onboarding.setupAi.local') : p === 'anthropic' ? 'Claude' : 'GPT-4o'}
                </div>
              </button>
            ))}
          </div>

          {/* API key input for cloud providers */}
          {(provider === 'anthropic' || provider === 'openai') && (
            <div>
              <div className="flex items-center justify-between mb-2">
                <label className="text-xs text-gray-500">
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
            <div className="text-yellow-400 text-sm p-3 bg-bg-tertiary rounded-lg">
              {t('onboarding.setupAi.ollamaNotDetected')}{' '}
              <a href="https://ollama.ai" target="_blank" rel="noopener noreferrer" className="text-orange-500 hover:underline">
                {t('onboarding.apiKeys.installOllama')}
              </a>
              {' '}{t('onboarding.setupAi.orChooseCloud')}
            </div>
          )}
        </>
      )}

      <p className="text-xs text-gray-500">
        {t('onboarding.setupAi.keywordHint')}
      </p>
    </div>
  );
}
