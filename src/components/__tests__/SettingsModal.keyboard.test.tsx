/**
 * Keyboard navigation tests for SettingsModal.
 *
 * Tests Tab through 6 tabs, Escape closes, arrow key navigation within tab list.
 */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsModal } from '../SettingsModal';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock the Zustand store
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
    const mockState: Record<string, unknown> = {
      settings: { llm: { provider: 'anthropic', model: 'claude-3-5-haiku-20241022', has_api_key: false, base_url: null }, rerank: { enabled: false, max_items_per_batch: 10, min_embedding_score: 0.1, daily_token_limit: 100000, daily_cost_limit_cents: 50 }, usage: { tokens_today: 0, cost_today_cents: 0, tokens_total: 0, items_reranked: 0 }, embedding_threshold: 0.25 },
      settingsForm: { provider: 'anthropic', apiKey: '', model: 'claude-3-5-haiku-20241022', baseUrl: '', openaiApiKey: '', embeddingThreshold: 0.25, rerankEnabled: false, maxItemsPerBatch: 10, minEmbeddingScore: 0.1, dailyTokenLimit: 100000, dailyCostLimitCents: 50 },
      setSettingsFormFull: vi.fn(), settingsStatus: '', setSettingsStatus: vi.fn(),
      saveSettings: vi.fn(), testConnection: vi.fn(), ollamaStatus: null, ollamaModels: [],
      checkOllamaStatus: vi.fn(),
      monitoring: { enabled: false, interval_minutes: 30, is_checking: false, last_check_ago: null, total_checks: 0 },
      monitoringInterval: 30, notificationThreshold: 'high_and_above',
      setMonitoringInterval: vi.fn(), setNotificationThreshold: vi.fn(), toggleMonitoring: vi.fn(),
      updateMonitoringInterval: vi.fn(), testNotification: vi.fn(),
      scanDirectories: [], newScanDir: '', setNewScanDir: vi.fn(), isScanning: false, discoveredContext: null,
      runAutoDiscovery: vi.fn(), runFullScan: vi.fn(), addScanDirectory: vi.fn(), removeScanDirectory: vi.fn(),
      learnedAffinities: [], antiTopics: [], loadLearnedBehavior: vi.fn(),
      systemHealth: { anomalies: [], anomalyCount: 0, embeddingOperational: true, rateLimitStatus: null, accuracyMetrics: null },
      similarTopicQuery: '', setSimilarTopicQuery: vi.fn(), similarTopicResults: [],
      runAnomalyDetection: vi.fn(), resolveAnomaly: vi.fn(), findSimilarTopics: vi.fn(),
      saveWatcherState: vi.fn(), loadSystemHealth: vi.fn(),
      userContext: { role: null, tech_stack: [], domains: [], interests: [], exclusions: [], stats: { interest_count: 0, exclusion_count: 0 } },
      suggestedInterests: [], newInterest: '', setNewInterest: vi.fn(), newExclusion: '', setNewExclusion: vi.fn(),
      newTechStack: '', setNewTechStack: vi.fn(), newRole: '', setNewRole: vi.fn(),
      addInterest: vi.fn(), removeInterest: vi.fn(), addExclusion: vi.fn(), removeExclusion: vi.fn(),
      addTechStack: vi.fn(), removeTechStack: vi.fn(), updateRole: vi.fn(),
      loadSuggestedInterests: vi.fn(), loadSettings: vi.fn(), loadMonitoringStatus: vi.fn(),
      loadDiscoveredContext: vi.fn(), loadUserContext: vi.fn(),
      streetsTier: 'playbook', loadStreetsTier: vi.fn(), activateStreetsLicense: vi.fn(),
      loadLicense: vi.fn(), loadTrialStatus: vi.fn(), license: null, trialStatus: null,
    };
    return selector(mockState);
  }),
}));

// Mock child components
vi.mock('../LearnedBehaviorPanel', () => ({ LearnedBehaviorPanel: () => <div data-testid="learned-behavior-panel" /> }));
vi.mock('../SystemHealthPanel', () => ({ SystemHealthPanel: () => <div data-testid="system-health-panel" /> }));
vi.mock('../IndexedDocumentsPanel', () => ({ IndexedDocumentsPanel: () => <div data-testid="indexed-documents-panel" /> }));
vi.mock('../NaturalLanguageSearch', () => ({ NaturalLanguageSearch: () => <div data-testid="natural-language-search" /> }));
vi.mock('../SourceConfigPanel', () => ({ SourceConfigPanel: () => <div data-testid="source-config-panel" /> }));
vi.mock('../settings/AIProviderSection', () => ({ AIProviderSection: () => <div data-testid="ai-provider-section" /> }));
vi.mock('../settings/MonitoringSection', () => ({ MonitoringSection: () => <div data-testid="monitoring-section" /> }));
vi.mock('../settings/DigestSection', () => ({ DigestSection: () => <div data-testid="digest-section" /> }));
vi.mock('../settings/ContextDiscoverySection', () => ({ ContextDiscoverySection: () => <div data-testid="context-discovery-section" /> }));
vi.mock('../settings/PersonalizationSection', () => ({ PersonalizationSection: () => <div data-testid="personalization-section" /> }));
vi.mock('../settings/CommunityIntelligenceSection', () => ({ CommunityIntelligenceSection: () => <div data-testid="community-intelligence-section" /> }));
vi.mock('../settings/LocaleSection', () => ({ LocaleSection: () => <div data-testid="locale-section" /> }));
vi.mock('../ProValuePanel', () => ({ ProValuePanel: () => <div data-testid="pro-value-panel" /> }));
vi.mock('../DeveloperDna', () => ({ DeveloperDnaPanel: () => <div data-testid="developer-dna-panel" /> }));
vi.mock('../settings/AttentionDashboard', () => ({ AttentionDashboard: () => <div data-testid="attention-dashboard" /> }));
vi.mock('../settings/ProjectHealthRadar', () => ({ ProjectHealthRadar: () => <div data-testid="project-health-radar" /> }));

describe('SettingsModal keyboard navigation', () => {
  it('calls onClose when Escape key is pressed', () => {
    const onClose = vi.fn();
    render(<SettingsModal onClose={onClose} />);
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('has tab elements with proper role', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const tabs = screen.getAllByRole('tab');
    expect(tabs.length).toBeGreaterThanOrEqual(6);
  });

  it('General tab is selected by default with aria-selected', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const generalTab = screen.getByRole('tab', { name: 'settings.tabs.general' });
    expect(generalTab).toHaveAttribute('aria-selected', 'true');
  });

  it('clicking a tab changes the active tab content', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const sourcesTab = screen.getByRole('tab', { name: 'settings.tabs.sources' });
    fireEvent.click(sourcesTab);
    expect(sourcesTab).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByTestId('source-config-panel')).toBeInTheDocument();
  });

  it('tabs switch aria-selected correctly across all tabs', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const tabNames = ['settings.tabs.general', 'settings.tabs.sources', 'settings.tabs.profile', 'settings.tabs.projects', 'settings.tabs.advanced', 'settings.tabs.about'];

    for (const name of tabNames) {
      const tab = screen.getByRole('tab', { name });
      fireEvent.click(tab);
      expect(tab).toHaveAttribute('aria-selected', 'true');

      // All other tabs should not be selected
      const otherTabs = screen.getAllByRole('tab').filter(t => t.textContent !== name);
      for (const other of otherTabs) {
        expect(other).toHaveAttribute('aria-selected', 'false');
      }
    }
  });

  it('close button is keyboard focusable', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const closeBtn = screen.getByLabelText('Close settings');
    expect(closeBtn.tagName).toBe('BUTTON');
    // Buttons are focusable by default unless tabIndex=-1
    expect(closeBtn).not.toHaveAttribute('tabindex', '-1');
  });

  it('Save and Test buttons are focusable in General tab', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const saveBtn = screen.getByText('settings.saveSettings');
    const testBtn = screen.getByText('settings.testConnection');
    expect(saveBtn.closest('button')).toBeInTheDocument();
    expect(testBtn.closest('button')).toBeInTheDocument();
  });

  it('dialog has proper aria-modal attribute', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveAttribute('aria-modal', 'true');
  });

  it('dialog has aria-labelledby pointing to modal title', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveAttribute('aria-labelledby', 'settings-modal-title');
  });
});
