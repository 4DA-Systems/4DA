// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import { ReRankingSection } from './ReRankingSection';
import { UsageStatsSection } from './UsageStatsSection';
import type { SettingsForm, AIProviderSectionProps } from './ai-provider-types';
import type { ModelRegistryData } from '../../store/types';

export type { SettingsForm, AIProviderSectionProps };

// Fallback provider model options (used while registry loads)
const fallbackModels: Record<string, string[]> = {
  anthropic: ['claude-haiku-4-5-20251001', 'claude-sonnet-4-6', 'claude-opus-4-6'],
  openai: ['gpt-4.1-nano', 'gpt-4.1-mini', 'gpt-4.1', 'gpt-4o-mini', 'gpt-4o'],
  ollama: ['llama3.2', 'gemma3', 'qwen2.5', 'deepseek-r1', 'mistral', 'phi4'],
};

// Popular OpenAI-compatible endpoints
const popularEndpoints: { name: string; url: string }[] = [
  { name: 'Groq', url: 'https://api.groq.com/openai/v1' },
  { name: 'Together', url: 'https://api.together.xyz/v1' },
  { name: 'DeepSeek', url: 'https://api.deepseek.com/v1' },
  { name: 'Mistral', url: 'https://api.mistral.ai/v1' },
  { name: 'OpenRouter', url: 'https://openrouter.ai/api/v1' },
  { name: 'LM Studio', url: 'http://localhost:1234/v1' },
  { name: 'llama.cpp', url: 'http://localhost:8080/v1' },
];

/** Get models for a provider from registry, falling back to hardcoded defaults. */
function getProviderModels(provider: string, registry: ModelRegistryData | null | undefined): string[] {
  if (registry && registry.providers[provider]?.length) {
    return registry.providers[provider]!.map(m => m.id);
  }
  return fallbackModels[provider] ?? [];
}

/** Format registry freshness as human-readable string. */
function formatFreshness(fetchedAt: number): string {
  if (fetchedAt === 0) return 'bundled defaults';
  const now = Math.floor(Date.now() / 1000);
  const diffSecs = now - fetchedAt;
  if (diffSecs < 60) return 'just now';
  if (diffSecs < 3600) return `${Math.floor(diffSecs / 60)}m ago`;
  if (diffSecs < 86400) return `${Math.floor(diffSecs / 3600)}h ago`;
  return `${Math.floor(diffSecs / 86400)}d ago`;
}

export function AIProviderSection({
  settings,
  settingsForm,
  setSettingsForm,
  ollamaStatus,
  ollamaModels,
  checkOllamaStatus,
  modelRegistry,
  onRefreshRegistry,
}: AIProviderSectionProps) {
  const { t } = useTranslation();

  // Environment detection state
  const [envDetection, setEnvDetection] = useState<{
    has_anthropic_env: boolean;
    anthropic_env_preview: string;
    has_openai_env: boolean;
    openai_env_preview: string;
  } | null>(null);
  const [importing, setImporting] = useState(false);

  // Key validation state
  const [validation, setValidation] = useState<{
    status: 'idle' | 'checking' | 'valid' | 'invalid' | 'format_error';
    message: string;
    models: string[];
  }>({ status: 'idle', message: '', models: [] });
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  // Detect env keys on mount
  useEffect(() => {
    cmd('detect_environment').then(setEnvDetection).catch((e) => {
      console.warn('[4DA] Environment detection failed:', e);
    });
  }, []);

  // Debounced key validation
  const validateKey = useCallback((provider: string, key: string, baseUrl?: string) => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (!key.trim() || key === '(imported from environment)') {
      setValidation({ status: 'idle', message: '', models: [] });
      return;
    }
    debounceRef.current = setTimeout(async () => {
      setValidation({ status: 'checking', message: 'Verifying...', models: [] });
      try {
        const result = await cmd('validate_api_key', {
          provider,
          key,
          baseUrl: baseUrl || null,
        });
        if (result.valid) {
          setValidation({
            status: 'valid',
            message: result.model_access.length > 0
              ? `Key verified \u2014 access to ${result.model_access.join(', ')}`
              : 'Key verified',
            models: result.model_access,
          });
        } else if (!result.format_ok) {
          setValidation({
            status: 'format_error',
            message: result.error || 'Invalid key format',
            models: [],
          });
        } else {
          setValidation({
            status: 'invalid',
            message: result.error || 'Connection failed',
            models: [],
          });
        }
      } catch {
        setValidation({ status: 'idle', message: '', models: [] });
      }
    }, 500);
  }, []);

  const handleImportEnv = async (provider: 'anthropic' | 'openai') => {
    setImporting(true);
    try {
      await cmd('import_env_key', { provider });
      setSettingsForm((f) => ({ ...f, provider, apiKey: '(imported from environment)' }));
    } catch { /* user can still enter manually */ }
    finally { setImporting(false); }
  };

  return (
    <>
      {/* LLM Provider Section */}
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <div className="flex items-center gap-3 mb-3">
          <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
            <span>&#x1f916;</span>
          </div>
          <div>
            <h3 className="text-sm font-medium text-white">{t('settings.ai.title')}</h3>
            <p className="text-xs text-text-muted">{t('settings.ai.description')}</p>
          </div>
        </div>

        <div className="space-y-3">
          {/* Environment key import banner */}
          {envDetection && (envDetection.has_anthropic_env || envDetection.has_openai_env) && (
            <div className="p-3 bg-blue-900/20 border border-blue-500/30 rounded-lg space-y-2">
              <p className="text-xs text-blue-300 font-medium">Import from environment</p>
              {envDetection.has_anthropic_env && (
                <div className="flex items-center justify-between">
                  <span className="text-xs text-text-secondary font-mono">{envDetection.anthropic_env_preview}</span>
                  <button onClick={() => handleImportEnv('anthropic')} disabled={importing}
                    className="text-xs px-2 py-0.5 bg-blue-500/20 text-blue-300 rounded hover:bg-blue-500/30 transition-colors disabled:opacity-50">
                    {importing ? '...' : 'Import'}
                  </button>
                </div>
              )}
              {envDetection.has_openai_env && (
                <div className="flex items-center justify-between">
                  <span className="text-xs text-text-secondary font-mono">{envDetection.openai_env_preview}</span>
                  <button onClick={() => handleImportEnv('openai')} disabled={importing}
                    className="text-xs px-2 py-0.5 bg-blue-500/20 text-blue-300 rounded hover:bg-blue-500/30 transition-colors disabled:opacity-50">
                    {importing ? '...' : 'Import'}
                  </button>
                </div>
              )}
            </div>
          )}

          <div>
            <label htmlFor="ai-provider-select" className="text-xs text-text-muted block mb-1.5">{t('settings.ai.provider')}</label>
            <select
              id="ai-provider-select"
              value={settingsForm.provider}
              onChange={(e) => {
                const newProvider = e.target.value;
                const registryModels = getProviderModels(newProvider, modelRegistry);
                const defaultModel = newProvider === 'local'
                  ? 'all-MiniLM-L6-v2'
                  : newProvider === 'openai-compatible'
                    ? ''
                    : newProvider === 'ollama' && ollamaModels.length > 0
                      ? ollamaModels[0] ?? ''
                      : registryModels[0] ?? '';
                setSettingsForm((f) => ({
                  ...f,
                  provider: newProvider,
                  model: defaultModel,
                  baseUrl: newProvider === 'ollama'
                    ? 'http://localhost:11434'
                    : newProvider === 'openai-compatible'
                      ? ''
                      : '',
                }));
                setValidation({ status: 'idle', message: '', models: [] });
              }}
              className="w-full px-4 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
            >
              <option value="local">{t('settings.ai.builtInLocal')}</option>
              <option value="anthropic">{t('settings.ai.providerAnthropic')}</option>
              <option value="openai">{t('settings.ai.providerOpenAI')}</option>
              <option value="openai-compatible">{t('settings.ai.providerOpenAICompatible')}</option>
              <option value="ollama">{t('settings.ai.providerOllama')}</option>
            </select>
          </div>

          {settingsForm.provider === 'local' && (
            <div className="bg-bg-secondary rounded-lg p-3 border border-green-500/20">
              <p className="text-xs text-green-400 font-medium mb-1">{t('settings.ai.builtInModel')}</p>
              <p className="text-xs text-text-muted">
                {t('settings.ai.builtInDescription')}
              </p>
            </div>
          )}

          {settingsForm.provider !== 'ollama' && settingsForm.provider !== 'local' && (
            <div>
              <label htmlFor="ai-api-key" className="text-xs text-text-muted block mb-1.5">{t('settings.ai.apiKey')}</label>
              <input
                id="ai-api-key"
                type="password"
                value={settingsForm.apiKey}
                onChange={(e) => {
                  const val = e.target.value;
                  setSettingsForm((f) => ({ ...f, apiKey: val }));
                  validateKey(
                    settingsForm.provider,
                    val,
                    settingsForm.provider === 'openai-compatible' ? settingsForm.baseUrl : undefined,
                  );
                }}
                placeholder={settings?.llm.has_api_key ? t('settings.ai.keySaved') : t('settings.ai.enterKey')}
                className="w-full px-4 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-orange-500 focus:outline-none font-mono"
              />
              {/* Real-time validation feedback */}
              {validation.status === 'checking' && (
                <div className="flex items-center gap-2 mt-1.5 text-xs text-text-muted">
                  <div className="w-3 h-3 border border-orange-500 border-t-transparent rounded-full animate-spin" />
                  {validation.message}
                </div>
              )}
              {validation.status === 'valid' && (
                <p className="mt-1.5 text-xs text-green-400">&#x2713; {validation.message}</p>
              )}
              {validation.status === 'format_error' && (
                <p className="mt-1.5 text-xs text-red-400">{validation.message}</p>
              )}
              {validation.status === 'invalid' && (
                <p className="mt-1.5 text-xs text-amber-400">{validation.message}</p>
              )}
            </div>
          )}

          {settingsForm.provider !== 'local' && settingsForm.provider !== 'openai-compatible' && (
            <div>
              <label htmlFor="ai-model-select" className="text-xs text-text-muted block mb-1.5">{t('settings.ai.model')}</label>
              <select
                id="ai-model-select"
                value={settingsForm.model}
                onChange={(e) => setSettingsForm((f) => ({ ...f, model: e.target.value }))}
                className="w-full px-4 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none"
              >
                {(settingsForm.provider === 'ollama' && ollamaModels.length > 0
                  ? ollamaModels
                  : getProviderModels(settingsForm.provider, modelRegistry)
                ).map((m) => (
                  <option key={m} value={m}>{m}</option>
                ))}
              </select>
              {settingsForm.provider === 'ollama' && (
                <div className="flex items-center gap-2 mt-2">
                  <p className="text-xs text-text-muted">
                    {ollamaStatus?.running
                      ? <span className="text-green-400">&#x2713; {t('settings.ai.ollamaRunning', { version: ollamaStatus.version, count: ollamaModels.length })}</span>
                      : <span className="text-yellow-400">&#x25cb; {t('settings.ai.ollamaNotDetected')}</span>}
                  </p>
                  <button
                    onClick={() => checkOllamaStatus(settingsForm.baseUrl || undefined)}
                    className="text-[10px] px-2 py-0.5 text-text-muted hover:text-orange-400 bg-bg-tertiary rounded transition-colors"
                  >
                    {t('settings.ai.recheck')}
                  </button>
                </div>
              )}
              {/* Registry freshness + refresh */}
              {settingsForm.provider !== 'ollama' && modelRegistry && (
                <div className="flex items-center gap-2 mt-2">
                  <p className="text-[10px] text-text-muted">
                    {t('settings.ai.registryLastUpdated', { time: formatFreshness(modelRegistry.fetched_at) })}
                  </p>
                  {onRefreshRegistry && (
                    <button
                      onClick={onRefreshRegistry}
                      className="text-[10px] px-2 py-0.5 text-text-muted hover:text-orange-400 bg-bg-tertiary rounded transition-colors"
                    >
                      {t('settings.ai.refreshModels')}
                    </button>
                  )}
                </div>
              )}
            </div>
          )}

          {settingsForm.provider === 'openai-compatible' && (
            <div>
              <label htmlFor="ai-model-name" className="text-xs text-text-muted block mb-1.5">{t('settings.ai.modelName')}</label>
              <input
                id="ai-model-name"
                type="text"
                value={settingsForm.model}
                onChange={(e) => setSettingsForm((f) => ({ ...f, model: e.target.value }))}
                placeholder={t('settings.ai.modelNamePlaceholder')}
                className="w-full px-4 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-orange-500 focus:outline-none font-mono"
              />
            </div>
          )}

          {settingsForm.provider === 'ollama' && (
            <div>
              <label htmlFor="ai-base-url" className="text-xs text-text-muted block mb-1.5">{t('settings.ai.baseUrl')}</label>
              <input
                id="ai-base-url"
                type="text"
                value={settingsForm.baseUrl}
                onChange={(e) => setSettingsForm((f) => ({ ...f, baseUrl: e.target.value }))}
                placeholder="http://localhost:11434"
                className="w-full px-4 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-orange-500 focus:outline-none font-mono"
              />
            </div>
          )}

          {settingsForm.provider === 'openai-compatible' && (
            <div>
              <label htmlFor="ai-base-url" className="text-xs text-text-muted block mb-1.5">{t('settings.ai.baseUrl')}</label>
              <input
                id="ai-base-url"
                type="text"
                value={settingsForm.baseUrl}
                onChange={(e) => setSettingsForm((f) => ({ ...f, baseUrl: e.target.value }))}
                placeholder="https://api.groq.com/openai/v1"
                className="w-full px-4 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-orange-500 focus:outline-none font-mono"
              />
              <div className="mt-2">
                <p className="text-[10px] text-text-muted mb-1.5">{t('settings.ai.popularEndpoints')}</p>
                <div className="flex flex-wrap gap-1.5">
                  {popularEndpoints.map((ep) => (
                    <button
                      key={ep.name}
                      type="button"
                      onClick={() => setSettingsForm((f) => ({ ...f, baseUrl: ep.url }))}
                      className="text-[10px] px-2 py-0.5 text-text-secondary hover:text-orange-400 bg-bg-secondary border border-border rounded hover:border-orange-500/30 transition-colors"
                    >
                      {ep.name}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      <ReRankingSection settingsForm={settingsForm} setSettingsForm={setSettingsForm} />

      {settings && (
        <UsageStatsSection settings={settings} provider={settingsForm.provider} />
      )}
    </>
  );
}
