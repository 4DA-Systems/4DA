import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsModal } from './SettingsModal';
import { useAppStore } from '../store';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Default mock state covering all selectors SettingsModal uses
const createMockState = (overrides: Record<string, unknown> = {}): Record<string, unknown> => ({
  settings: { llm: { provider: 'anthropic', model: 'claude-3-5-haiku-20241022', has_api_key: false, base_url: null }, rerank: { enabled: false, max_items_per_batch: 10, min_embedding_score: 0.1, daily_token_limit: 100000, daily_cost_limit_cents: 50 }, usage: { tokens_today: 0, cost_today_cents: 0, tokens_total: 0, items_reranked: 0 }, embedding_threshold: 0.25 },
  settingsForm: { provider: 'anthropic', apiKey: '', model: 'claude-3-5-haiku-20241022', baseUrl: '', openaiApiKey: '', embeddingThreshold: 0.25, rerankEnabled: false, maxItemsPerBatch: 10, minEmbeddingScore: 0.1, dailyTokenLimit: 100000, dailyCostLimitCents: 50 },
  setSettingsFormFull: vi.fn(),
  settingsStatus: '',
  setSettingsStatus: vi.fn(),
  saveSettings: vi.fn(),
  testConnection: vi.fn(),
  ollamaStatus: null,
  ollamaModels: [],
  checkOllamaStatus: vi.fn(),
  modelRegistry: null,
  refreshModelRegistry: vi.fn(),
  monitoring: { enabled: false, interval_minutes: 30, is_checking: false, last_check_ago: null, total_checks: 0 },
  monitoringInterval: 30,
  notificationThreshold: 'high_and_above',
  setMonitoringInterval: vi.fn(),
  setNotificationThreshold: vi.fn(),
  toggleMonitoring: vi.fn(),
  updateMonitoringInterval: vi.fn(),
  testNotification: vi.fn(),
  scanDirectories: [],
  newScanDir: '',
  setNewScanDir: vi.fn(),
  isScanning: false,
  discoveredContext: null,
  runAutoDiscovery: vi.fn(),
  runFullScan: vi.fn(),
  addScanDirectory: vi.fn(),
  removeScanDirectory: vi.fn(),
  loadSettings: vi.fn(),
  loadMonitoringStatus: vi.fn(),
  loadDiscoveredContext: vi.fn(),
  loadUserContext: vi.fn(),
  loadSuggestedInterests: vi.fn(),
  tier: 'free',
  showTeamInviteDialog: false,
  setShowTeamInviteDialog: vi.fn(),
  ...overrides,
});

// Mock the Zustand store
vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
    return selector(createMockState());
  }),
}));

// Mock child components to keep tests focused on SettingsModal itself
vi.mock('./IndexedDocumentsPanel', () => ({
  IndexedDocumentsPanel: () => <div data-testid="indexed-documents-panel" />,
}));

vi.mock('./SourceConfigPanel', () => ({
  SourceConfigPanel: () => <div data-testid="source-config-panel" />,
}));

vi.mock('./settings/AIProviderSection', () => ({
  AIProviderSection: () => <div data-testid="ai-provider-section" />,
}));

vi.mock('./settings/MonitoringSection', () => ({
  MonitoringSection: () => <div data-testid="monitoring-section" />,
}));

vi.mock('./settings/DigestSection', () => ({
  DigestSection: () => <div data-testid="digest-section" />,
}));

vi.mock('./settings/ContextDiscoverySection', () => ({
  ContextDiscoverySection: () => <div data-testid="context-discovery-section" />,
}));

vi.mock('./settings/PersonalizationSection', () => ({
  PersonalizationSection: () => <div data-testid="personalization-section" />,
}));

vi.mock('./settings/CommunityIntelligenceSection', () => ({
  CommunityIntelligenceSection: () => <div data-testid="community-intelligence-section" />,
}));

vi.mock('./settings/LocaleSection', () => ({
  LocaleSection: () => <div data-testid="locale-section" />,
}));

vi.mock('./settings/LicenseSection', () => ({
  LicenseSection: () => <div data-testid="license-section" />,
}));

vi.mock('./ProValuePanel', () => ({
  ProValuePanel: () => <div data-testid="pro-value-panel" />,
}));

describe('SettingsModal', () => {
  it('does not render when not mounted (parent controls visibility)', () => {
    const show = false;
    const { container } = render(<div>{show && <SettingsModal onClose={vi.fn()} />}</div>);
    expect(container.querySelector('[role="dialog"]')).toBeNull();
  });

  it('renders the dialog when mounted', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  it('has role="dialog" and aria-modal="true"', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveAttribute('aria-modal', 'true');
  });

  it('has aria-labelledby pointing to the title', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveAttribute('aria-labelledby', 'settings-modal-title');
    expect(screen.getByText('settings.title')).toHaveAttribute('id', 'settings-modal-title');
  });

  it('calls onClose when close button is clicked', () => {
    const onClose = vi.fn();
    render(<SettingsModal onClose={onClose} />);

    const closeButton = screen.getByLabelText('Close settings');
    fireEvent.click(closeButton);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('renders the Settings title', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    expect(screen.getByText('settings.title')).toBeInTheDocument();
  });

  it('renders 5 tab buttons for free tier', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(5);
    expect(tabs.map(t => t.textContent)).toEqual([
      'settings.tabs.general',
      'settings.tabs.intelligence',
      'settings.tabs.sources',
      'settings.tabs.projects',
      'settings.tabs.about',
    ]);
  });

  it('General tab is active by default', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const generalTab = screen.getByRole('tab', { name: 'settings.tabs.general' });
    expect(generalTab).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByTestId('locale-section')).toBeInTheDocument();
    expect(screen.getByTestId('monitoring-section')).toBeInTheDocument();
  });

  it('switches to Intelligence tab and shows AI provider + license', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('tab', { name: 'settings.tabs.intelligence' }));
    expect(screen.getByTestId('ai-provider-section')).toBeInTheDocument();
    expect(screen.getByTestId('license-section')).toBeInTheDocument();
    expect(screen.getByText('settings.ai.saveConfiguration')).toBeInTheDocument();
    expect(screen.getByText('settings.testConnection')).toBeInTheDocument();
  });

  it('switches to Sources tab and shows SourceConfigPanel', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('tab', { name: 'settings.tabs.sources' }));
    expect(screen.getByTestId('source-config-panel')).toBeInTheDocument();
  });

  it('switches to Projects tab and shows PersonalizationSection', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('tab', { name: 'settings.tabs.projects' }));
    expect(screen.getByTestId('personalization-section')).toBeInTheDocument();
    expect(screen.getByTestId('context-discovery-section')).toBeInTheDocument();
  });

  it('Save/Test buttons are in Intelligence tab, not General', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    // General tab should NOT have save/test buttons
    expect(screen.queryByText('settings.ai.saveConfiguration')).not.toBeInTheDocument();
    // Switch to Intelligence
    fireEvent.click(screen.getByRole('tab', { name: 'settings.tabs.intelligence' }));
    expect(screen.getByText('settings.ai.saveConfiguration')).toBeInTheDocument();
    expect(screen.getByText('settings.testConnection')).toBeInTheDocument();
  });

  // --- Error-path tests ---

  it('shows error alert when settingsStatus contains "Error"', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(useAppStore).mockImplementation(((selector: any) => {
      return selector(createMockState({ settingsStatus: 'Error: Invalid API key' }));
    }) as any);
    render(<SettingsModal onClose={vi.fn()} />);
    const statusEl = screen.getByText('Error: Invalid API key');
    expect(statusEl.closest('[role="alert"]')).toBeInTheDocument();
    expect(statusEl).toBeVisible();
  });

  it('shows error alert when settingsStatus contains "failed"', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(useAppStore).mockImplementation(((selector: any) => {
      return selector(createMockState({ settingsStatus: 'Connection failed: timeout' }));
    }) as any);
    render(<SettingsModal onClose={vi.fn()} />);
    const statusEl = screen.getByText('Connection failed: timeout');
    expect(statusEl.closest('[role="alert"]')).toBeInTheDocument();
    expect(statusEl).toBeVisible();
  });

  it('shows success status with role="status" (not alert)', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(useAppStore).mockImplementation(((selector: any) => {
      return selector(createMockState({ settingsStatus: 'Settings saved!' }));
    }) as any);
    render(<SettingsModal onClose={vi.fn()} />);
    const statusEl = screen.getByText('Settings saved!');
    expect(statusEl.closest('[role="status"]')).toBeInTheDocument();
    expect(statusEl.closest('[role="alert"]')).not.toBeInTheDocument();
  });

  it('shows error alert when settingsStatus contains "Connection failed:"', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(useAppStore).mockImplementation(((selector: any) => {
      return selector(createMockState({ settingsStatus: 'Connection failed: ECONNREFUSED' }));
    }) as any);
    render(<SettingsModal onClose={vi.fn()} />);
    const statusEl = screen.getByText('Connection failed: ECONNREFUSED');
    expect(statusEl.closest('[role="alert"]')).toBeInTheDocument();
    expect(statusEl).toBeVisible();
  });

  it('does not show settings status strip when settingsStatus is empty', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(useAppStore).mockImplementation(((selector: any) => {
      return selector(createMockState({ settingsStatus: '' }));
    }) as any);
    const { container } = render(<SettingsModal onClose={vi.fn()} />);
    const statusStrip = container.querySelector('.mx-6.mt-4[role="alert"], .mx-6.mt-4[role="status"]');
    expect(statusStrip).toBeNull();
  });
});
