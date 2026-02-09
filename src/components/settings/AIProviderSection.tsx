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
  return (
    <>
      {/* LLM Provider Section */}
      <div className="bg-[#1F1F1F] rounded-lg p-5 border border-[#2A2A2A]">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
            <span>&#x1f916;</span>
          </div>
          <div>
            <h3 className="text-sm font-medium text-white">AI Provider</h3>
            <p className="text-xs text-gray-500">Choose your LLM provider</p>
          </div>
        </div>

        <div className="space-y-4">
          <div>
            <label className="text-xs text-gray-500 block mb-2">Provider</label>
            <select
              value={settingsForm.provider}
              onChange={(e) => {
                const newProvider = e.target.value;
                const defaultModel = newProvider === 'ollama' && ollamaModels.length > 0
                  ? ollamaModels[0]
                  : providerModels[newProvider]?.[0] || '';
                setSettingsForm((f) => ({
                  ...f,
                  provider: newProvider,
                  model: defaultModel,
                  baseUrl: newProvider === 'ollama' ? 'http://localhost:11434' : '',
                }));
              }}
              className="w-full px-4 py-3 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
            >
              <option value="anthropic">Anthropic Claude</option>
              <option value="openai">OpenAI</option>
              <option value="ollama">Ollama (Local)</option>
            </select>
          </div>

          {settingsForm.provider !== 'ollama' && (
            <div>
              <label className="text-xs text-gray-500 block mb-2">API Key</label>
              <input
                type="password"
                value={settingsForm.apiKey}
                onChange={(e) => setSettingsForm((f) => ({ ...f, apiKey: e.target.value }))}
                placeholder={settings?.llm.has_api_key ? '(key saved)' : 'Enter your API key'}
                className="w-full px-4 py-3 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-600 focus:border-orange-500 focus:outline-none font-mono"
              />
            </div>
          )}

          <div>
            <label className="text-xs text-gray-500 block mb-2">Model</label>
            <select
              value={settingsForm.model}
              onChange={(e) => setSettingsForm((f) => ({ ...f, model: e.target.value }))}
              className="w-full px-4 py-3 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
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
                    ? <span className="text-green-400">&#x2713; Ollama v{ollamaStatus.version} - {ollamaModels.length} models</span>
                    : <span className="text-yellow-400">&#x25cb; Ollama not detected</span>}
                </p>
                <button
                  onClick={() => checkOllamaStatus(settingsForm.baseUrl || undefined)}
                  className="text-[10px] px-2 py-0.5 text-gray-500 hover:text-orange-400 bg-[#1F1F1F] rounded transition-colors"
                >
                  Re-check
                </button>
              </div>
            )}
          </div>

          {settingsForm.provider === 'ollama' && (
            <div>
              <label className="text-xs text-gray-500 block mb-2">Base URL</label>
              <input
                type="text"
                value={settingsForm.baseUrl}
                onChange={(e) => setSettingsForm((f) => ({ ...f, baseUrl: e.target.value }))}
                placeholder="http://localhost:11434"
                className="w-full px-4 py-3 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-600 focus:border-orange-500 focus:outline-none font-mono"
              />
            </div>
          )}
        </div>
      </div>

      {/* Re-ranking Section */}
      <div className="bg-[#1F1F1F] rounded-lg p-5 border border-[#2A2A2A]">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
            <span>&#x26a1;</span>
          </div>
          <div>
            <h3 className="text-sm font-medium text-white">LLM Re-ranking</h3>
            <p className="text-xs text-gray-500">Deeper analysis of top candidates</p>
          </div>
        </div>

        <div className="space-y-4">
          <label className="flex items-center gap-3 cursor-pointer p-3 bg-[#141414] rounded-lg border border-[#2A2A2A] hover:border-orange-500/30 transition-all">
            <input
              type="checkbox"
              checked={settingsForm.rerankEnabled}
              onChange={(e) => setSettingsForm((f) => ({ ...f, rerankEnabled: e.target.checked }))}
              className="w-5 h-5 accent-orange-500 rounded"
            />
            <div>
              <span className="text-sm text-white">Enable LLM re-ranking</span>
              <p className="text-xs text-gray-500 mt-0.5">Improves precision but uses API tokens</p>
            </div>
          </label>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs text-gray-500 block mb-2">Max items/batch</label>
              <input
                type="number"
                value={settingsForm.maxItems}
                onChange={(e) => setSettingsForm((f) => ({ ...f, maxItems: parseInt(e.target.value) || 15 }))}
                className="w-full px-4 py-3 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
              />
            </div>
            <div>
              <label className="text-xs text-gray-500 block mb-2">Min score</label>
              <input
                type="number"
                step="0.05"
                value={settingsForm.minScore}
                onChange={(e) => setSettingsForm((f) => ({ ...f, minScore: parseFloat(e.target.value) || 0.25 }))}
                className="w-full px-4 py-3 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
              />
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs text-gray-500 block mb-2">Daily token limit</label>
              <input
                type="number"
                value={settingsForm.dailyTokenLimit}
                onChange={(e) => setSettingsForm((f) => ({ ...f, dailyTokenLimit: parseInt(e.target.value) || 100000 }))}
                className="w-full px-4 py-3 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
              />
            </div>
            <div>
              <label className="text-xs text-gray-500 block mb-2">Cost limit (&cent;/day)</label>
              <input
                type="number"
                value={settingsForm.dailyCostLimit}
                onChange={(e) => setSettingsForm((f) => ({ ...f, dailyCostLimit: parseInt(e.target.value) || 50 }))}
                className="w-full px-4 py-3 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
              />
            </div>
          </div>
        </div>
      </div>

      {/* Usage Stats */}
      {settings && (
        <div className="bg-[#1F1F1F] rounded-lg p-5 border border-[#2A2A2A]">
          <div className="flex items-center gap-3 mb-4">
            <div className="w-8 h-8 bg-green-500/20 rounded-lg flex items-center justify-center">
              <span>&#x1f4c8;</span>
            </div>
            <div>
              <h3 className="text-sm font-medium text-white">Usage Today</h3>
              <p className="text-xs text-gray-500">Token consumption</p>
            </div>
          </div>
          <div className="grid grid-cols-3 gap-4">
            <div className="bg-[#141414] rounded-lg p-3 text-center">
              <p className="text-xl font-semibold text-white">{settings.usage.tokens_today.toLocaleString()}</p>
              <p className="text-xs text-gray-500">Tokens</p>
            </div>
            <div className="bg-[#141414] rounded-lg p-3 text-center">
              <p className="text-xl font-semibold text-green-400">${(settings.usage.cost_today_cents / 100).toFixed(2)}</p>
              <p className="text-xs text-gray-500">Cost</p>
            </div>
            <div className="bg-[#141414] rounded-lg p-3 text-center">
              <p className="text-xl font-semibold text-orange-400">{settings.usage.items_reranked}</p>
              <p className="text-xs text-gray-500">Re-ranked</p>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
