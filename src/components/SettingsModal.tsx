// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState, useCallback, memo, useMemo, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { PanelErrorBoundary } from './PanelErrorBoundary';
import { SourceConfigPanel } from './SourceConfigPanel';
import { ContextDiscoverySection } from './settings/ContextDiscoverySection';
import { PersonalizationSection } from './settings/PersonalizationSection';
import { LearnedPreferencesSection } from './settings/LearnedPreferencesSection';
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

// Side-rail grouping — mirrors ctx.rs's grouped side menu, adapted to 4DA's
// six (growing) settings sections. Order within a group is preserved from TAB_IDS.
const TAB_GROUP_MEMBERS: Record<'configuration' | 'account', SettingsTab[]> = {
  configuration: ['general', 'intelligence', 'sources', 'projects'],
  account: ['team', 'about'],
};

const TAB_ICONS: Record<SettingsTab, ReactNode> = {
  general: (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
      <path d="M4 6h10M18 6h2M4 12h2M10 12h10M4 18h6M14 18h6" strokeLinecap="round" />
      <circle cx="16" cy="6" r="2" /><circle cx="8" cy="12" r="2" /><circle cx="12" cy="18" r="2" />
    </svg>
  ),
  intelligence: (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
      <path d="M12 3a6 6 0 0 0-4 10.5V16a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2v-2.5A6 6 0 0 0 12 3Z" strokeLinejoin="round" />
      <path d="M9.5 21h5" strokeLinecap="round" />
    </svg>
  ),
  sources: (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
      <path d="M4 6h16M4 12h16M4 18h16" strokeLinecap="round" />
      <circle cx="7" cy="6" r="1.5" fill="currentColor" stroke="none" />
      <circle cx="14" cy="12" r="1.5" fill="currentColor" stroke="none" />
      <circle cx="9" cy="18" r="1.5" fill="currentColor" stroke="none" />
    </svg>
  ),
  projects: (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
      <path d="M3 7a2 2 0 0 1 2-2h3.5l2 2H19a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" strokeLinejoin="round" />
    </svg>
  ),
  team: (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
      <circle cx="9" cy="8" r="3" /><path d="M15.5 5.5a3 3 0 0 1 0 5" strokeLinecap="round" />
      <path d="M3.5 19a5.5 5.5 0 0 1 11 0M16 14a5.5 5.5 0 0 1 4.5 5" strokeLinecap="round" />
    </svg>
  ),
  about: (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
      <circle cx="12" cy="12" r="9" /><path d="M12 11v5M12 7.5h.01" strokeLinecap="round" />
    </svg>
  ),
};

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
  const settingsInitialTab = useAppStore(s => s.settingsInitialTab);
  const setSettingsInitialTab = useAppStore(s => s.setSettingsInitialTab);
  const isTeamOrEnterprise = tier === 'team' || tier === 'enterprise';

  // Dynamically add Team tab only for Team/Enterprise tiers
  const TAB_IDS = useMemo<SettingsTab[]>(() => {
    if (isTeamOrEnterprise) {
      return ['general', 'intelligence', 'sources', 'projects', 'team', 'about'];
    }
    return BASE_TAB_IDS;
  }, [isTeamOrEnterprise]);

  // Group the (possibly Team-augmented) tab list for the side rail, dropping
  // any empty group so the header never renders above nothing.
  const TAB_GROUPS = useMemo(() => {
    const present = new Set(TAB_IDS);
    return (['configuration', 'account'] as const)
      .map(key => ({ key, ids: TAB_GROUP_MEMBERS[key].filter(id => present.has(id)) }))
      .filter(g => g.ids.length > 0);
  }, [TAB_IDS]);

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
    void loadSettings();
    void loadMonitoringStatus();
  // eslint-disable-next-line react-hooks/exhaustive-deps -- load once on mount
  }, []);

  // Lazy load data when a tab is first visited
  const initTab = useCallback((tab: SettingsTab) => {
    if (initialized.has(tab)) return;
    setInitialized(prev => new Set(prev).add(tab));
    switch (tab) {
      case 'projects': void loadDiscoveredContext(); void loadUserContext(); void loadSuggestedInterests(); break;
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- stable store actions
  }, [initialized]);

  const handleTabChange = (tab: SettingsTab) => { setActiveTab(tab); initTab(tab); };

  // Honor a one-shot deep link (e.g. the first-run "Add your stack" CTA opens
  // straight to Projects, where folders/stack are added — General is useless
  // for that and nobody finds the Projects tab on their own).
  useEffect(() => {
    if (settingsInitialTab && TAB_IDS.includes(settingsInitialTab as SettingsTab)) {
      handleTabChange(settingsInitialTab as SettingsTab);
      setSettingsInitialTab(null);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- run when the deep link is set
  }, [settingsInitialTab]);

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
      <div className="bg-bg-secondary border border-border rounded-xl w-full max-w-4xl max-h-[calc(100vh-4rem)] flex flex-col overflow-hidden shadow-2xl">
        {/* Header */}
        <div className="px-6 py-4 border-b border-border flex items-center justify-between flex-shrink-0">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
              {/* eslint-disable-next-line i18next/no-literal-string */}
              <span aria-hidden="true">&#x2699;&#xfe0f;</span>
            </div>
            <h2 id="settings-modal-title" className="text-lg font-medium text-text-primary">{t('settings.title')}</h2>
          </div>
          <button onClick={onClose} aria-label={t('settings.closeSettings')} className="w-8 h-8 rounded-lg bg-bg-tertiary text-text-muted hover:text-text-primary hover:bg-border flex items-center justify-center transition-all">
            &times;
          </button>
        </div>

        {/* Body: side rail + scrollable content column */}
        <div className="flex flex-1 min-h-0">
          {/* Side rail — grouped vertical navigation */}
          <nav
            className="w-52 flex-shrink-0 overflow-y-auto border-e border-border bg-bg-primary/40 py-3 px-2"
            aria-label={t('settings.navigation')}
          >
            {TAB_GROUPS.map(group => (
              <div key={group.key} className="mb-2 last:mb-0">
                <div id={`settings-group-${group.key}`} className="px-2.5 pt-2 pb-1 text-[10px] font-medium uppercase tracking-wider text-text-muted select-none">
                  {t(`settings.groups.${group.key}`)}
                </div>
                {/* One tablist per group keeps the ARIA valid (tablist children must be tabs)
                    while the visible header stays outside it, labelling the group. */}
                <div role="tablist" aria-orientation="vertical" aria-labelledby={`settings-group-${group.key}`}>
                  {group.ids.map(tabId => {
                    const isActive = activeTab === tabId;
                    return (
                      <button
                        key={tabId} id={`tab-${tabId}`} role="tab" aria-selected={isActive} aria-controls={`tabpanel-${tabId}`}
                        onClick={() => handleTabChange(tabId)}
                        className={`group relative w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-sm transition-all ${
                          isActive ? 'bg-orange-500/10 text-text-primary font-medium' : 'text-text-muted hover:text-text-secondary hover:bg-bg-tertiary'
                        }`}
                      >
                        {isActive && <span className="absolute start-0 top-1.5 bottom-1.5 w-0.5 rounded-full bg-orange-500" aria-hidden="true" />}
                        <span className={isActive ? 'text-orange-400' : 'text-text-muted group-hover:text-text-secondary'} aria-hidden="true">
                          {TAB_ICONS[tabId]}
                        </span>
                        <span>{t(`settings.tabs.${tabId}`)}</span>
                      </button>
                    );
                  })}
                </div>
              </div>
            ))}
          </nav>

          {/* Content column */}
          <div className="flex-1 min-w-0 overflow-y-auto">
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
              onToggleMonitoring={() => { void handleToggleMonitoring(); }}
              onUpdateInterval={() => { void handleUpdateMonitoringInterval(); }}
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
              checkOllamaStatus={(baseUrl?: string) => { void checkOllamaStatus(baseUrl); }}
              modelRegistry={modelRegistry}
              onRefreshRegistry={() => { void refreshModelRegistry(); }}
              setSettingsStatus={setSettingsStatus}
              saveSettings={() => { void saveSettings(); }}
              testConnection={() => { void testConnection(); }}
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
                    isScanning={isScanning} discoveredContext={discoveredContext} runAutoDiscovery={() => { void runAutoDiscovery(); }}
                    runFullScan={() => { void runFullScan(); }} addScanDirectory={() => { void addScanDirectory(); }} removeScanDirectory={(dir: string) => { void removeScanDirectory(dir); }} />
                </PanelErrorBoundary>
                <PanelErrorBoundary name="Indexed Documents"><IndexedDocumentsPanel onStatusChange={setSettingsStatus} /></PanelErrorBoundary>
                <PanelErrorBoundary name="Personalization"><PersonalizationSection /></PanelErrorBoundary>
                <PanelErrorBoundary name="Learned Preferences"><LearnedPreferencesSection /></PanelErrorBoundary>
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
            {/* eslint-disable i18next/no-literal-string */}
            <p className="text-xs text-text-muted">4DA v{__APP_VERSION__} &copy; 2025-2026 4DA Systems. All rights reserved.</p>
            <p className="text-xs text-text-muted mt-1">Licensed under FSL-1.1-Apache-2.0</p>
            {/* eslint-enable i18next/no-literal-string */}
          </div>
        </div>
          </div>{/* content column */}
        </div>{/* body: rail + content */}
      </div>

      {showTeamInviteDialog && <TeamInviteDialog onClose={() => setShowTeamInviteDialog(false)} />}
    </div>
  );
});
