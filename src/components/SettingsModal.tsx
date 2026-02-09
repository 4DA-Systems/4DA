import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { LearnedBehaviorPanel } from './LearnedBehaviorPanel';
import { SystemHealthPanel } from './SystemHealthPanel';
import { IndexedDocumentsPanel } from './IndexedDocumentsPanel';
import { NaturalLanguageSearch } from './NaturalLanguageSearch';
import { SourceConfigPanel } from './SourceConfigPanel';
import type { Settings, MonitoringStatus, UserContext, SystemHealth } from '../types';
import type { OllamaStatus } from '../hooks/use-settings';

// Provider model options
const providerModels: Record<string, string[]> = {
  anthropic: ['claude-3-haiku-20240307', 'claude-3-sonnet-20240229', 'claude-3-opus-20240229'],
  openai: ['gpt-4o-mini', 'gpt-4o', 'gpt-4-turbo', 'gpt-3.5-turbo'],
  ollama: ['llama3', 'mistral', 'mixtral', 'phi3'],
};

// Types for feedback data (matches LearnedBehaviorPanel props)
interface TopicAffinity {
  topic: string;
  positive_signals: number;
  negative_signals: number;
  affinity_score: number;
}

interface AntiTopic {
  topic: string;
  rejection_count: number;
  confidence: number;
  auto_detected: boolean;
}

interface SimilarTopicResult {
  topic: string;
  similarity: number;
}

// ============================================================================
// Props
// ============================================================================

interface SettingsModalProps {
  onClose: () => void;

  // Settings hook
  settings: Settings | null;
  settingsForm: {
    provider: string;
    apiKey: string;
    model: string;
    baseUrl: string;
    rerankEnabled: boolean;
    maxItems: number;
    minScore: number;
    dailyTokenLimit: number;
    dailyCostLimit: number;
  };
  setSettingsForm: React.Dispatch<React.SetStateAction<SettingsModalProps['settingsForm']>>;
  settingsStatus: string;
  setSettingsStatus: (status: string) => void;
  saveSettings: () => void;
  testConnection: () => void;
  ollamaStatus: OllamaStatus | null;
  ollamaModels: string[];
  checkOllamaStatus: (baseUrl?: string) => void;

  // Monitoring hook
  monitoring: MonitoringStatus | null;
  monitoringInterval: number;
  setMonitoringInterval: (val: number) => void;
  toggleMonitoring: () => Promise<string>;
  updateMonitoringInterval: () => Promise<string>;
  testNotification: () => Promise<string>;

  // Context discovery hook
  scanDirectories: string[];
  newScanDir: string;
  setNewScanDir: (val: string) => void;
  isScanning: boolean;
  discoveredContext: {
    tech: { name: string; category: string; confidence: number }[];
    topics: string[];
    lastScan: string | null;
  };
  runAutoDiscovery: () => void;
  runFullScan: () => void;
  addScanDirectory: () => void;
  removeScanDirectory: (dir: string) => void;

  // Feedback hook
  learnedAffinities: TopicAffinity[];
  antiTopics: AntiTopic[];
  loadLearnedBehavior: () => void;

  // System health hook
  systemHealth: SystemHealth | null;
  similarTopicQuery: string;
  setSimilarTopicQuery: (q: string) => void;
  similarTopicResults: SimilarTopicResult[];
  runAnomalyDetection: () => void;
  resolveAnomaly: (anomalyId: number) => void;
  findSimilarTopics: () => void;
  saveWatcherState: () => void;
  loadSystemHealth: () => void;

  // User context hook
  userContext: UserContext | null;
  newInterest: string;
  setNewInterest: (val: string) => void;
  newExclusion: string;
  setNewExclusion: (val: string) => void;
  newTechStack: string;
  setNewTechStack: (val: string) => void;
  newRole: string;
  setNewRole: (val: string) => void;
  addInterest: () => void;
  removeInterest: (topic: string) => void;
  addExclusion: () => void;
  removeExclusion: (exclusion: string) => void;
  addTechStack: () => void;
  removeTechStack: (tech: string) => void;
  updateRole: () => void;
}

// ============================================================================
// SettingsModal Component
// ============================================================================

export function SettingsModal({
  onClose,
  settings,
  settingsForm,
  setSettingsForm,
  settingsStatus,
  setSettingsStatus,
  saveSettings,
  testConnection,
  ollamaStatus,
  ollamaModels,
  checkOllamaStatus,
  monitoring,
  monitoringInterval,
  setMonitoringInterval,
  toggleMonitoring,
  updateMonitoringInterval,
  testNotification,
  scanDirectories,
  newScanDir,
  setNewScanDir,
  isScanning,
  discoveredContext,
  runAutoDiscovery,
  runFullScan,
  addScanDirectory,
  removeScanDirectory,
  learnedAffinities,
  antiTopics,
  loadLearnedBehavior,
  systemHealth,
  similarTopicQuery,
  setSimilarTopicQuery,
  similarTopicResults,
  runAnomalyDetection,
  resolveAnomaly,
  findSimilarTopics,
  saveWatcherState,
  loadSystemHealth,
  userContext,
  newInterest,
  setNewInterest,
  newExclusion,
  setNewExclusion,
  newTechStack,
  setNewTechStack,
  newRole,
  setNewRole,
  addInterest,
  removeInterest,
  addExclusion,
  removeExclusion,
  addTechStack,
  removeTechStack,
  updateRole,
}: SettingsModalProps) {
  // Digest config - local to settings modal
  const [digestConfig, setDigestConfig] = useState<{
    enabled: boolean;
    save_local: boolean;
    frequency: string;
    min_score: number;
    max_items: number;
  } | null>(null);

  useEffect(() => {
    loadDigestConfig();
  }, []);

  const loadDigestConfig = async () => {
    try {
      const config = await invoke<{
        enabled: boolean;
        save_local: boolean;
        frequency: string;
        min_score: number;
        max_items: number;
      }>('get_digest_config');
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

  // Monitoring action wrappers (add status messages)
  const handleToggleMonitoring = async () => {
    try {
      const msg = await toggleMonitoring();
      setSettingsStatus(msg);
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  };

  const handleUpdateMonitoringInterval = async () => {
    try {
      const msg = await updateMonitoringInterval();
      setSettingsStatus(msg);
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  };

  const handleTestNotification = async () => {
    try {
      const msg = await testNotification();
      setSettingsStatus(msg);
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Notification error: ${error}`);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-50 p-4" role="dialog" aria-modal="true" aria-labelledby="settings-modal-title">
      <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl w-full max-w-lg max-h-[90vh] overflow-y-auto shadow-2xl">
        {/* Modal Header */}
        <div className="px-6 py-4 border-b border-[#2A2A2A] flex items-center justify-between sticky top-0 bg-[#141414] z-10">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
              <span>⚙️</span>
            </div>
            <h2 id="settings-modal-title" className="text-lg font-medium text-white">Settings</h2>
          </div>
          <button
            onClick={onClose}
            aria-label="Close settings"
            className="w-8 h-8 rounded-lg bg-[#1F1F1F] text-gray-500 hover:text-white hover:bg-[#2A2A2A] flex items-center justify-center transition-all"
          >
            ×
          </button>
        </div>

        <div className="p-6 space-y-6">
          {/* LLM Provider Section */}
          <div className="bg-[#1F1F1F] rounded-lg p-5 border border-[#2A2A2A]">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
                <span>🤖</span>
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
                        ? <span className="text-green-400">✓ Ollama v{ollamaStatus.version} - {ollamaModels.length} models</span>
                        : <span className="text-yellow-400">○ Ollama not detected</span>}
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
                <span>⚡</span>
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
                  <label className="text-xs text-gray-500 block mb-2">Cost limit (¢/day)</label>
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
                  <span>📈</span>
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

          {/* Continuous Monitoring */}
          <div className="bg-[#1F1F1F] rounded-lg p-5 border border-[#2A2A2A]">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center">
                <span>🔄</span>
              </div>
              <div>
                <h3 className="text-sm font-medium text-white">Background Monitoring</h3>
                <p className="text-xs text-gray-500">Auto-analyze at intervals</p>
              </div>
            </div>

            {monitoring ? (
              <div className="space-y-4">
                <div className="flex items-center justify-between p-3 bg-[#141414] rounded-lg border border-[#2A2A2A]">
                  <div className="flex items-center gap-2">
                    {monitoring.enabled ? (
                      <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
                    ) : (
                      <div className="w-2 h-2 bg-gray-600 rounded-full" />
                    )}
                    <span className="text-sm text-white">
                      {monitoring.enabled ? (
                        <span className="text-green-400">Active</span>
                      ) : (
                        <span className="text-gray-500">Inactive</span>
                      )}
                    </span>
                    {monitoring.is_checking && (
                      <span className="text-xs text-orange-400 ml-2">(checking...)</span>
                    )}
                  </div>
                  <button
                    onClick={handleToggleMonitoring}
                    className={`px-4 py-2 text-sm rounded-lg transition-all ${
                      monitoring.enabled
                        ? 'bg-red-500/20 text-red-400 hover:bg-red-500/30'
                        : 'bg-green-500/20 text-green-400 hover:bg-green-500/30'
                    }`}
                  >
                    {monitoring.enabled ? 'Stop' : 'Start'}
                  </button>
                </div>

                <div className="flex items-center gap-3">
                  <label className="text-sm text-gray-400">Every</label>
                  <input
                    type="number"
                    min="5"
                    max="1440"
                    value={monitoringInterval}
                    onChange={(e) => setMonitoringInterval(parseInt(e.target.value) || 30)}
                    className="w-20 px-3 py-2 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white text-center focus:border-orange-500 focus:outline-none"
                  />
                  <span className="text-sm text-gray-400">minutes</span>
                  <button
                    onClick={handleUpdateMonitoringInterval}
                    className="px-4 py-2 text-sm bg-[#141414] border border-[#2A2A2A] text-gray-400 rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
                  >
                    Update
                  </button>
                </div>

                <div className="flex items-center justify-between text-xs text-gray-500 px-1">
                  <span>Total checks: {monitoring.total_checks}</span>
                  {monitoring.last_check_ago && (
                    <span>Last: {monitoring.last_check_ago}</span>
                  )}
                </div>

                <button
                  onClick={handleTestNotification}
                  className="w-full px-4 py-2.5 text-sm bg-[#141414] border border-[#2A2A2A] text-gray-400 rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
                >
                  Test Notification
                </button>
              </div>
            ) : (
              <div className="text-xs text-text-muted">Loading monitoring status...</div>
            )}
          </div>

          {/* Digest Settings */}
          <div className="bg-[#1F1F1F] rounded-lg p-5 border border-[#2A2A2A]">
            <div className="flex items-start gap-3 mb-4">
              <div className="w-8 h-8 bg-purple-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
                <span className="text-purple-400">📋</span>
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

          {/* Source Configuration */}
          <SourceConfigPanel onStatusChange={setSettingsStatus} />

          {/* Automatic Context Discovery */}
          <div className="bg-[#1F1F1F] rounded-lg p-5 border border-[#2A2A2A]">
            <div className="flex items-start gap-3 mb-4">
              <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
                <span className="text-orange-400">🔍</span>
              </div>
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <h3 className="text-white font-medium">Automatic Context Discovery</h3>
                  <span className="px-2 py-0.5 text-[10px] bg-orange-500/20 text-orange-400 rounded-full font-medium">ACE</span>
                </div>
                <p className="text-gray-500 text-sm mt-1">
                  Scan directories that define who you are - projects, notes, documents
                </p>
              </div>
            </div>

            <div className="space-y-4">
              <button
                onClick={runAutoDiscovery}
                disabled={isScanning}
                className="w-full px-4 py-3 text-sm bg-gradient-to-r from-orange-500/20 to-orange-600/10 text-orange-400 border border-orange-500/30 rounded-lg hover:from-orange-500/30 hover:to-orange-600/20 transition-all disabled:opacity-50 disabled:cursor-not-allowed font-medium"
              >
                {isScanning ? 'Discovering...' : '✨ Auto-Discover My Context'}
              </button>

              <div className="flex gap-2">
                <input
                  type="text"
                  value={newScanDir}
                  onChange={(e) => setNewScanDir(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && addScanDirectory()}
                  placeholder="Or add specific directory: ~/notes, D:\research"
                  className="flex-1 px-3 py-2.5 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-500 focus:border-orange-500/50 focus:outline-none transition-colors"
                />
                <button
                  onClick={addScanDirectory}
                  className="px-4 py-2.5 text-sm bg-[#141414] border border-[#2A2A2A] rounded-lg text-gray-400 hover:text-white hover:border-orange-500/30 transition-all"
                >
                  Add
                </button>
              </div>

              <div className="space-y-2">
                {scanDirectories.length === 0 ? (
                  <p className="text-sm text-gray-500 text-center py-3">No directories configured yet</p>
                ) : (
                  <>
                    <div className="text-xs text-gray-500 mb-2">Configured Directories ({scanDirectories.length})</div>
                    <div className="space-y-1.5 max-h-32 overflow-y-auto">
                      {scanDirectories.map((dir) => (
                        <div key={dir} className="flex items-center justify-between px-3 py-2 bg-[#141414] rounded-lg border border-[#2A2A2A] group">
                          <span className="font-mono text-sm text-white truncate">{dir}</span>
                          <button
                            onClick={() => removeScanDirectory(dir)}
                            className="text-gray-500 hover:text-red-400 ml-2 opacity-0 group-hover:opacity-100 transition-opacity"
                          >
                            ×
                          </button>
                        </div>
                      ))}
                    </div>
                  </>
                )}
              </div>

              {scanDirectories.length > 0 && (
                <button
                  onClick={runFullScan}
                  disabled={isScanning}
                  className="w-full px-4 py-2.5 text-sm bg-[#141414] border border-[#2A2A2A] rounded-lg text-gray-400 hover:text-white hover:border-orange-500/30 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {isScanning ? 'Scanning...' : 'Re-scan Configured Directories'}
                </button>
              )}

              {(discoveredContext.tech.length > 0 || discoveredContext.topics.length > 0) && (
                <div className="bg-[#141414] rounded-lg p-4 border border-[#2A2A2A] space-y-3">
                  <div className="text-xs text-gray-500 flex items-center gap-2">
                    <span className="w-1.5 h-1.5 bg-green-500 rounded-full animate-pulse" />
                    Discovered Context {discoveredContext.lastScan && `(${new Date(discoveredContext.lastScan).toLocaleDateString()})`}
                  </div>
                  {discoveredContext.tech.length > 0 && (
                    <div>
                      <div className="text-xs text-gray-400 mb-2">Tech Stack</div>
                      <div className="flex flex-wrap gap-1.5">
                        {discoveredContext.tech.slice(0, 10).map((tech) => (
                          <span
                            key={tech.name}
                            className="px-2 py-1 text-xs bg-green-500/10 text-green-400 rounded-md border border-green-500/20"
                            title={`${tech.category} - ${Math.round(tech.confidence * 100)}% confidence`}
                          >
                            {tech.name}
                          </span>
                        ))}
                        {discoveredContext.tech.length > 10 && (
                          <span className="text-xs text-gray-500 self-center">+{discoveredContext.tech.length - 10} more</span>
                        )}
                      </div>
                    </div>
                  )}
                  {discoveredContext.topics.length > 0 && (
                    <div>
                      <div className="text-xs text-gray-400 mb-2">Topics</div>
                      <div className="flex flex-wrap gap-1.5">
                        {discoveredContext.topics.slice(0, 8).map((topic) => (
                          <span
                            key={topic}
                            className="px-2 py-1 text-xs bg-orange-500/10 text-orange-400 rounded-md border border-orange-500/20"
                          >
                            {topic}
                          </span>
                        ))}
                        {discoveredContext.topics.length > 8 && (
                          <span className="text-xs text-gray-500 self-center">+{discoveredContext.topics.length - 8} more</span>
                        )}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>

          {/* Manual Context / Personalization */}
          <div className="bg-[#1F1F1F] rounded-lg p-5 border border-[#2A2A2A]">
            <div className="flex items-start gap-3 mb-4">
              <div className="w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
                <span className="text-blue-400">🎯</span>
              </div>
              <div>
                <h3 className="text-white font-medium">Manual Adjustments</h3>
                <p className="text-gray-500 text-sm mt-1">
                  Fine-tune your context with interests and exclusions
                </p>
              </div>
            </div>

            {userContext ? (
              <div className="space-y-5">
                {/* Role */}
                <div>
                  <label className="text-xs text-gray-400 block mb-2">Your Role</label>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={newRole}
                      onChange={(e) => setNewRole(e.target.value)}
                      placeholder="e.g. Backend Developer"
                      className="flex-1 px-3 py-2.5 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-500 focus:border-blue-500/50 focus:outline-none transition-colors"
                    />
                    <button
                      onClick={updateRole}
                      className="px-4 py-2.5 text-sm bg-[#141414] border border-[#2A2A2A] rounded-lg text-gray-400 hover:text-white hover:border-blue-500/30 transition-all"
                    >
                      Set
                    </button>
                  </div>
                </div>

                {/* Tech Stack */}
                <div>
                  <label className="text-xs text-gray-400 block mb-2">Tech Stack</label>
                  <div className="flex gap-2 mb-3">
                    <input
                      type="text"
                      value={newTechStack}
                      onChange={(e) => setNewTechStack(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && addTechStack()}
                      placeholder="e.g. Rust, TypeScript"
                      className="flex-1 px-3 py-2.5 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-500 focus:border-blue-500/50 focus:outline-none transition-colors"
                    />
                    <button
                      onClick={addTechStack}
                      className="px-4 py-2.5 text-sm bg-[#141414] border border-[#2A2A2A] rounded-lg text-gray-400 hover:text-white hover:border-blue-500/30 transition-all"
                    >
                      Add
                    </button>
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {userContext.tech_stack.map((tech) => (
                      <span
                        key={tech}
                        className="inline-flex items-center gap-1.5 px-2.5 py-1 bg-orange-500/10 text-orange-400 text-xs rounded-md border border-orange-500/20 group"
                      >
                        {tech}
                        <button
                          onClick={() => removeTechStack(tech)}
                          className="text-orange-400/50 hover:text-red-400 transition-colors"
                        >
                          ×
                        </button>
                      </span>
                    ))}
                    {userContext.tech_stack.length === 0 && (
                      <span className="text-sm text-gray-500">No technologies added</span>
                    )}
                  </div>
                </div>

                {/* Interests */}
                <div>
                  <div className="flex items-center gap-2 mb-2">
                    <label className="text-xs text-gray-400">Interests</label>
                    <span className="px-1.5 py-0.5 text-[10px] bg-green-500/20 text-green-400 rounded">{userContext.interests.length}</span>
                  </div>
                  <div className="flex gap-2 mb-3">
                    <input
                      type="text"
                      value={newInterest}
                      onChange={(e) => setNewInterest(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && addInterest()}
                      placeholder="e.g. machine learning, distributed systems"
                      className="flex-1 px-3 py-2.5 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-500 focus:border-green-500/50 focus:outline-none transition-colors"
                    />
                    <button
                      onClick={addInterest}
                      className="px-4 py-2.5 text-sm bg-green-500/10 text-green-400 border border-green-500/30 rounded-lg hover:bg-green-500/20 transition-all"
                    >
                      Add
                    </button>
                  </div>
                  <div className="flex flex-wrap gap-1.5 max-h-28 overflow-y-auto">
                    {userContext.interests.map((interest) => (
                      <span
                        key={interest.topic}
                        className="inline-flex items-center gap-1.5 px-2.5 py-1 bg-green-500/10 text-green-400 text-xs rounded-md border border-green-500/20 group"
                        title={interest.has_embedding ? 'Has embedding' : 'No embedding'}
                      >
                        {interest.has_embedding && <span className="w-1.5 h-1.5 bg-green-400 rounded-full" />}
                        {interest.topic}
                        <button
                          onClick={() => removeInterest(interest.topic)}
                          className="text-green-400/50 hover:text-red-400 transition-colors"
                        >
                          ×
                        </button>
                      </span>
                    ))}
                    {userContext.interests.length === 0 && (
                      <span className="text-sm text-gray-500">No interests added</span>
                    )}
                  </div>
                </div>

                {/* Exclusions */}
                <div>
                  <div className="flex items-center gap-2 mb-2">
                    <label className="text-xs text-gray-400">Exclusions</label>
                    <span className="px-1.5 py-0.5 text-[10px] bg-red-500/20 text-red-400 rounded">{userContext.exclusions.length}</span>
                  </div>
                  <div className="flex gap-2 mb-3">
                    <input
                      type="text"
                      value={newExclusion}
                      onChange={(e) => setNewExclusion(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && addExclusion()}
                      placeholder="e.g. crypto, sports"
                      className="flex-1 px-3 py-2.5 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-500 focus:border-red-500/50 focus:outline-none transition-colors"
                    />
                    <button
                      onClick={addExclusion}
                      className="px-4 py-2.5 text-sm bg-red-500/10 text-red-400 border border-red-500/30 rounded-lg hover:bg-red-500/20 transition-all"
                    >
                      Block
                    </button>
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {userContext.exclusions.map((exclusion) => (
                      <span
                        key={exclusion}
                        className="inline-flex items-center gap-1.5 px-2.5 py-1 bg-red-500/10 text-red-400 text-xs rounded-md border border-red-500/20 group"
                      >
                        {exclusion}
                        <button
                          onClick={() => removeExclusion(exclusion)}
                          className="text-red-400/50 hover:text-white transition-colors"
                        >
                          ×
                        </button>
                      </span>
                    ))}
                    {userContext.exclusions.length === 0 && (
                      <span className="text-sm text-gray-500">No exclusions set</span>
                    )}
                  </div>
                </div>
              </div>
            ) : (
              <div className="text-sm text-gray-500">Loading context...</div>
            )}
          </div>

          {/* Learned Behavior */}
          <LearnedBehaviorPanel
            affinities={learnedAffinities}
            antiTopics={antiTopics}
            onRefresh={loadLearnedBehavior}
          />

          {/* Natural Language Search */}
          <NaturalLanguageSearch
            onStatusChange={setSettingsStatus}
          />

          {/* Indexed Documents */}
          <IndexedDocumentsPanel
            onStatusChange={setSettingsStatus}
          />

          {/* System Health */}
          <SystemHealthPanel
            health={systemHealth}
            similarTopicQuery={similarTopicQuery}
            onSimilarTopicQueryChange={setSimilarTopicQuery}
            similarTopicResults={similarTopicResults}
            onRunAnomalyDetection={runAnomalyDetection}
            onResolveAnomaly={resolveAnomaly}
            onFindSimilarTopics={findSimilarTopics}
            onSaveWatcherState={saveWatcherState}
            onRefresh={loadSystemHealth}
          />

          {/* Status */}
          {settingsStatus && (
            <div className={`text-sm p-4 rounded-lg border ${settingsStatus.includes('Error') || settingsStatus.includes('failed') ? 'bg-red-500/10 text-red-400 border-red-500/30' : 'bg-green-500/10 text-green-400 border-green-500/30'}`}>
              {settingsStatus}
            </div>
          )}

          {/* Actions */}
          <div className="flex gap-3 pt-2">
            <button
              onClick={saveSettings}
              className="flex-1 px-4 py-3 text-sm bg-gradient-to-r from-orange-500 to-orange-600 text-white font-medium rounded-lg hover:from-orange-600 hover:to-orange-700 transition-all shadow-lg shadow-orange-500/20"
            >
              Save Settings
            </button>
            <button
              onClick={testConnection}
              className="px-6 py-3 text-sm bg-[#1F1F1F] text-gray-300 border border-[#2A2A2A] rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
            >
              Test Connection
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
