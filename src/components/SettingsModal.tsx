import { useEffect, useState, useCallback, memo } from 'react';
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
import { StreetsMembershipSection } from './settings/StreetsMembershipSection';
import { ShowAllViewsToggle } from './settings/ShowAllViewsToggle';
import { NaturalLanguageQueryPanel } from './NaturalLanguageQuery';
import { ProValuePanel } from './ProValuePanel';
import { AboutPanel } from './AboutPanel';
import { TeamSection } from './settings/TeamSection';
import WaitlistSignup from './WaitlistSignup';
import { TeamInviteDialog } from './settings/TeamInviteDialog';
import { TeamSharedSources } from './team/TeamSharedSources';
import { AuditLogViewer } from './enterprise/AuditLogViewer';
import { WebhookManager } from './enterprise/WebhookManager';
import { OrgDashboard } from './enterprise/OrgDashboard';
import { PolicyEditor } from './enterprise/PolicyEditor';
import { AdminHealthDashboard } from './enterprise/AdminHealthDashboard';
import { TeamOnboardingWizard } from './enterprise/TeamOnboardingWizard';
import { SsoConfigPanel } from './enterprise/SsoConfigPanel';
import { DataExportPanel } from './enterprise/DataExportPanel';
import { ConfigDiagnostics } from './enterprise/ConfigDiagnostics';
import { WebhookDocsPanel } from './enterprise/WebhookDocsPanel';
import { useAppStore } from '../store';
import { translateError } from '../utils/error-messages';

// ============================================================================
// Types
// ============================================================================

type SettingsTab = 'general' | 'sources' | 'profile' | 'projects' | 'advanced' | 'team' | 'about';

const BASE_TAB_IDS: SettingsTab[] = ['general', 'sources', 'profile', 'projects', 'team', 'advanced', 'about'];
const TEAM_TAB_IDS: SettingsTab[] = ['general', 'sources', 'profile', 'projects', 'team', 'advanced', 'about'];

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
  const TAB_IDS = isTeamOrEnterprise ? TEAM_TAB_IDS : BASE_TAB_IDS;
  const [activeTab, setActiveTab] = useState<SettingsTab>('general');
  const [initialized, setInitialized] = useState<Set<SettingsTab>>(new Set(['general']));

  // Data selectors (may change — useShallow prevents unnecessary re-renders)
  const {
    settings, settingsForm, settingsStatus, ollamaStatus, ollamaModels, modelRegistry,
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
      modelRegistry: s.modelRegistry,
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
  const refreshModelRegistry = useAppStore(s => s.refreshModelRegistry);
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
      case 'team':
        // Team/enterprise data loads on-demand inside components
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

    const getFocusable = () => modal.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    );

    const initialFocusable = getFocusable();
    initialFocusable[0]?.focus();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
        return;
      }
      if (e.key !== 'Tab') return;
      const focusable = getFocusable();
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
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
                    modelRegistry={modelRegistry}
                    onRefreshRegistry={refreshModelRegistry}
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

          {/* Team & Enterprise Tab */}
          {activeTab === 'team' && (
            <div id="tabpanel-team" role="tabpanel">
              {isTeamOrEnterprise ? (
                <div className="space-y-6">
                  <PanelErrorBoundary name="Team Setup Wizard">
                    <TeamOnboardingWizard />
                  </PanelErrorBoundary>

                  <PanelErrorBoundary name="Team Sync">
                    <TeamSection onStatus={setSettingsStatus} />
                  </PanelErrorBoundary>

                  <PanelErrorBoundary name="Shared Sources">
                    <TeamSharedSources />
                  </PanelErrorBoundary>

                  {tier === 'enterprise' && (
                    <>
                      <PanelErrorBoundary name="Organization">
                        <OrgDashboard />
                      </PanelErrorBoundary>

                      <PanelErrorBoundary name="Audit Log">
                        <AuditLogViewer />
                      </PanelErrorBoundary>

                      <PanelErrorBoundary name="Webhooks">
                        <WebhookManager />
                      </PanelErrorBoundary>

                      <PanelErrorBoundary name="Retention Policies">
                        <PolicyEditor />
                      </PanelErrorBoundary>

                      <PanelErrorBoundary name="SSO">
                        <SsoConfigPanel />
                      </PanelErrorBoundary>

                      <PanelErrorBoundary name="Admin Health">
                        <AdminHealthDashboard />
                      </PanelErrorBoundary>

                      <PanelErrorBoundary name="Webhook Docs">
                        <WebhookDocsPanel />
                      </PanelErrorBoundary>
                    </>
                  )}

                  <PanelErrorBoundary name="Data Export">
                    <DataExportPanel />
                  </PanelErrorBoundary>

                  <PanelErrorBoundary name="Diagnostics">
                    <ConfigDiagnostics />
                  </PanelErrorBoundary>
                </div>
              ) : (
                <div className="space-y-6 py-2">
                  <WaitlistSignup tier="team" inline />
                  <div className="border-t border-[#2A2A2A] pt-6">
                    <WaitlistSignup tier="enterprise" inline />
                  </div>
                </div>
              )}
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

      {/* Team Invite Dialog (portal-style overlay) */}
      {showTeamInviteDialog && (
        <TeamInviteDialog onClose={() => setShowTeamInviteDialog(false)} />
      )}
    </div>
  );
});
