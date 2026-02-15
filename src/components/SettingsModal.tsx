import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
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
import { DeveloperDnaPanel } from './DeveloperDna';
import { AttentionDashboard } from './settings/AttentionDashboard';
import { ProjectHealthRadar } from './settings/ProjectHealthRadar';
import { useAppStore } from '../store';

// ============================================================================
// Props - now minimal after Zustand migration
// ============================================================================

interface SettingsModalProps {
  onClose: () => void;
}

// ============================================================================
// SettingsModal Component
// ============================================================================

export function SettingsModal({ onClose }: SettingsModalProps) {
  // Pull all needed state from Zustand store
  const settings = useAppStore(s => s.settings);
  const settingsForm = useAppStore(s => s.settingsForm);
  const setSettingsFormFull = useAppStore(s => s.setSettingsFormFull);
  const settingsStatus = useAppStore(s => s.settingsStatus);
  const setSettingsStatus = useAppStore(s => s.setSettingsStatus);
  const saveSettings = useAppStore(s => s.saveSettings);
  const testConnection = useAppStore(s => s.testConnection);
  const ollamaStatus = useAppStore(s => s.ollamaStatus);
  const ollamaModels = useAppStore(s => s.ollamaModels);
  const checkOllamaStatus = useAppStore(s => s.checkOllamaStatus);

  const monitoring = useAppStore(s => s.monitoring);
  const monitoringInterval = useAppStore(s => s.monitoringInterval);
  const setMonitoringInterval = useAppStore(s => s.setMonitoringInterval);
  const notificationThreshold = useAppStore(s => s.notificationThreshold);
  const setNotificationThreshold = useAppStore(s => s.setNotificationThreshold);
  const toggleMonitoring = useAppStore(s => s.toggleMonitoring);
  const updateMonitoringInterval = useAppStore(s => s.updateMonitoringInterval);
  const testNotification = useAppStore(s => s.testNotification);

  const scanDirectories = useAppStore(s => s.scanDirectories);
  const newScanDir = useAppStore(s => s.newScanDir);
  const setNewScanDir = useAppStore(s => s.setNewScanDir);
  const isScanning = useAppStore(s => s.isScanning);
  const discoveredContext = useAppStore(s => s.discoveredContext);
  const runAutoDiscovery = useAppStore(s => s.runAutoDiscovery);
  const runFullScan = useAppStore(s => s.runFullScan);
  const addScanDirectory = useAppStore(s => s.addScanDirectory);
  const removeScanDirectory = useAppStore(s => s.removeScanDirectory);

  const learnedAffinities = useAppStore(s => s.learnedAffinities);
  const antiTopics = useAppStore(s => s.antiTopics);
  const loadLearnedBehavior = useAppStore(s => s.loadLearnedBehavior);

  const systemHealth = useAppStore(s => s.systemHealth);
  const similarTopicQuery = useAppStore(s => s.similarTopicQuery);
  const setSimilarTopicQuery = useAppStore(s => s.setSimilarTopicQuery);
  const similarTopicResults = useAppStore(s => s.similarTopicResults);
  const runAnomalyDetection = useAppStore(s => s.runAnomalyDetection);
  const resolveAnomaly = useAppStore(s => s.resolveAnomaly);
  const findSimilarTopics = useAppStore(s => s.findSimilarTopics);
  const saveWatcherState = useAppStore(s => s.saveWatcherState);
  const loadSystemHealth = useAppStore(s => s.loadSystemHealth);

  const userContext = useAppStore(s => s.userContext);
  const suggestedInterests = useAppStore(s => s.suggestedInterests);
  const newInterest = useAppStore(s => s.newInterest);
  const setNewInterest = useAppStore(s => s.setNewInterest);
  const newExclusion = useAppStore(s => s.newExclusion);
  const setNewExclusion = useAppStore(s => s.setNewExclusion);
  const newTechStack = useAppStore(s => s.newTechStack);
  const setNewTechStack = useAppStore(s => s.setNewTechStack);
  const newRole = useAppStore(s => s.newRole);
  const setNewRole = useAppStore(s => s.setNewRole);
  const addInterest = useAppStore(s => s.addInterest);
  const removeInterest = useAppStore(s => s.removeInterest);
  const addExclusion = useAppStore(s => s.addExclusion);
  const removeExclusion = useAppStore(s => s.removeExclusion);
  const addTechStack = useAppStore(s => s.addTechStack);
  const removeTechStack = useAppStore(s => s.removeTechStack);
  const updateRole = useAppStore(s => s.updateRole);
  const loadSuggestedInterests = useAppStore(s => s.loadSuggestedInterests);

  // Load all settings data into the store when modal opens
  const loadSettings = useAppStore(s => s.loadSettings);
  const loadMonitoringStatus = useAppStore(s => s.loadMonitoringStatus);
  const loadDiscoveredContext = useAppStore(s => s.loadDiscoveredContext);
  const loadUserContext = useAppStore(s => s.loadUserContext);

  useEffect(() => {
    loadSettings();
    loadMonitoringStatus();
    loadDiscoveredContext();
    loadLearnedBehavior();
    loadSystemHealth();
    loadUserContext();
    loadSuggestedInterests();
  // eslint-disable-next-line react-hooks/exhaustive-deps -- load once on mount
  }, []);

  // Focus trap: cycle Tab within modal, auto-focus first element
  useEffect(() => {
    const modal = document.querySelector('[role="dialog"]') as HTMLElement;
    if (!modal) return;

    const focusable = modal.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    );
    const first = focusable[0];
    const last = focusable[focusable.length - 1];

    first?.focus();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;
      if (e.shiftKey) {
        if (document.activeElement === first) {
          e.preventDefault();
          last?.focus();
        }
      } else {
        if (document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }
    };

    modal.addEventListener('keydown', handleKeyDown);
    return () => modal.removeEventListener('keydown', handleKeyDown);
  }, []);

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

  const handleAddSuggestion = async (topic: string) => {
    try {
      await invoke('add_interest', { topic });
      await loadUserContext();
      await loadSuggestedInterests();
      setSettingsStatus('Interest added from suggestion');
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  };

  const handleDismissSuggestion = async (topic: string) => {
    try {
      await invoke('add_exclusion', { topic });
      await loadUserContext();
      await loadSuggestedInterests();
      setSettingsStatus('Suggestion dismissed');
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
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
            setSettingsForm={setSettingsFormFull}
            ollamaStatus={ollamaStatus}
            ollamaModels={ollamaModels}
            checkOllamaStatus={checkOllamaStatus}
          />

          <MonitoringSection
            monitoring={monitoring}
            monitoringInterval={monitoringInterval}
            setMonitoringInterval={setMonitoringInterval}
            notificationThreshold={notificationThreshold}
            onSetNotificationThreshold={setNotificationThreshold}
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
            suggestedInterests={suggestedInterests}
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
            onAddSuggestion={handleAddSuggestion}
            onDismissSuggestion={handleDismissSuggestion}
          />

          <DeveloperDnaPanel />

          <LearnedBehaviorPanel
            affinities={learnedAffinities}
            antiTopics={antiTopics}
            onRefresh={loadLearnedBehavior}
          />

          <NaturalLanguageSearch onStatusChange={setSettingsStatus} />

          <IndexedDocumentsPanel onStatusChange={setSettingsStatus} />

          <AttentionDashboard />

          <ProjectHealthRadar />

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

          {/* Copyright */}
          <div className="pt-4 border-t border-[#2A2A2A] text-center">
            <p className="text-xs text-[#666666]">
              4DA v1.0.0 &copy; 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
            </p>
            <p className="text-xs text-[#666666] mt-1">
              Licensed under BSL-1.1
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
