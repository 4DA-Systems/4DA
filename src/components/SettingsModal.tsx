// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState, useCallback, memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { PanelErrorBoundary } from './PanelErrorBoundary';
import { SourceConfigPanel } from './SourceConfigPanel';
import { ContextDiscoverySection } from './settings/ContextDiscoverySection';
import { PersonalizationSection } from './settings/PersonalizationSection';
import { IndexedDocumentsPanel } from './IndexedDocumentsPanel';
import { AboutPanel } from './AboutPanel';
import { SettingsGeneralTab } from './settings/SettingsGeneralTab';
import { SettingsIntelligenceTab } from './settings/SettingsIntelligenceTab';
import { SettingsTeamTab } from './settings/SettingsTeamTab';
import { TeamInviteDialog } from './settings/TeamInviteDialog';
import { useAppStore } from '../store';
import { translateError } from '../utils/error-messages';

// ============================================================================
// Types
// ============================================================================

type SettingsTab = 'general' | 'intelligence' | 'sources' | 'projects' | 'team' | 'about';

const BASE_TAB_IDS: SettingsTab[] = ['general', 'intelligence', 'sources', 'projects', 'about'];

// ============================================================================
// Props
// ============================================================================

interface SettingsModalProps {
  onClose: () => void;
}

// ============================================================================
// SettingsModal Component
// ============================================================================

export const SettingsModal = memo(function SettingsModal({ onClose }: SettingsModalProps) {
  const { t } = useTranslation();
  const tier = useAppStore(s => s.tier);
  const showTeamInviteDialog = useAppStore(s => s.showTeamInviteDialog);
  const setShowTeamInviteDialog = useAppStore(s => s.setShowTeamInviteDialog);
  const isTeamOrEnterprise = tier === 'team' || tier === 'enterprise';

  // Dynamically add Team tab only for Team/Enterprise tiers
  const TAB_IDS = useMemo<SettingsTab[]>(() => {
    if (isTeamOrEnterprise) {
      return ['general', 'intelligence', 'sources', 'projects', 'team', 'about'];
    }
    return BASE_TAB_IDS;
  }, [isTeamOrEnterprise]);

  const [activeTab, setActiveTab] = useState<SettingsTab>('general');
  const [initialized, setInitialized] = useState<Set<SettingsTab>>(new Set(['general']));

  // Data selectors — streamlined (removed ~20 unused selectors)
  const {
    settings, settingsForm, settingsStatus, ollamaStatus, ollamaModels, modelRegistry,
    monitoring, monitoringInterval,
    scanDirectories, newScanDir, isScanning, discoveredContext,
  } = useAppStore(
    useShallow((s) => ({
      settings: s.settings,
      settingsForm: s.settingsForm,
      settingsStatus: s.settingsStatus,
      ollamaStatus: s.ollamaStatus,
      ollamaModels: s.ollamaModels,
      modelRegistry: s.modelRegistry,
      monitoring: s.monitoring,
      monitoringInterval: s.monitoringInterval,
      scanDirectories: s.scanDirectories,
      newScanDir: s.newScanDir,
      isScanning: s.isScanning,
      discoveredContext: s.discoveredContext,
    })),
  );

  // Action selectors
  const setSettingsFormFull = useAppStore(s => s.setSettingsFormFull);
  const setSettingsStatus = useAppStore(s => s.setSettingsStatus);
  const saveSettings = useAppStore(s => s.saveSettings);
  const testConnection = useAppStore(s => s.testConnection);
  const checkOllamaStatus = useAppStore(s => s.checkOllamaStatus);
  const refreshModelRegistry = useAppStore(s => s.refreshModelRegistry);
  const setMonitoringInterval = useAppStore(s => s.setMonitoringInterval);
  const toggleMonitoring = useAppStore(s => s.toggleMonitoring);
  const updateMonitoringInterval = useAppStore(s => s.updateMonitoringInterval);
  const setNewScanDir = useAppStore(s => s.setNewScanDir);
  const runAutoDiscovery = useAppStore(s => s.runAutoDiscovery);
  const runFullScan = useAppStore(s => s.runFullScan);
  const addScanDirectory = useAppStore(s => s.addScanDirectory);
  const removeScanDirectory = useAppStore(s => s.removeScanDirectory);
  const loadSettings = useAppStore(s => s.loadSettings);
  const loadMonitoringStatus = useAppStore(s => s.loadMonitoringStatus);
  const loadDiscoveredContext = useAppStore(s => s.loadDiscoveredContext);
  const loadUserContext = useAppStore(s => s.loadUserContext);
  const loadSuggestedInterests = useAppStore(s => s.loadSuggestedInterests);

  // General + Intelligence tabs load on mount
  useEffect(() => {
    loadSettings();
    loadMonitoringStatus();
  // eslint-disable-next-line react-hooks/exhaustive-deps -- load once on mount
  }, []);

  // Lazy load data when a tab is first visited
  const initTab = useCallback((tab: SettingsTab) => {
    if (initialized.has(tab)) return;
    setInitialized(prev => new Set(prev).add(tab));
    switch (tab) {
      case 'projects': loadDiscoveredContext(); loadUserContext(); loadSuggestedInterests(); break;
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- stable store actions
  }, [initialized]);

  const handleTabChange = (tab: SettingsTab) => { setActiveTab(tab); initTab(tab); };

  // Focus trap
  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const modal = document.querySelector('[role="dialog"]') as HTMLElement;
    if (!modal) return;
    const getFocusable = () => modal.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    );
    getFocusable()[0]?.focus();
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.stopPropagation(); onClose(); return; }
      if (e.key !== 'Tab') return;
      const focusable = getFocusable();
      const first = focusable[0], last = focusable[focusable.length - 1];
      if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
    };
    modal.addEventListener('keydown', handleKeyDown);
    return () => { modal.removeEventListener('keydown', handleKeyDown); previouslyFocused?.focus(); };
  }, [onClose]);

  // Monitoring action wrappers
  const handleToggleMonitoring = async () => {
    try { const msg = await toggleMonitoring(); setSettingsStatus(msg); setTimeout(() => setSettingsStatus(''), 2000); }
    catch (error) { setSettingsStatus(`Error: ${translateError(error)}`); }
  };
  const handleUpdateMonitoringInterval = async () => {
    try { const msg = await updateMonitoringInterval(); setSettingsStatus(msg); setTimeout(() => setSettingsStatus(''), 2000); }
    catch (error) { setSettingsStatus(`Error: ${translateError(error)}`); }
  };
  return (
    <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-50 p-4" role="dialog" aria-modal="true" aria-labelledby="settings-modal-title">
      <div className="bg-bg-secondary border border-border rounded-xl w-full max-w-2xl max-h-[calc(100vh-4rem)] overflow-y-auto shadow-2xl">
        {/* Sticky Header + Tab Bar */}
        <div className="sticky top-0 bg-bg-secondary z-10">
          <div className="px-6 py-4 border-b border-border flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
                <span aria-hidden="true">&#x2699;&#xfe0f;</span>
              </div>
              <h2 id="settings-modal-title" className="text-lg font-medium text-white">{t('settings.title')}</h2>
            </div>
            <button onClick={onClose} aria-label="Close settings" className="w-8 h-8 rounded-lg bg-bg-tertiary text-text-muted hover:text-white hover:bg-border flex items-center justify-center transition-all">
              &times;
            </button>
          </div>
          <div className="px-6 flex gap-1 border-b border-border" role="tablist" aria-label="Settings navigation">
            {TAB_IDS.map(tabId => (
              <button key={tabId} id={`tab-${tabId}`} role="tab" aria-selected={activeTab === tabId} aria-controls={`tabpanel-${tabId}`} onClick={() => handleTabChange(tabId)}
                className={`px-4 py-3 text-sm transition-all relative ${activeTab === tabId ? 'text-orange-400 font-medium' : 'text-text-muted hover:text-text-secondary'}`}>
                {t(`settings.tabs.${tabId}`)}
                {activeTab === tabId && <span className="absolute bottom-0 start-0 end-0 h-0.5 bg-orange-500" />}
              </button>
            ))}
          </div>
        </div>

        {/* Status Strip */}
        {settingsStatus && (
          <div role={settingsStatus.includes('Error') || settingsStatus.includes('failed') ? 'alert' : 'status'} className={`mx-6 mt-4 text-sm p-3 rounded-lg border ${settingsStatus.includes('Error') || settingsStatus.includes('failed') ? 'bg-red-500/10 text-red-400 border-red-500/30' : 'bg-green-500/10 text-green-400 border-green-500/30'}`}>
            {settingsStatus}
          </div>
        )}

        {/* Tab Content */}
        <div className="p-6 space-y-6">
          {activeTab === 'general' && (
            <div id={`tabpanel-${activeTab}`} role="tabpanel" aria-labelledby={`tab-${activeTab}`}>
            <SettingsGeneralTab
              monitoring={monitoring}
              monitoringInterval={monitoringInterval}
              setMonitoringInterval={setMonitoringInterval}
              onToggleMonitoring={handleToggleMonitoring}
              onUpdateInterval={handleUpdateMonitoringInterval}
            />
            </div>
          )}

          {activeTab === 'intelligence' && (
            <div id={`tabpanel-${activeTab}`} role="tabpanel" aria-labelledby={`tab-${activeTab}`}>
            <SettingsIntelligenceTab
              settings={settings}
              settingsForm={settingsForm}
              setSettingsForm={setSettingsFormFull}
              ollamaStatus={ollamaStatus}
              ollamaModels={ollamaModels}
              checkOllamaStatus={checkOllamaStatus}
              modelRegistry={modelRegistry}
              onRefreshRegistry={refreshModelRegistry}
              setSettingsStatus={setSettingsStatus}
              saveSettings={saveSettings}
              testConnection={testConnection}
            />
            </div>
          )}

          {activeTab === 'sources' && (
            <div id="tabpanel-sources" role="tabpanel" aria-labelledby="tab-sources">
              <PanelErrorBoundary name="Source Configuration">
                <SourceConfigPanel onStatusChange={setSettingsStatus} />
              </PanelErrorBoundary>
            </div>
          )}

          {activeTab === 'projects' && (
            <div id="tabpanel-projects" role="tabpanel" aria-labelledby="tab-projects">
              <div className="space-y-6">
                <PanelErrorBoundary name="Context Discovery">
                  <ContextDiscoverySection scanDirectories={scanDirectories} newScanDir={newScanDir} setNewScanDir={setNewScanDir}
                    isScanning={isScanning} discoveredContext={discoveredContext} runAutoDiscovery={runAutoDiscovery}
                    runFullScan={runFullScan} addScanDirectory={addScanDirectory} removeScanDirectory={removeScanDirectory} />
                </PanelErrorBoundary>
                <PanelErrorBoundary name="Indexed Documents"><IndexedDocumentsPanel onStatusChange={setSettingsStatus} /></PanelErrorBoundary>
                <PanelErrorBoundary name="Personalization"><PersonalizationSection /></PanelErrorBoundary>
              </div>
            </div>
          )}

          {activeTab === 'team' && isTeamOrEnterprise && (
            <div id={`tabpanel-${activeTab}`} role="tabpanel" aria-labelledby={`tab-${activeTab}`}>
            <SettingsTeamTab tier={tier} isTeamOrEnterprise={isTeamOrEnterprise} setSettingsStatus={setSettingsStatus} />
            </div>
          )}

          {activeTab === 'about' && (
            <div id="tabpanel-about" role="tabpanel" aria-labelledby="tab-about">
              <PanelErrorBoundary name="About"><AboutPanel /></PanelErrorBoundary>
            </div>
          )}
        </div>

        {/* Copyright */}
        <div className="px-6 pb-6">
          <div className="pt-4 border-t border-border text-center">
            <p className="text-xs text-text-muted">4DA v{__APP_VERSION__} &copy; 2025-2026 4DA Systems. All rights reserved.</p>
            <p className="text-xs text-text-muted mt-1">Licensed under FSL-1.1-Apache-2.0</p>
          </div>
        </div>
      </div>

      {showTeamInviteDialog && <TeamInviteDialog onClose={() => setShowTeamInviteDialog(false)} />}
    </div>
  );
});
