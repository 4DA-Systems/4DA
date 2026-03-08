import { useTranslation } from 'react-i18next';
import type { Settings } from '../../types';
import type { OllamaStatus } from '../../hooks/use-settings';

// Provider model options
const providerModels: Record<string, string[]> = {
  anthropic: ['claude-3-haiku-20240307', 'claude-3-sonnet-20240229', 'claude-3-opus-20240229'],
  openai: ['gpt-4o-mini', 'gpt-4o', 'gpt-4-turbo', 'gpt-3.5-turbo'],
  ollama: ['llama3', 'mistral', 'mixtral', 'phi3'],
};

interface SettingsForm {
  provider: string;
  apiKey: string;
  model: string;
  baseUrl: string;
  rerankEnabled: boolean;
  maxItems: number;
  minScore: number;
  dailyTokenLimit: number;
  dailyCostLimit: number;
}

interface AIProviderSectionProps {
  settings: Settings | null;
  settingsForm: SettingsForm;
  setSettingsForm: React.Dispatch<React.SetStateAction<SettingsForm>>;
  ollamaStatus: OllamaStatus | null;
  ollamaModels: string[];
  checkOllamaStatus: (baseUrl?: string) => void;
}

export function AIProviderSection({
  settings,
  settingsForm,
  setSettingsForm,
  ollamaStatus,
  ollamaModels,
  checkOllamaStatus,
}: AIProviderSectionProps) {
  const { t } = useTranslation();
  return (
    <>
      {/* LLM Provider Section */}
      <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
            <span>&#x1f916;</span>
          </div>
          <div>
            <h3 className="text-sm font-medium text-white">{t('settings.ai.title')}</h3>
            <p className="text-xs text-gray-500">{t('settings.ai.description')}</p>
          </div>
        </div>

        <div className="space-y-4">
          <div>
            <label className="text-xs text-gray-500 block mb-2">{t('settings.ai.provider')}</label>
            <select
              value={settingsForm.provider}
              onChange={(e) => {
                const newProvider = e.target.value;
                const defaultModel = newProvider === 'local'
                  ? 'all-MiniLM-L6-v2'
                  : newProvider === 'ollama' && ollamaModels.length > 0
                    ? ollamaModels[0]
                    : providerModels[newProvider]?.[0] || '';
                setSettingsForm((f) => ({
                  ...f,
                  provider: newProvider,
                  model: defaultModel,
                  baseUrl: newProvider === 'ollama' ? 'http://localhost:11434' : '',
                }));
              }}
              className="w-full px-4 py-3 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
            >
              <option value="local">{t('settings.ai.builtInLocal')}</option>
              <option value="anthropic">{t('settings.ai.providerAnthropic')}</option>
              <option value="openai">{t('settings.ai.providerOpenAI')}</option>
              <option value="ollama">{t('settings.ai.providerOllama')}</option>
            </select>
          </div>

          {settingsForm.provider === 'local' && (
            <div className="bg-bg-secondary rounded-lg p-3 border border-green-500/20">
              <p className="text-xs text-green-400 font-medium mb-1">{t('settings.ai.builtInModel')}</p>
              <p className="text-xs text-gray-500">
                {t('settings.ai.builtInDescription')}
              </p>
            </div>
          )}

          {settingsForm.provider !== 'ollama' && settingsForm.provider !== 'local' && (
            <div>
              <label className="text-xs text-gray-500 block mb-2">{t('settings.ai.apiKey')}</label>
              <input
                type="password"
                value={settingsForm.apiKey}
                onChange={(e) => setSettingsForm((f) => ({ ...f, apiKey: e.target.value }))}
                placeholder={settings?.llm.has_api_key ? t('settings.ai.keySaved') : t('settings.ai.enterKey')}
                className="w-full px-4 py-3 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-gray-600 focus:border-orange-500 focus:outline-none font-mono"
              />
            </div>
          )}

          {settingsForm.provider !== 'local' && (
            <div>
              <label className="text-xs text-gray-500 block mb-2">{t('settings.ai.model')}</label>
              <select
                value={settingsForm.model}
                onChange={(e) => setSettingsForm((f) => ({ ...f, model: e.target.value }))}
                className="w-full px-4 py-3 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
              >
                {(settingsForm.provider === 'ollama' && ollamaModels.length > 0
                  ? ollamaModels
                  : providerModels[settingsForm.provider] || []
                ).map((m) => (
                  <option key={m} value={m}>{m}</option>
                ))}
              </select>
              {settingsForm.provider === 'ollama' && (
                <div className="flex items-center gap-2 mt-2">
                  <p className="text-xs text-gray-500">
                    {ollamaStatus?.running
                      ? <span className="text-green-400">&#x2713; {t('settings.ai.ollamaRunning', { version: ollamaStatus.version, count: ollamaModels.length })}</span>
                      : <span className="text-yellow-400">&#x25cb; {t('settings.ai.ollamaNotDetected')}</span>}
                  </p>
                  <button
                    onClick={() => checkOllamaStatus(settingsForm.baseUrl || undefined)}
                    className="text-[10px] px-2 py-0.5 text-gray-500 hover:text-orange-400 bg-bg-tertiary rounded transition-colors"
                  >
                    {t('settings.ai.recheck')}
                  </button>
                </div>
              )}
            </div>
          )}

          {settingsForm.provider === 'ollama' && (
            <div>
              <label className="text-xs text-gray-500 block mb-2">{t('settings.ai.baseUrl')}</label>
              <input
                type="text"
                value={settingsForm.baseUrl}
                onChange={(e) => setSettingsForm((f) => ({ ...f, baseUrl: e.target.value }))}
                placeholder="http://localhost:11434"
                className="w-full px-4 py-3 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-gray-600 focus:border-orange-500 focus:outline-none font-mono"
              />
            </div>
          )}
        </div>
      </div>

      {/* Re-ranking Section */}
      <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
            <span>&#x26a1;</span>
          </div>
          <div>
            <h3 className="text-sm font-medium text-white">{t('settings.ai.rerankTitle')}</h3>
            <p className="text-xs text-gray-500">{t('settings.ai.rerankDescription')}</p>
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
              <p className="text-xs text-gray-500 mt-0.5">{t('settings.ai.rerankNote')}</p>
            </div>
          </label>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs text-gray-500 block mb-2">{t('settings.ai.maxItemsBatch')}</label>
              <input
                type="number"
                value={settingsForm.maxItems}
                onChange={(e) => setSettingsForm((f) => ({ ...f, maxItems: parseInt(e.target.value) || 15 }))}
                className="w-full px-4 py-3 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
              />
            </div>
            <div>
              <label className="text-xs text-gray-500 block mb-2">{t('settings.ai.minScore')}</label>
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
              <label className="text-xs text-gray-500 block mb-2">{t('settings.ai.dailyTokenLimit')}</label>
              <input
                type="number"
                value={settingsForm.dailyTokenLimit}
                onChange={(e) => setSettingsForm((f) => ({ ...f, dailyTokenLimit: parseInt(e.target.value) || 100000 }))}
                className="w-full px-4 py-3 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
              />
            </div>
            <div>
              <label className="text-xs text-gray-500 block mb-2">{t('settings.ai.costLimit')}</label>
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

      {/* Usage Stats */}
      {settings && (
        <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
          <div className="flex items-center gap-3 mb-4">
            <div className="w-8 h-8 bg-green-500/20 rounded-lg flex items-center justify-center">
              <span>&#x1f4c8;</span>
            </div>
            <div>
              <h3 className="text-sm font-medium text-white">{t('settings.ai.usageTitle')}</h3>
              <p className="text-xs text-gray-500">{t('settings.ai.usageDescription')}</p>
            </div>
          </div>
          <div className="grid grid-cols-3 gap-4">
            <div className="bg-bg-secondary rounded-lg p-3 text-center">
              <p className="text-xl font-semibold text-white">{settings.usage.tokens_today.toLocaleString()}</p>
              <p className="text-xs text-gray-500">{t('settings.ai.tokens')}</p>
            </div>
            <div className="bg-bg-secondary rounded-lg p-3 text-center">
              <p className="text-xl font-semibold text-green-400">${(settings.usage.cost_today_cents / 100).toFixed(2)}</p>
              <p className="text-xs text-gray-500">{t('settings.ai.cost')}</p>
            </div>
            <div className="bg-bg-secondary rounded-lg p-3 text-center">
              <p className="text-xl font-semibold text-orange-400">{settings.usage.items_reranked}</p>
              <p className="text-xs text-gray-500">{t('settings.ai.reranked')}</p>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
