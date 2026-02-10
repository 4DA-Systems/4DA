import { useEffect } from 'react';
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
  // Auto-detect Ollama on mount
  useEffect(() => {
    checkOllamaStatus();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const hasEmbeddingService = apiKeys.openai || (ollamaStatus?.running ?? false);

  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h2 className="text-3xl font-semibold text-white mb-2 text-center">API Keys</h2>
      <p className="text-gray-400 mb-6 text-center">
        Configure API keys for best results, or try keyword-only mode.
      </p>

      <div className="space-y-4 mb-6">
        {/* Ollama status banner */}
        {ollamaStatus?.running && !apiKeys.openai && (
          <>
            {pullingModels ? (
              <div className="p-4 bg-[#141414] border border-orange-500/30 rounded-lg space-y-3">
                <div className="flex items-center gap-2 text-sm text-orange-300">
                  <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
                  Installing models for local AI...
                </div>
                {Object.entries(pullProgress).map(([model, p]) => (
                  <div key={model} className="space-y-1">
                    <div className="flex items-center justify-between text-xs">
                      <span className="text-gray-300 font-mono">{model}</span>
                      <span className="text-gray-500">
                        {p.done ? 'Complete' : p.status || `${p.percent}%`}
                      </span>
                    </div>
                    <div className="w-full h-1.5 bg-[#1F1F1F] rounded-full overflow-hidden">
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
                  This may take a few minutes depending on your connection.
                </p>
              </div>
            ) : ollamaStatus.has_embedding_model && ollamaStatus.has_llm_model ? (
              <div className="p-3 bg-green-900/20 border border-green-500/30 rounded-lg text-sm text-green-300 flex items-center gap-2">
                <span className="text-green-500">&#x2713;</span>
                Ollama ready — no API keys needed.
              </div>
            ) : ollamaStatus.models.length === 0 && !pullingModels ? (
              <div className="p-3 bg-yellow-900/20 border border-yellow-500/30 rounded-lg text-sm text-yellow-300 flex items-center gap-2">
                <span className="text-yellow-500">&#x26A0;</span>
                Ollama detected but no models installed. Models will be downloaded automatically.
              </div>
            ) : (
              <div className="p-3 bg-green-900/20 border border-green-500/30 rounded-lg text-sm text-green-300 flex items-center gap-2">
                <span className="text-green-500">&#x2713;</span>
                Ollama detected with {ollamaStatus.models.length} model{ollamaStatus.models.length !== 1 ? 's' : ''}.
              </div>
            )}
          </>
        )}

        {/* OpenAI Key */}
        <div className="bg-[#141414] p-5 rounded-lg">
          <div className="flex items-center gap-2 mb-3">
            {hasEmbeddingService ? (
              <span className="px-2 py-0.5 bg-green-500/20 text-green-400 text-xs rounded font-medium">Recommended</span>
            ) : (
              <span className="px-2 py-0.5 bg-yellow-500/20 text-yellow-400 text-xs rounded font-medium">Recommended</span>
            )}
            <h3 className="text-white font-medium">OpenAI API Key</h3>
          </div>
          <p className="text-sm text-gray-400 mb-3">
            Used for semantic understanding (embeddings). Provides the best relevance scoring.
          </p>
          <div className="flex items-center justify-between mb-2">
            <label className="text-xs text-gray-500">API Key</label>
            <a
              href="https://platform.openai.com/api-keys"
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs text-orange-500 hover:underline"
            >
              Get your API key &rarr;
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
            className="w-full px-4 py-3 bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
          />
          <p className="text-xs text-gray-500 mt-2">
            ~$0.02/1M tokens for embeddings. Typical usage costs pennies/month.
          </p>
        </div>

        {/* LLM Provider - for analysis/reasoning */}
        <div className="bg-[#141414] p-5 rounded-lg">
          <div className="flex items-center gap-2 mb-3">
            <span className="px-2 py-0.5 bg-blue-500/20 text-blue-400 text-xs rounded font-medium">Analysis</span>
            <h3 className="text-white font-medium">LLM Provider</h3>
          </div>
          <p className="text-sm text-gray-400 mb-3">
            Choose which AI to use for analyzing and explaining relevance.
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
                    : 'bg-[#1F1F1F] border-2 border-transparent hover:border-[#2A2A2A]'
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
                <label className="text-xs text-gray-500">Anthropic API Key</label>
                <a
                  href="https://console.anthropic.com/settings/keys"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-xs text-orange-500 hover:underline"
                >
                  Get key &rarr;
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
                className="w-full px-4 py-3 bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
              />
            </div>
          )}

          {apiKeys.provider === 'ollama' && (
            <div className="mt-3 p-3 bg-[#1F1F1F] rounded-lg">
              {isCheckingOllama ? (
                <div className="flex items-center gap-2">
                  <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
                  <span className="text-gray-400 text-sm">Checking Ollama...</span>
                </div>
              ) : ollamaStatus?.running ? (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <span className="text-green-500">&#x2713;</span>
                    <span className="text-green-300 text-sm">Ollama running (v{ollamaStatus.version})</span>
                  </div>
                  {ollamaStatus.models.length > 0 && (
                    <select
                      value={selectedOllamaModel}
                      onChange={(e) => setSelectedOllamaModel(e.target.value)}
                      className="w-full px-3 py-2 bg-[#141414] border border-[#2A2A2A] rounded text-white text-sm"
                    >
                      {ollamaStatus.models.map((model) => (
                        <option key={model} value={model}>{model}</option>
                      ))}
                    </select>
                  )}
                </div>
              ) : (
                <div className="text-yellow-400 text-sm">
                  Ollama not detected.{' '}
                  <a href="https://ollama.ai" target="_blank" rel="noopener noreferrer" className="text-orange-500 hover:underline">
                    Install Ollama
                  </a>
                  {' '}or{' '}
                  <button onClick={checkOllamaStatus} className="text-orange-500 hover:underline">retry</button>
                </div>
              )}
            </div>
          )}

          {apiKeys.provider === 'openai' && (
            <p className="text-xs text-gray-500 mt-2">
              Using same OpenAI key for both embeddings and analysis.
            </p>
          )}
        </div>

        {/* X/Twitter API Key - Optional */}
        <div className="bg-[#141414] p-5 rounded-lg">
          <div className="flex items-center gap-2 mb-3">
            <span className="px-2 py-0.5 bg-gray-500/20 text-gray-400 text-xs rounded font-medium">Optional</span>
            <h3 className="text-white font-medium">X / Twitter API Key</h3>
          </div>
          <p className="text-sm text-gray-400 mb-3">
            Enables monitoring tweets from tech influencers. Requires a free X Developer account.
          </p>
          <div className="flex items-center justify-between mb-2">
            <label className="text-xs text-gray-500">Bearer Token</label>
            <a
              href="https://developer.x.com/en/portal/dashboard"
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs text-orange-500 hover:underline"
            >
              Get X Developer access &rarr;
            </a>
          </div>
          <input
            type="password"
            value={apiKeys.xApiKey}
            onChange={(e) => setApiKeys({ ...apiKeys, xApiKey: e.target.value })}
            placeholder="Bearer token..."
            className="w-full px-4 py-3 bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
          />
          <p className="text-xs text-gray-500 mt-2">
            Skip this if you don't have an X developer account. You can add it later in Settings.
          </p>
        </div>

        {/* Test connection button */}
        <button
          onClick={onTest}
          disabled={isTesting || (!apiKeys.openai && apiKeys.provider !== 'ollama' && !ollamaStatus?.running)}
          className="w-full px-4 py-3 bg-[#1F1F1F] border border-[#2A2A2A] text-gray-300 rounded-lg hover:bg-[#2A2A2A] transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        >
          {isTesting ? (
            <>
              <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
              Testing connection...
            </>
          ) : (
            <>
              <span>&#x1f50c;</span>
              Test Connection
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
                Connection successful! Ready to use.
              </div>
            ) : (
              <div>
                <div className="font-medium">Connection failed</div>
                <div className="text-xs mt-1 opacity-80">{testResult.message}</div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Keyword-only mode info */}
      {!hasEmbeddingService && (
        <div className="p-3 bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg text-sm">
          <div className="text-yellow-400 font-medium mb-1">Keyword-only mode available</div>
          <p className="text-gray-400 text-xs">
            Without an API key or Ollama, 4DA can still work using keyword matching.
            Results will be less precise than semantic search, but still useful.
            You can add API keys later in Settings.
          </p>
        </div>
      )}

      {/* Navigation */}
      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          className="px-6 py-2 text-gray-400 hover:text-white transition-colors"
        >
          &larr; Back
        </button>
        <div className="flex items-center gap-3">
          {!hasEmbeddingService && (
            <button
              onClick={onSkip}
              className="px-4 py-2 text-yellow-500 hover:text-yellow-300 text-sm transition-colors"
            >
              Try keyword-only mode
            </button>
          )}
          {hasEmbeddingService && !apiKeys.openai && apiKeys.provider !== 'ollama' && (
            <button
              onClick={onSkip}
              className="px-4 py-2 text-gray-500 hover:text-gray-300 text-sm transition-colors"
            >
              Skip for now
            </button>
          )}
          <button
            onClick={onSave}
            disabled={pullingModels || (!apiKeys.openai && apiKeys.provider !== 'ollama')}
            className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {pullingModels ? 'Installing models...' : 'Continue'}
          </button>
        </div>
      </div>
    </div>
  );
}
