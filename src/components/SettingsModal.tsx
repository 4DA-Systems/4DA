import { useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { PanelErrorBoundary } from './PanelErrorBoundary';
import { LearnedBehaviorPanel } from './LearnedBehaviorPanel';
import { SystemHealthPanel } from './SystemHealthPanel';
import { IndexedDocumentsPanel } from './IndexedDocumentsPanel';
import { NaturalLanguageSearch } from './NaturalLanguageSearch';
import { SourceConfigPanel } from './SourceConfigPanel';
import { AIProviderSection } from './settings/AIProviderSection';
import { MonitoringSection } from './settings/MonitoringSection';
import { DigestSection } from './settings/DigestSection';
import { LocaleSection } from './settings/LocaleSection';
import { LicenseSection } from './settings/LicenseSection';
import { ContextDiscoverySection } from './settings/ContextDiscoverySection';
import { PersonalizationSection } from './settings/PersonalizationSection';
import { CommunityIntelligenceSection } from './settings/CommunityIntelligenceSection';
import { DeveloperDnaPanel } from './DeveloperDna';
import { AttentionDashboard } from './settings/AttentionDashboard';
import { ProjectHealthRadar } from './settings/ProjectHealthRadar';
import { NaturalLanguageQueryPanel } from './NaturalLanguageQuery';
import { ProValuePanel } from './ProValuePanel';
import { AboutPanel } from './AboutPanel';
import { useAppStore } from '../store';
import { translateError } from '../utils/error-messages';
import type { StreetsTier } from '../store/playbook-slice';

// ============================================================================
// STREETS Membership Section
// ============================================================================

function StreetsMembershipSection({ onStatus }: { onStatus: (s: string) => void }) {
  const { t } = useTranslation();
  const streetsTier = useAppStore(s => s.streetsTier);
  const activateStreetsLicense = useAppStore(s => s.activateStreetsLicense);
  const loadStreetsTier = useAppStore(s => s.loadStreetsTier);
  const [key, setKey] = useState('');
  const [activating, setActivating] = useState(false);

  useEffect(() => { loadStreetsTier(); }, [loadStreetsTier]);

  const tierLabels: Record<StreetsTier, { label: string; color: string }> = {
    playbook: { label: t('settings.streets.tierPlaybook'), color: 'text-text-secondary' },
    community: { label: t('settings.streets.tierCommunity'), color: 'text-[#D4AF37]' },
    cohort: { label: t('settings.streets.tierCohort'), color: 'text-[#22C55E]' },
  };

  const { label, color } = tierLabels[streetsTier] || tierLabels.playbook;

  const handleActivate = async () => {
    if (!key.trim()) return;
    setActivating(true);
    const ok = await activateStreetsLicense(key.trim());
    setActivating(false);
    if (ok) {
      onStatus(t('settings.streets.activated'));
      setKey('');
      setTimeout(() => onStatus(''), 3000);
    } else {
      onStatus(t('settings.streets.invalidKey'));
      setTimeout(() => onStatus(''), 3000);
    }
  };

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <h3 className="text-sm font-medium text-white mb-3">{t('settings.streets.title')}</h3>
      <div className="flex items-center gap-2 mb-3">
        <span className="text-xs text-text-muted">{t('settings.streets.currentTier')}</span>
        <span className={`text-xs font-semibold ${color}`}>{label}</span>
      </div>
      {streetsTier === 'playbook' && (
        <div className="flex gap-2">
          <input
            type="text"
            value={key}
            onChange={e => setKey(e.target.value)}
            placeholder={t('settings.streets.placeholder')}
            className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-lg text-sm text-white placeholder-gray-600 focus:outline-none focus:border-[#D4AF37]/50"
          />
          <button
            onClick={handleActivate}
            disabled={activating || !key.trim()}
            className="px-4 py-2 text-sm font-medium text-black bg-[#D4AF37] rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50"
          >
            {activating ? '...' : t('action.activate')}
          </button>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Show All Views Toggle
// ============================================================================

function ShowAllViewsToggle() {
  const showAllViews = useAppStore(s => s.showAllViews);
  const setShowAllViews = useAppStore(s => s.setShowAllViews);

  return (
    <div className="flex items-center justify-between py-3">
      <div>
        <span className="text-white text-sm">Show all views</span>
        <p className="text-text-muted text-xs">Display all 9 navigation tabs regardless of usage</p>
      </div>
      <button
        onClick={() => setShowAllViews(!showAllViews)}
        role="switch"
        aria-checked={showAllViews}
        aria-label="Show all views"
        className={`relative w-10 h-5 rounded-full transition-colors ${
          showAllViews ? 'bg-green-500/40' : 'bg-gray-600'
        }`}
      >
        <span className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
          showAllViews ? 'translate-x-5' : 'translate-x-0'
        }`} />
      </button>
    </div>
  );
}

// ============================================================================
// Types
// ============================================================================

type SettingsTab = 'general' | 'sources' | 'profile' | 'projects' | 'advanced' | 'about';

const TAB_IDS: SettingsTab[] = ['general', 'sources', 'profile', 'projects', 'advanced', 'about'];

// ============================================================================
// Props
// ============================================================================

interface SettingsModalProps {
  onClose: () => void;
}

// ============================================================================
// SettingsModal Component
// ============================================================================

export function SettingsModal({ onClose }: SettingsModalProps) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<SettingsTab>('general');
  const [initialized, setInitialized] = useState<Set<SettingsTab>>(new Set(['general']));

  // Data selectors (may change — useShallow prevents unnecessary re-renders)
  const {
    settings, settingsForm, settingsStatus, ollamaStatus, ollamaModels,
    monitoring, monitoringInterval, notificationThreshold,
    scanDirectories, newScanDir, isScanning, discoveredContext,
    learnedAffinities, antiTopics, systemHealth,
    similarTopicQuery, similarTopicResults,
  } = useAppStore(
    useShallow((s) => ({
      settings: s.settings,
      settingsForm: s.settingsForm,
      settingsStatus: s.settingsStatus,
      ollamaStatus: s.ollamaStatus,
      ollamaModels: s.ollamaModels,
      monitoring: s.monitoring,
      monitoringInterval: s.monitoringInterval,
      notificationThreshold: s.notificationThreshold,
      scanDirectories: s.scanDirectories,
      newScanDir: s.newScanDir,
      isScanning: s.isScanning,
      discoveredContext: s.discoveredContext,
      learnedAffinities: s.learnedAffinities,
      antiTopics: s.antiTopics,
      systemHealth: s.systemHealth,
      similarTopicQuery: s.similarTopicQuery,
      similarTopicResults: s.similarTopicResults,
    })),
  );

  // Action selectors (stable references — no useShallow needed)
  const setSettingsFormFull = useAppStore(s => s.setSettingsFormFull);
  const setSettingsStatus = useAppStore(s => s.setSettingsStatus);
  const saveSettings = useAppStore(s => s.saveSettings);
  const testConnection = useAppStore(s => s.testConnection);
  const checkOllamaStatus = useAppStore(s => s.checkOllamaStatus);
  const setMonitoringInterval = useAppStore(s => s.setMonitoringInterval);
  const setNotificationThreshold = useAppStore(s => s.setNotificationThreshold);
  const toggleMonitoring = useAppStore(s => s.toggleMonitoring);
  const updateMonitoringInterval = useAppStore(s => s.updateMonitoringInterval);
  const testNotification = useAppStore(s => s.testNotification);
  const setNewScanDir = useAppStore(s => s.setNewScanDir);
  const runAutoDiscovery = useAppStore(s => s.runAutoDiscovery);
  const runFullScan = useAppStore(s => s.runFullScan);
  const addScanDirectory = useAppStore(s => s.addScanDirectory);
  const removeScanDirectory = useAppStore(s => s.removeScanDirectory);
  const loadLearnedBehavior = useAppStore(s => s.loadLearnedBehavior);
  const setSimilarTopicQuery = useAppStore(s => s.setSimilarTopicQuery);
  const runAnomalyDetection = useAppStore(s => s.runAnomalyDetection);
  const resolveAnomaly = useAppStore(s => s.resolveAnomaly);
  const findSimilarTopics = useAppStore(s => s.findSimilarTopics);
  const saveWatcherState = useAppStore(s => s.saveWatcherState);
  const loadSystemHealth = useAppStore(s => s.loadSystemHealth);
  const loadSettings = useAppStore(s => s.loadSettings);
  const loadMonitoringStatus = useAppStore(s => s.loadMonitoringStatus);
  const loadDiscoveredContext = useAppStore(s => s.loadDiscoveredContext);
  const loadUserContext = useAppStore(s => s.loadUserContext);
  const loadSuggestedInterests = useAppStore(s => s.loadSuggestedInterests);

  // General tab loads on mount
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
      case 'profile':
        loadUserContext();
        loadSuggestedInterests();
        loadLearnedBehavior();
        break;
      case 'projects':
        loadDiscoveredContext();
        break;
      case 'advanced':
        loadSystemHealth();
        break;
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- stable store actions
  }, [initialized]);

  const handleTabChange = (tab: SettingsTab) => {
    setActiveTab(tab);
    initTab(tab);
  };

  // Focus trap: cycle Tab within modal, auto-focus first element, restore on close
  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const modal = document.querySelector('[role="dialog"]') as HTMLElement;
    if (!modal) return;

    const focusable = modal.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    );
    const first = focusable[0];
    const last = focusable[focusable.length - 1];

    first?.focus();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
        return;
      }
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
    return () => {
      modal.removeEventListener('keydown', handleKeyDown);
      previouslyFocused?.focus();
    };
  }, [activeTab, onClose]);

  // Monitoring action wrappers (add status messages)
  const handleToggleMonitoring = async () => {
    try {
      const msg = await toggleMonitoring();
      setSettingsStatus(msg);
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${translateError(error)}`);
    }
  };

  const handleUpdateMonitoringInterval = async () => {
    try {
      const msg = await updateMonitoringInterval();
      setSettingsStatus(msg);
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${translateError(error)}`);
    }
  };

  const handleTestNotification = async () => {
    try {
      const msg = await testNotification();
      setSettingsStatus(msg);
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Notification error: ${translateError(error)}`);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-50 p-4" role="dialog" aria-modal="true" aria-labelledby="settings-modal-title">
      <div className="bg-bg-secondary border border-border rounded-xl w-full max-w-2xl max-h-[90vh] overflow-y-auto shadow-2xl">
        {/* Sticky Header + Tab Bar */}
        <div className="sticky top-0 bg-bg-secondary z-10">
          {/* Modal Header */}
          <div className="px-6 py-4 border-b border-border flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
                <span aria-hidden="true">&#x2699;&#xfe0f;</span>
              </div>
              <h2 id="settings-modal-title" className="text-lg font-medium text-white">{t('settings.title')}</h2>
            </div>
            <button
              onClick={onClose}
              aria-label="Close settings"
              className="w-8 h-8 rounded-lg bg-bg-tertiary text-text-muted hover:text-white hover:bg-border flex items-center justify-center transition-all"
            >
              &times;
            </button>
          </div>

          {/* Tab Bar */}
          <div className="px-6 flex gap-1 border-b border-border" role="tablist" aria-label="Settings navigation">
            {TAB_IDS.map(tabId => (
              <button
                key={tabId}
                role="tab"
                aria-selected={activeTab === tabId}
                aria-controls={`tabpanel-${tabId}`}
                onClick={() => handleTabChange(tabId)}
                className={`px-4 py-3 text-sm transition-all relative ${
                  activeTab === tabId
                    ? 'text-orange-400 font-medium'
                    : 'text-text-muted hover:text-text-secondary'
                }`}
              >
                {t(`settings.tabs.${tabId}`)}
                {activeTab === tabId && (
                  <span className="absolute bottom-0 left-0 right-0 h-0.5 bg-orange-500" />
                )}
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
          {/* General Tab */}
          {activeTab === 'general' && (
            <div id="tabpanel-general" role="tabpanel">
              <div className="space-y-6">
                <PanelErrorBoundary name="Language">
                  <LocaleSection />
                </PanelErrorBoundary>

                <ShowAllViewsToggle />

                <PanelErrorBoundary name="AI Provider">
                  <AIProviderSection
                    settings={settings}
                    settingsForm={settingsForm}
                    setSettingsForm={setSettingsFormFull}
                    ollamaStatus={ollamaStatus}
                    ollamaModels={ollamaModels}
                    checkOllamaStatus={checkOllamaStatus}
                  />
                </PanelErrorBoundary>

                <PanelErrorBoundary name="Monitoring">
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
                </PanelErrorBoundary>

                <PanelErrorBoundary name="Digest">
                  <DigestSection setSettingsStatus={setSettingsStatus} />
                </PanelErrorBoundary>

                <PanelErrorBoundary name="Community Intelligence">
                  <CommunityIntelligenceSection />
                </PanelErrorBoundary>

                <PanelErrorBoundary name="License">
                  <LicenseSection onStatus={setSettingsStatus} />
                </PanelErrorBoundary>

                <PanelErrorBoundary name="STREETS Membership">
                  <StreetsMembershipSection onStatus={setSettingsStatus} />
                </PanelErrorBoundary>

                {/* Actions */}
                <div className="flex gap-3 pt-2">
                  <button
                    onClick={saveSettings}
                    aria-label={t('settings.saveSettings')}
                    className="flex-1 px-4 py-3 text-sm bg-gradient-to-r from-orange-500 to-orange-600 text-white font-medium rounded-lg hover:from-orange-600 hover:to-orange-700 transition-all shadow-lg shadow-orange-500/20"
                  >
                    {t('settings.saveSettings')}
                  </button>
                  <button
                    onClick={testConnection}
                    aria-label={t('settings.testConnection')}
                    className="px-6 py-3 text-sm bg-bg-tertiary text-text-secondary border border-border rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
                  >
                    {t('settings.testConnection')}
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Sources Tab */}
          {activeTab === 'sources' && (
            <div id="tabpanel-sources" role="tabpanel">
              <PanelErrorBoundary name="Source Configuration">
                <SourceConfigPanel onStatusChange={setSettingsStatus} />
              </PanelErrorBoundary>
            </div>
          )}

          {/* Profile Tab */}
          {activeTab === 'profile' && (
            <div id="tabpanel-profile" role="tabpanel">
              <div className="space-y-6">
                <PanelErrorBoundary name="Personalization">
                  <PersonalizationSection />
                </PanelErrorBoundary>

                <PanelErrorBoundary name="Developer DNA">
                  <DeveloperDnaPanel />
                </PanelErrorBoundary>

                <PanelErrorBoundary name="Learned Behavior">
                  <LearnedBehaviorPanel
                    affinities={learnedAffinities}
                    antiTopics={antiTopics}
                    onRefresh={loadLearnedBehavior}
                  />
                </PanelErrorBoundary>
              </div>
            </div>
          )}

          {/* Projects Tab */}
          {activeTab === 'projects' && (
            <div id="tabpanel-projects" role="tabpanel">
              <div className="space-y-6">
                <PanelErrorBoundary name="Context Discovery">
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
                </PanelErrorBoundary>

                <PanelErrorBoundary name="Indexed Documents">
                  <IndexedDocumentsPanel onStatusChange={setSettingsStatus} />
                </PanelErrorBoundary>

                <PanelErrorBoundary name="Natural Language Search">
                  <NaturalLanguageSearch onStatusChange={setSettingsStatus} />
                </PanelErrorBoundary>
              </div>
            </div>
          )}

          {/* Advanced Tab */}
          {activeTab === 'advanced' && (
            <div id="tabpanel-advanced" role="tabpanel">
              <div className="space-y-6">
                <PanelErrorBoundary name="Pro Value">
                  <ProValuePanel />
                </PanelErrorBoundary>

                <PanelErrorBoundary name="Attention Dashboard">
                  <AttentionDashboard />
                </PanelErrorBoundary>

                <PanelErrorBoundary name="Query Panel">
                  <NaturalLanguageQueryPanel />
                </PanelErrorBoundary>

                <PanelErrorBoundary name="Project Health">
                  <ProjectHealthRadar />
                </PanelErrorBoundary>

                <PanelErrorBoundary name="System Health">
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
                </PanelErrorBoundary>
              </div>
            </div>
          )}

          {/* About Tab */}
          {activeTab === 'about' && (
            <div id="tabpanel-about" role="tabpanel">
              <PanelErrorBoundary name="About">
                <AboutPanel />
              </PanelErrorBoundary>
            </div>
          )}
        </div>

        {/* Copyright - outside tab content */}
        <div className="px-6 pb-6">
          <div className="pt-4 border-t border-border text-center">
            <p className="text-xs text-text-muted">
              4DA v{__APP_VERSION__} &copy; 2025-2026 4DA Systems. All rights reserved.
            </p>
            <p className="text-xs text-text-muted mt-1">
              Licensed under FSL-1.1-Apache-2.0
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
