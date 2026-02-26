import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import type { ApiKeyState, OllamaStatus, PullProgress } from './types';

interface ApiKeysStepProps {
  isAnimating: boolean;
  apiKeys: ApiKeyState;
  setApiKeys: React.Dispatch<React.SetStateAction<ApiKeyState>>;
  ollamaStatus: OllamaStatus | null;
  isCheckingOllama: boolean;
  checkOllamaStatus: () => void;
  selectedOllamaModel: string;
  setSelectedOllamaModel: (model: string) => void;
  isTesting: boolean;
  testResult: { success: boolean; message: string } | null;
  setTestResult: (result: { success: boolean; message: string } | null) => void;
  error: string | null;
  setError: (error: string | null) => void;
  onTest: () => void;
  onSave: () => void;
  onSkip: () => void;
  onBack: () => void;
  pullingModels: boolean;
  pullProgress: Record<string, PullProgress>;
}

export function ApiKeysStep({
  isAnimating,
  apiKeys,
  setApiKeys,
  ollamaStatus,
  isCheckingOllama,
  checkOllamaStatus,
  selectedOllamaModel,
  setSelectedOllamaModel,
  isTesting,
  testResult,
  setTestResult,
  onTest,
  onSave,
  onSkip,
  onBack,
  pullingModels,
  pullProgress,
}: ApiKeysStepProps) {
  const { t } = useTranslation();

  // Auto-detect Ollama on mount
  useEffect(() => {
    checkOllamaStatus();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const hasEmbeddingService = apiKeys.openai || (ollamaStatus?.running ?? false);

  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h2 className="text-3xl font-semibold text-white mb-2 text-center">{t('onboarding.apiKeys.title')}</h2>
      <p className="text-gray-400 mb-6 text-center">
        {t('onboarding.apiKeys.subtitle')}
      </p>

      <div className="space-y-4 mb-6">
        {/* Ollama status banner */}
        {ollamaStatus?.running && apiKeys.provider === 'ollama' && (
          <>
            {pullingModels ? (
              <div className="p-4 bg-bg-secondary border border-orange-500/30 rounded-lg space-y-3">
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
            ) : ollamaStatus.has_embedding_model && ollamaStatus.has_llm_model ? (
              <div className="p-3 bg-green-900/20 border border-green-500/30 rounded-lg text-sm text-green-300 flex items-center gap-2">
                <span className="text-green-500">&#x2713;</span>
                {t('onboarding.apiKeys.ollamaReady')}
              </div>
            ) : ollamaStatus.models.length === 0 && !pullingModels ? (
              <div className="p-3 bg-yellow-900/20 border border-yellow-500/30 rounded-lg text-sm text-yellow-300 flex items-center gap-2">
                <span className="text-yellow-500">&#x26A0;</span>
                {t('onboarding.apiKeys.ollamaNoModels')}
              </div>
            ) : (
              <div className="p-3 bg-green-900/20 border border-green-500/30 rounded-lg text-sm text-green-300 flex items-center gap-2">
                <span className="text-green-500">&#x2713;</span>
                {t('onboarding.apiKeys.ollamaDetected', { count: ollamaStatus.models.length })}
              </div>
            )}
          </>
        )}

        {/* OpenAI Key */}
        <div className="bg-bg-secondary p-5 rounded-lg">
          <div className="flex items-center gap-2 mb-3">
            {hasEmbeddingService ? (
              <span className="px-2 py-0.5 bg-green-500/20 text-green-400 text-xs rounded font-medium">{t('onboarding.apiKeys.recommended')}</span>
            ) : (
              <span className="px-2 py-0.5 bg-yellow-500/20 text-yellow-400 text-xs rounded font-medium">{t('onboarding.apiKeys.recommended')}</span>
            )}
            <h3 className="text-white font-medium">{t('onboarding.apiKeys.openaiLabel')}</h3>
          </div>
          <p className="text-sm text-gray-400 mb-3">
            {t('onboarding.apiKeys.openaiDesc')}
          </p>
          <div className="flex items-center justify-between mb-2">
            <label className="text-xs text-gray-500">{t('settings.llm.apiKey')}</label>
            <a
              href="https://platform.openai.com/api-keys"
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs text-orange-500 hover:underline"
            >
              {t('onboarding.apiKeys.getApiKey')} &rarr;
            </a>
          </div>
          <input
            type="password"
            value={apiKeys.openai}
            onChange={(e) => {
              setApiKeys({ ...apiKeys, openai: e.target.value });
              setTestResult(null);
            }}
            placeholder="sk-proj-..."
            className="w-full px-4 py-3 bg-bg-tertiary border border-border rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
          />
          <p className="text-xs text-gray-500 mt-2">
            {t('onboarding.apiKeys.openaiCostHint')}
          </p>
        </div>

        {/* LLM Provider - for analysis/reasoning */}
        <div className="bg-bg-secondary p-5 rounded-lg">
          <div className="flex items-center gap-2 mb-3">
            <span className="px-2 py-0.5 bg-blue-500/20 text-blue-400 text-xs rounded font-medium">{t('onboarding.apiKeys.analysisTag')}</span>
            <h3 className="text-white font-medium">{t('onboarding.apiKeys.llmProviderLabel')}</h3>
          </div>
          <p className="text-sm text-gray-400 mb-3">
            {t('onboarding.apiKeys.llmProviderDesc')}
          </p>

          {/* Provider selection */}
          <div className="grid grid-cols-3 gap-2 mb-3">
            {(['openai', 'anthropic', 'ollama'] as const).map((p) => (
              <button
                key={p}
                onClick={() => {
                  setApiKeys({ ...apiKeys, provider: p });
                  setTestResult(null);
                  if (p === 'ollama') checkOllamaStatus();
                }}
                className={`p-3 rounded-lg text-center transition-all ${
                  apiKeys.provider === p
                    ? 'bg-orange-500/20 border-2 border-orange-500'
                    : 'bg-bg-tertiary border-2 border-transparent hover:border-border'
                }`}
              >
                <div className="text-sm font-medium text-white">
                  {p === 'openai' ? 'OpenAI' : p === 'anthropic' ? 'Anthropic' : 'Ollama'}
                </div>
                <div className="text-xs text-gray-500 mt-1">
                  {p === 'openai' ? 'GPT-4o' : p === 'anthropic' ? 'Claude' : 'Local'}
                </div>
              </button>
            ))}
          </div>

          {/* Provider-specific config */}
          {apiKeys.provider === 'anthropic' && (
            <div className="mt-3">
              <div className="flex items-center justify-between mb-2">
                <label className="text-xs text-gray-500">{t('onboarding.apiKeys.anthropicLabel')}</label>
                <a
                  href="https://console.anthropic.com/settings/keys"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-xs text-orange-500 hover:underline"
                >
                  {t('onboarding.apiKeys.getKey')} &rarr;
                </a>
              </div>
              <input
                type="password"
                value={apiKeys.anthropic}
                onChange={(e) => {
                  setApiKeys({ ...apiKeys, anthropic: e.target.value });
                  setTestResult(null);
                }}
                placeholder="sk-ant-api03-..."
                className="w-full px-4 py-3 bg-bg-tertiary border border-border rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
              />
            </div>
          )}

          {apiKeys.provider === 'ollama' && (
            <div className="mt-3 p-3 bg-bg-tertiary rounded-lg">
              {isCheckingOllama ? (
                <div className="flex items-center gap-2">
                  <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
                  <span className="text-gray-400 text-sm">{t('onboarding.apiKeys.checkingOllama')}</span>
                </div>
              ) : ollamaStatus?.running ? (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <span className="text-green-500">&#x2713;</span>
                    <span className="text-green-300 text-sm">{t('onboarding.apiKeys.ollamaRunning', { version: ollamaStatus.version })}</span>
                  </div>
                  {ollamaStatus.models.length > 0 && (
                    <select
                      value={selectedOllamaModel}
                      onChange={(e) => setSelectedOllamaModel(e.target.value)}
                      className="w-full px-3 py-2 bg-bg-secondary border border-border rounded text-white text-sm"
                    >
                      {ollamaStatus.models.map((model) => (
                        <option key={model} value={model}>{model}</option>
                      ))}
                    </select>
                  )}
                </div>
              ) : (
                <div className="text-yellow-400 text-sm">
                  {t('onboarding.apiKeys.ollamaNotDetected')}{' '}
                  <a href="https://ollama.ai" target="_blank" rel="noopener noreferrer" className="text-orange-500 hover:underline">
                    {t('onboarding.apiKeys.installOllama')}
                  </a>
                  {' '}{t('onboarding.apiKeys.or')}{' '}
                  <button onClick={checkOllamaStatus} className="text-orange-500 hover:underline">{t('action.retry').toLowerCase()}</button>
                </div>
              )}
            </div>
          )}

          {apiKeys.provider === 'openai' && (
            <p className="text-xs text-gray-500 mt-2">
              {t('onboarding.apiKeys.openaiSharedKeyHint')}
            </p>
          )}
        </div>

        {/* X/Twitter API Key - Optional */}
        <div className="bg-bg-secondary p-5 rounded-lg">
          <div className="flex items-center gap-2 mb-3">
            <span className="px-2 py-0.5 bg-gray-500/20 text-gray-400 text-xs rounded font-medium">{t('onboarding.apiKeys.optionalTag')}</span>
            <h3 className="text-white font-medium">{t('onboarding.apiKeys.xApiLabel')}</h3>
          </div>
          <p className="text-sm text-gray-400 mb-3">
            {t('onboarding.apiKeys.xApiDesc')}
          </p>
          <div className="flex items-center justify-between mb-2">
            <label className="text-xs text-gray-500">{t('onboarding.apiKeys.bearerToken')}</label>
            <a
              href="https://developer.x.com/en/portal/dashboard"
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs text-orange-500 hover:underline"
            >
              {t('onboarding.apiKeys.getXAccess')} &rarr;
            </a>
          </div>
          <input
            type="password"
            value={apiKeys.xApiKey}
            onChange={(e) => setApiKeys({ ...apiKeys, xApiKey: e.target.value })}
            placeholder="Bearer token..."
            className="w-full px-4 py-3 bg-bg-tertiary border border-border rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
          />
          <p className="text-xs text-gray-500 mt-2">
            {t('onboarding.apiKeys.xApiSkipHint')}
          </p>
        </div>

        {/* Test connection button */}
        <button
          onClick={onTest}
          disabled={isTesting || (!apiKeys.openai && apiKeys.provider !== 'ollama' && !ollamaStatus?.running)}
          className="w-full px-4 py-3 bg-bg-tertiary border border-border text-gray-300 rounded-lg hover:bg-border transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        >
          {isTesting ? (
            <>
              <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
              {t('onboarding.apiKeys.testingConnection')}
            </>
          ) : (
            <>
              <span>&#x1f50c;</span>
              {t('settings.testConnection')}
            </>
          )}
        </button>

        {/* Test result */}
        {testResult && (
          <div className={`p-3 rounded-lg text-sm ${
            testResult.success
              ? 'bg-green-900/30 text-green-300 border border-green-500/30'
              : 'bg-red-900/30 text-red-300 border border-red-500/30'
          }`}>
            {testResult.success ? (
              <div className="flex items-center gap-2">
                <span className="text-green-500">&#x2713;</span>
                {t('onboarding.apiKeys.connectionSuccess')}
              </div>
            ) : (
              <div>
                <div className="font-medium">{t('onboarding.apiKeys.connectionFailed')}</div>
                <div className="text-xs mt-1 opacity-80">{testResult.message}</div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Keyword-only mode info */}
      {!hasEmbeddingService && (
        <div className="p-3 bg-bg-tertiary border border-border rounded-lg text-sm">
          <div className="text-yellow-400 font-medium mb-1">{t('onboarding.apiKeys.keywordModeTitle')}</div>
          <p className="text-gray-400 text-xs">
            {t('onboarding.apiKeys.keywordModeDesc')}
          </p>
        </div>
      )}

      {/* Navigation */}
      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          className="px-6 py-2 text-gray-400 hover:text-white transition-colors"
        >
          &larr; {t('onboarding.nav.back')}
        </button>
        <div className="flex items-center gap-3">
          {!hasEmbeddingService && (
            <button
              onClick={onSkip}
              className="px-4 py-2 text-yellow-500 hover:text-yellow-300 text-sm transition-colors"
            >
              {t('onboarding.apiKeys.tryKeywordMode')}
            </button>
          )}
          {hasEmbeddingService && !apiKeys.openai && apiKeys.provider !== 'ollama' && (
            <button
              onClick={onSkip}
              className="px-4 py-2 text-gray-500 hover:text-gray-300 text-sm transition-colors"
            >
              {t('onboarding.nav.skipForNow')}
            </button>
          )}
          <button
            onClick={onSave}
            disabled={pullingModels || (!apiKeys.openai && apiKeys.provider !== 'ollama')}
            className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {pullingModels ? t('onboarding.apiKeys.installingModelsBtn') : t('onboarding.nav.continue')}
          </button>
        </div>
      </div>
    </div>
  );
}
