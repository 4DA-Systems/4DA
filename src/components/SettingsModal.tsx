import { LearnedBehaviorPanel } from './LearnedBehaviorPanel';
import { SystemHealthPanel } from './SystemHealthPanel';
import { IndexedDocumentsPanel } from './IndexedDocumentsPanel';
import { NaturalLanguageSearch } from './NaturalLanguageSearch';
import { SourceConfigPanel } from './SourceConfigPanel';
import { AIProviderSection } from './settings/AIProviderSection';
import { MonitoringSection } from './settings/MonitoringSection';
import { DigestSection } from './settings/DigestSection';
import { ContextDiscoverySection } from './settings/ContextDiscoverySection';
import { PersonalizationSection } from './settings/PersonalizationSection';
import type { Settings, MonitoringStatus, UserContext, SystemHealth } from '../types';
import type { OllamaStatus } from '../hooks/use-settings';

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
              <span>&#x2699;&#xfe0f;</span>
            </div>
            <h2 id="settings-modal-title" className="text-lg font-medium text-white">Settings</h2>
          </div>
          <button
            onClick={onClose}
            aria-label="Close settings"
            className="w-8 h-8 rounded-lg bg-[#1F1F1F] text-gray-500 hover:text-white hover:bg-[#2A2A2A] flex items-center justify-center transition-all"
          >
            &times;
          </button>
        </div>

        <div className="p-6 space-y-6">
          <AIProviderSection
            settings={settings}
            settingsForm={settingsForm}
            setSettingsForm={setSettingsForm}
            ollamaStatus={ollamaStatus}
            ollamaModels={ollamaModels}
            checkOllamaStatus={checkOllamaStatus}
          />

          <MonitoringSection
            monitoring={monitoring}
            monitoringInterval={monitoringInterval}
            setMonitoringInterval={setMonitoringInterval}
            onToggle={handleToggleMonitoring}
            onUpdateInterval={handleUpdateMonitoringInterval}
            onTestNotification={handleTestNotification}
          />

          <DigestSection setSettingsStatus={setSettingsStatus} />

          <SourceConfigPanel onStatusChange={setSettingsStatus} />

          <ContextDiscoverySection
            scanDirectories={scanDirectories}
            newScanDir={newScanDir}
            setNewScanDir={setNewScanDir}
            isScanning={isScanning}
            discoveredContext={discoveredContext}
            runAutoDiscovery={runAutoDiscovery}
            runFullScan={runFullScan}
            addScanDirectory={addScanDirectory}
            removeScanDirectory={removeScanDirectory}
          />

          <PersonalizationSection
            userContext={userContext}
            newInterest={newInterest}
            setNewInterest={setNewInterest}
            newExclusion={newExclusion}
            setNewExclusion={setNewExclusion}
            newTechStack={newTechStack}
            setNewTechStack={setNewTechStack}
            newRole={newRole}
            setNewRole={setNewRole}
            addInterest={addInterest}
            removeInterest={removeInterest}
            addExclusion={addExclusion}
            removeExclusion={removeExclusion}
            addTechStack={addTechStack}
            removeTechStack={removeTechStack}
            updateRole={updateRole}
          />

          <LearnedBehaviorPanel
            affinities={learnedAffinities}
            antiTopics={antiTopics}
            onRefresh={loadLearnedBehavior}
          />

          <NaturalLanguageSearch onStatusChange={setSettingsStatus} />

          <IndexedDocumentsPanel onStatusChange={setSettingsStatus} />

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
