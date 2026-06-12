// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/* eslint-disable i18next/no-literal-string -- brand names (Anthropic, OpenAI, Ollama) are intentional */
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import type { OllamaStatus, PullProgress } from './types';

type ProviderType = 'anthropic' | 'openai' | 'ollama' | 'openai-compatible';

interface EnvDetection {
  has_anthropic_env: boolean;
  anthropic_env_preview: string;
  has_openai_env: boolean;
  openai_env_preview: string;
  ollama_running: boolean;
  ollama_url: string | null;
}

interface LocalServer {
  name: string;
  base_url: string;
  model_count: number;
  running: boolean;
}

interface SetupAIProviderProps {
  ollamaStatus: OllamaStatus | null;
  provider: ProviderType;
  apiKey: string;
  pullingModels: boolean;
  pullProgress: Record<string, PullProgress>;
  onProviderChange: (provider: ProviderType) => void;
  onApiKeyChange: (key: string) => void;
  onDownloadModels?: () => void;
}

export function SetupAIProvider({
  ollamaStatus,
  provider,
  apiKey,
  pullingModels,
  pullProgress,
  onProviderChange,
  onApiKeyChange,
  onDownloadModels,
}: SetupAIProviderProps) {
  const { t } = useTranslation();
  const [envDetection, setEnvDetection] = useState<EnvDetection | null>(null);
  const [importing, setImporting] = useState(false);
  const [localServers, setLocalServers] = useState<LocalServer[]>([]);

  useEffect(() => {
    cmd('detect_environment').then(setEnvDetection).catch((e) => {
      console.warn('[4DA] Environment detection failed:', e);
    });
    cmd('detect_local_servers').then((r) => setLocalServers(r.servers)).catch((e) => {
      console.warn('[4DA] Local server detection failed:', e);
    });
  }, []);

  const handleImportEnvKey = async (envProvider: 'anthropic' | 'openai') => {
    setImporting(true);
    try {
      await cmd('import_env_key', { provider: envProvider });
      onProviderChange(envProvider);
      onApiKeyChange('(imported from environment)');
    } catch {
      // Silently fail — user can still enter manually
    } finally {
      setImporting(false);
    }
  };

  const ollamaReady = ollamaStatus?.running && ollamaStatus.has_embedding_model && ollamaStatus.has_llm_model;

  return (
    <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border space-y-3">
      {/* Built-in private semantic search reassurance — calm, prominent, top of step */}
      <div className="flex items-start gap-3 p-3 bg-bg-secondary border border-border rounded-lg">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="w-4 h-4 mt-0.5 shrink-0 text-text-secondary"
          aria-hidden="true"
        >
          <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
          <path d="M7 11V7a5 5 0 0 1 10 0v4" />
        </svg>
        <div>
          <p className="text-sm font-medium text-text-primary">{t('onboarding.setupAi.builtInSemanticTitle')}</p>
          <p className="text-xs text-text-secondary mt-0.5">{t('onboarding.setupAi.builtInSemanticBody')}</p>
        </div>
      </div>

      {/* Environment key detection banner */}
      {envDetection && (envDetection.has_anthropic_env || envDetection.has_openai_env) && (
        <div className="p-3 bg-blue-900/20 border border-blue-500/30 rounded-lg space-y-2">
          <p className="text-sm text-blue-300 font-medium">{t('onboarding.setupAi.envKeysDetected')}</p>
          {envDetection.has_anthropic_env && (
            <div className="flex items-center justify-between">
              <span className="text-xs text-text-secondary font-mono">
                Anthropic: {envDetection.anthropic_env_preview}
              </span>
              <button
                onClick={() => { void handleImportEnvKey('anthropic'); }}
                disabled={importing}
                className="text-xs px-3 py-1 bg-blue-500/20 text-blue-300 rounded hover:bg-blue-500/30 transition-colors disabled:opacity-50"
              >
                {importing ? t('action.importing') : t('onboarding.setupAi.useThisKey')}
              </button>
            </div>
          )}
          {envDetection.has_openai_env && (
            <div className="flex items-center justify-between">
              <span className="text-xs text-text-secondary font-mono">
                OpenAI: {envDetection.openai_env_preview}
              </span>
              <button
                onClick={() => { void handleImportEnvKey('openai'); }}
                disabled={importing}
                className="text-xs px-3 py-1 bg-blue-500/20 text-blue-300 rounded hover:bg-blue-500/30 transition-colors disabled:opacity-50"
              >
                {importing ? t('action.importing') : t('onboarding.setupAi.useThisKey')}
              </button>
            </div>
          )}
        </div>
      )}

      {/* Ollama fully ready */}
      {ollamaReady && provider === 'ollama' && (
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
          {Object.entries(pullProgress).map(([model, p]) => {
            const isCancelled = p.status === 'cancelled';
            return (
              <div key={model} className="space-y-1">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-text-primary font-mono font-medium">{model}</span>
                  <span className={isCancelled ? 'text-red-400' : 'text-text-muted'}>
                    {isCancelled
                      ? t('action.cancelled')
                      : p.done
                        ? t('onboarding.apiKeys.pullComplete')
                        : p.status || `${p.percent}%`}
                  </span>
                </div>
                <div className="w-full h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                  <div
                    className={`h-full rounded-full transition-all duration-300 ${
                      isCancelled ? 'bg-red-500' : p.done ? 'bg-green-500' : 'bg-orange-500'
                    }`}
                    style={{ width: `${isCancelled ? 100 : p.done ? 100 : p.percent}%` }}
                  />
                </div>
              </div>
            );
          })}
          <div className="flex items-center justify-between">
            <p className="text-xs text-text-muted">
              {t('onboarding.apiKeys.pullWaitMessage')}
            </p>
            <button
              onClick={() => { void cmd('cancel_ollama_pull'); }}
              className="px-3 py-1.5 text-xs text-red-400 border border-red-500/30 rounded-lg hover:bg-red-500/10 transition-colors"
            >
              {t('action.cancelDownload')}
            </button>
          </div>
        </div>
      )}

      {/* Provider selector */}
      {!ollamaReady && !pullingModels && (
        <>
          {/* CLOUD section — recommended for best results */}
          <div>
            <div className="flex items-center gap-2 mb-2">
              <span className="text-xs font-medium text-text-secondary uppercase tracking-wider">{t('onboarding.setupAi.cloudLabel')}</span>
              <span className="text-[9px] px-1.5 py-0.5 bg-green-500/20 text-green-400 rounded font-medium">{t('onboarding.setupAi.bestResults')}</span>
            </div>
            <p className="text-[10px] text-text-muted mb-2">{t('onboarding.setupAi.byokExplainer')}</p>
            <div className="grid grid-cols-3 gap-2">
              {(['anthropic', 'openai', 'openai-compatible'] as const).map((p) => (
                <button
                  key={p}
                  onClick={() => onProviderChange(p)}
                  className={`p-3 rounded-lg text-center transition-all ${
                    provider === p
                      ? 'bg-green-500/15 border-2 border-green-500/50'
                      : 'bg-bg-tertiary border-2 border-transparent hover:border-border'
                  }`}
                >
                  <div className="text-sm font-medium text-white">
                    {p === 'anthropic' ? 'Anthropic' : p === 'openai' ? 'OpenAI' : t('onboarding.setupAi.otherLabel')}
                  </div>
                  <div className="text-[10px] text-text-muted mt-0.5">
                    {p === 'anthropic' ? 'Claude' : p === 'openai' ? 'GPT' : t('onboarding.setupAi.otherDesc')}
                  </div>
                </button>
              ))}
            </div>
          </div>

          {/* LOCAL section — privacy-first alternative */}
          <div>
            <div className="flex items-center gap-2 mb-2 mt-1">
              <span className="text-xs font-medium text-text-secondary uppercase tracking-wider">{t('onboarding.setupAi.localLabel')}</span>
              <span className="text-[9px] px-1.5 py-0.5 bg-bg-tertiary text-text-muted rounded font-medium">{t('onboarding.setupAi.zeroCloud')}</span>
            </div>
            <p className="text-[10px] text-text-muted mb-2">{t('onboarding.setupAi.localTradeoff')}</p>
            <div className="grid grid-cols-2 gap-2">
              <button
                onClick={() => onProviderChange('ollama')}
                className={`p-3 rounded-lg text-start transition-all ${
                  provider === 'ollama'
                    ? 'bg-orange-500/20 border-2 border-orange-500'
                    : 'bg-bg-tertiary border-2 border-transparent hover:border-border'
                }`}
              >
                <div className="text-sm font-medium text-white">Ollama</div>
                <div className="text-[10px] text-text-muted mt-0.5">{t('onboarding.setupAi.ollamaDesc')}</div>
              </button>
              {/* Auto-detected local servers */}
              {localServers.filter(s => s.name !== 'Ollama').map((server) => (
                <button
                  key={server.name}
                  onClick={() => {
                    onProviderChange('openai-compatible');
                    onApiKeyChange('');
                  }}
                  className="p-3 rounded-lg text-start bg-bg-tertiary border-2 border-transparent hover:border-border transition-all"
                >
                  <div className="text-sm font-medium text-white flex items-center gap-1.5">
                    {server.name}
                    <span className="w-1.5 h-1.5 rounded-full bg-green-500" />
                  </div>
                  <div className="text-[10px] text-text-muted mt-0.5">
                    {t('onboarding.setupAi.modelCount', { count: server.model_count })} &middot; {server.base_url}
                  </div>
                </button>
              ))}
            </div>
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

          {/* OpenAI-compatible provider input */}
          {provider === 'openai-compatible' && (
            <div className="space-y-2">
              <label className="text-xs text-text-muted">{t('onboarding.setupAi.otherProviderHint')}</label>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => onApiKeyChange(e.target.value)}
                placeholder={t('settings.llm.apiKey')}
                className="w-full px-4 py-3 bg-bg-tertiary border border-border rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
              />
              <p className="text-[10px] text-text-muted">{t('onboarding.setupAi.configureInSettings')}</p>
            </div>
          )}

          {/* Cloud data disclosure — informed consent at the BYOK moment */}
          {(provider === 'anthropic' || provider === 'openai' || provider === 'openai-compatible') && (
            <p className="text-[10px] text-text-muted leading-relaxed border-s-2 border-border/60 ps-2">
              {t(
                'onboarding.setupAi.cloudDataDisclosure',
                'What gets sent: when you use a cloud model, the titles and short snippets 4DA analyzes (capped ~2000 characters) go to your chosen provider under their terms. Your indexed data, files, and API key stay on your machine — 4DA never sees or stores them.',
              )}
            </p>
          )}

          {/* Ollama not running hint */}
          {provider === 'ollama' && !ollamaStatus?.running && (
            <div className="text-text-secondary text-sm p-3 bg-bg-tertiary rounded-lg border border-border">
              <p className="mb-1.5">
                {t('onboarding.setupAi.ollamaNotDetected')}{' '}
                <a href="https://ollama.com" target="_blank" rel="noopener noreferrer" className="text-orange-500 hover:underline">
                  {t('onboarding.apiKeys.installOllama')}
                </a>
                {' '}{t('onboarding.setupAi.orChooseCloud')}
              </p>
              <p className="text-xs text-text-muted mb-1">
                {t('onboarding.setupAi.ollamaExplainer')}
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

          {/* Ollama running but missing models — explicit, consented download
              (never auto-pulled; downloading GBs unprompted is a false-state surprise) */}
          {provider === 'ollama' && ollamaStatus?.running && !ollamaReady && (
            <div className="text-text-secondary text-sm p-3 bg-bg-tertiary rounded-lg border border-border space-y-2">
              <p className="text-xs text-text-muted">{t('onboarding.setupAi.modelsNeeded')}</p>
              <button
                onClick={() => onDownloadModels?.()}
                className="px-3 py-1.5 text-xs font-medium bg-orange-500/15 text-orange-300 border border-orange-500/30 rounded-lg hover:bg-orange-500/25 transition-colors"
              >
                {t('onboarding.setupAi.downloadModels')}
              </button>
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
