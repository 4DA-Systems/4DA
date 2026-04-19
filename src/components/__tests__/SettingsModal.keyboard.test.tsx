// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Keyboard navigation tests for SettingsModal.
 *
 * Tests Tab through 5 tabs, Escape closes, aria-selected toggling.
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
      checkOllamaStatus: vi.fn(), modelRegistry: null, refreshModelRegistry: vi.fn(),
      monitoring: { enabled: false, interval_minutes: 30, is_checking: false, last_check_ago: null, total_checks: 0 },
      monitoringInterval: 30, notificationThreshold: 'high_and_above',
      setMonitoringInterval: vi.fn(), setNotificationThreshold: vi.fn(), toggleMonitoring: vi.fn(),
      updateMonitoringInterval: vi.fn(), testNotification: vi.fn(),
      scanDirectories: [], newScanDir: '', setNewScanDir: vi.fn(), isScanning: false, discoveredContext: null,
      runAutoDiscovery: vi.fn(), runFullScan: vi.fn(), addScanDirectory: vi.fn(), removeScanDirectory: vi.fn(),
      loadSettings: vi.fn(), loadMonitoringStatus: vi.fn(),
      loadDiscoveredContext: vi.fn(), loadUserContext: vi.fn(), loadSuggestedInterests: vi.fn(),
      tier: 'free', showTeamInviteDialog: false, setShowTeamInviteDialog: vi.fn(),
    };
    return selector(mockState);
  }),
}));

// Mock child components
vi.mock('../IndexedDocumentsPanel', () => ({ IndexedDocumentsPanel: () => <div data-testid="indexed-documents-panel" /> }));
vi.mock('../SourceConfigPanel', () => ({ SourceConfigPanel: () => <div data-testid="source-config-panel" /> }));
vi.mock('../settings/AIProviderSection', () => ({ AIProviderSection: () => <div data-testid="ai-provider-section" /> }));
vi.mock('../settings/MonitoringSection', () => ({ MonitoringSection: () => <div data-testid="monitoring-section" /> }));
vi.mock('../settings/DigestSection', () => ({ DigestSection: () => <div data-testid="digest-section" /> }));
vi.mock('../settings/ContextDiscoverySection', () => ({ ContextDiscoverySection: () => <div data-testid="context-discovery-section" /> }));
vi.mock('../settings/PersonalizationSection', () => ({ PersonalizationSection: () => <div data-testid="personalization-section" /> }));
vi.mock('../settings/CommunityIntelligenceSection', () => ({ CommunityIntelligenceSection: () => <div data-testid="community-intelligence-section" /> }));
vi.mock('../settings/LocaleSection', () => ({ LocaleSection: () => <div data-testid="locale-section" /> }));
vi.mock('../settings/LicenseSection', () => ({ LicenseSection: () => <div data-testid="license-section" /> }));
vi.mock('../ProValuePanel', () => ({ ProValuePanel: () => <div data-testid="pro-value-panel" /> }));

describe('SettingsModal keyboard navigation', () => {
  it('calls onClose when Escape key is pressed', () => {
    const onClose = vi.fn();
    render(<SettingsModal onClose={onClose} />);
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('has 5 tab elements with proper role', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(5);
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
    const tabNames = [
      'settings.tabs.general',
      'settings.tabs.intelligence',
      'settings.tabs.sources',
      'settings.tabs.projects',
      'settings.tabs.about',
    ];

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
    expect(closeBtn).not.toHaveAttribute('tabindex', '-1');
  });

  it('Save and Test buttons are focusable in Intelligence tab', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('tab', { name: 'settings.tabs.intelligence' }));
    const saveBtn = screen.getByText('settings.ai.saveConfiguration');
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
